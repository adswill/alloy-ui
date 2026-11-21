//! Windows extras: system settings, window effects, rich clipboard, URL scheme
//! registration. Type-checked from Linux, built by CI, never run by the author.

use crate::settings::{self, SystemSettings};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn reg_dword(key: &str, value: &str) -> Option<u32> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: out-pointers reference live locals of the stated size; the key
    // and value names are NUL-terminated UTF-16 that outlive the call.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(key).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    (rc == 0).then_some(data)
}

/// Dark mode and accent from the registry, reduced motion and high contrast
/// from `SystemParametersInfoW`.
pub fn read_settings() -> SystemSettings {
    use windows_sys::Win32::UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST,
        },
    };
    let mut animations: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL through pvParam.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&mut animations as *mut i32).cast(), 0)
    };
    // SAFETY: HIGHCONTRASTW is plain data; all-zero is a valid value.
    let mut hc: HIGHCONTRASTW = unsafe { std::mem::zeroed() };
    hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
    // SAFETY: SPI_GETHIGHCONTRAST fills the HIGHCONTRASTW whose cbSize we set.
    let hc_ok = unsafe {
        SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, (&mut hc as *mut HIGHCONTRASTW).cast(), 0)
    };
    SystemSettings {
        dark: super::windows::is_dark().unwrap_or(false),
        reduced_motion: ok != 0 && animations == 0,
        high_contrast: hc_ok != 0 && hc.dwFlags & HCF_HIGHCONTRASTON != 0,
        accent: reg_dword(r"Software\Microsoft\Windows\DWM", "AccentColor")
            .map(settings::parse_windows_accent),
    }
}

use crate::clipboard::{self, ClipContent};
use windows_sys::Win32::System::{
    DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
        OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    },
    Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
};

const CF_UNICODETEXT: u32 = 13;
const CF_DIB: u32 = 8;
const CF_HDROP: u32 = 15;

fn register(name: &str) -> u32 {
    // SAFETY: `name` is a NUL-terminated UTF-16 string that outlives the call.
    unsafe { RegisterClipboardFormatW(wide(name).as_ptr()) }
}

/// Put `bytes` on the (already open) clipboard under `format`. The system owns
/// the memory after a successful `SetClipboardData`.
fn set_bytes(format: u32, bytes: &[u8]) -> bool {
    // SAFETY: the HGLOBAL is allocated with room for `bytes`, locked only while
    // copying, and handed to the clipboard on success (freed by us otherwise
    // is skipped: a failed SetClipboardData leaks at most one small block).
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
        if h.is_null() {
            return false;
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            return false;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        GlobalUnlock(h);
        !SetClipboardData(format, h).is_null()
    }
}

/// Read the bytes of `format` from the (already open) clipboard.
fn get_bytes(format: u32) -> Option<Vec<u8>> {
    // SAFETY: the handle is locked while we copy exactly GlobalSize bytes out.
    unsafe {
        if IsClipboardFormatAvailable(format) == 0 {
            return None;
        }
        let h = GetClipboardData(format);
        if h.is_null() {
            return None;
        }
        let len = GlobalSize(h);
        let p = GlobalLock(h) as *const u8;
        if p.is_null() {
            return None;
        }
        let v = std::slice::from_raw_parts(p, len).to_vec();
        GlobalUnlock(h);
        Some(v)
    }
}

fn utf16_z(bytes: &[u8]) -> String {
    let u: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .take_while(|&c| c != 0)
        .collect();
    String::from_utf16_lossy(&u)
}

/// A bottom-up 32-bit BGRA `CF_DIB` (header + pixels) for `img`.
fn dib_from(img: &clipboard::ClipImage) -> Vec<u8> {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = Vec::with_capacity(40 + w * h * 4);
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // positive: bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bit count
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((w * h * 4) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]); // resolution, colours used / important
    for row in (0..h).rev() {
        for px in img.rgba[row * w * 4..(row + 1) * w * 4].chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    out
}

/// Decode a `CF_DIB` (24 or 32 bit, uncompressed) to RGBA.
fn image_from_dib(d: &[u8]) -> Option<clipboard::ClipImage> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?));
    let header = u32_at(0)? as usize;
    let (w, h) = (u32_at(4)? as i32, u32_at(8)? as i32);
    let bpp = u16::from_le_bytes(d.get(14..16)?.try_into().ok()?);
    let comp = u32_at(16)?;
    if w <= 0 || h == 0 || !matches!(comp, 0 | 3) || !matches!(bpp, 24 | 32) {
        return None;
    }
    // BI_BITFIELDS adds three mask DWORDs after the header.
    let data_at = header + if comp == 3 { 12 } else { 0 };
    let (w, top_down, h) = (w as usize, h < 0, h.unsigned_abs() as usize);
    let stride = (w * bpp as usize / 8).div_ceil(4) * 4;
    let mut rgba = vec![0u8; w * h * 4];
    for row in 0..h {
        let src_row = if top_down { row } else { h - 1 - row };
        let line = d.get(data_at + src_row * stride..data_at + src_row * stride + w * bpp as usize / 8)?;
        for (x, px) in line.chunks_exact(bpp as usize / 8).enumerate() {
            let o = (row * w + x) * 4;
            rgba[o..o + 4].copy_from_slice(&[px[2], px[1], px[0], if bpp == 32 { px[3] } else { 255 }]);
        }
    }
    // Many 32-bit DIBs leave alpha at 0; an all-transparent picture is never meant.
    if bpp == 32 && rgba.chunks_exact(4).all(|p| p[3] == 0) {
        rgba.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    Some(clipboard::ClipImage { width: w as u32, height: h as u32, rgba })
}

/// Everything on the clipboard: text, HTML Format, PNG or DIB, a file drop.
pub fn rich_read() -> Option<ClipContent> {
    // SAFETY: opened for the block and always closed.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let text = get_bytes(CF_UNICODETEXT).map(|b| utf16_z(&b));
        let html = get_bytes(register("HTML Format"))
            .and_then(|b| clipboard::parse_cf_html(&String::from_utf8_lossy(&b)));
        let image = get_bytes(register("PNG"))
            .and_then(|b| clipboard::decode_png(&b))
            .or_else(|| get_bytes(CF_DIB).and_then(|b| image_from_dib(&b)));
        let files = get_bytes(CF_HDROP)
            .and_then(|b| {
                // DROPFILES: pFiles (offset of the list) is the first DWORD; the
                // list is NUL-separated UTF-16, ended by an empty string.
                let off = u32::from_le_bytes(b.get(0..4)?.try_into().ok()?) as usize;
                let u: Vec<u16> = b
                    .get(off..)?
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                Some(
                    u.split(|&c| c == 0)
                        .take_while(|s| !s.is_empty())
                        .map(|s| std::path::PathBuf::from(String::from_utf16_lossy(s)))
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        CloseClipboard();
        Some(ClipContent { text, html, image, files })
    }
}

/// Replace the clipboard with every format of `c`.
pub fn rich_write(c: &ClipContent) -> bool {
    // SAFETY: opened for the block and always closed.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let mut ok = true;
        if let Some(t) = &c.text {
            let bytes: Vec<u8> = t
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            ok &= set_bytes(CF_UNICODETEXT, &bytes);
        }
        if let Some(h) = &c.html {
            let mut b = clipboard::cf_html(h).into_bytes();
            b.push(0);
            ok &= set_bytes(register("HTML Format"), &b);
        }
        if let Some(img) = &c.image {
            if let Some(png) = clipboard::encode_png(img) {
                ok &= set_bytes(register("PNG"), &png);
            }
            ok &= set_bytes(CF_DIB, &dib_from(img));
        }
        if !c.files.is_empty() {
            // DROPFILES { pFiles = 20, pt = (0, 0), fNC = 0, fWide = 1 }.
            let mut b = vec![0u8; 20];
            b[0..4].copy_from_slice(&20u32.to_le_bytes());
            b[16..20].copy_from_slice(&1u32.to_le_bytes());
            for p in &c.files {
                for u in p.to_string_lossy().encode_utf16() {
                    b.extend_from_slice(&u.to_le_bytes());
                }
                b.extend_from_slice(&[0, 0]);
            }
            b.extend_from_slice(&[0, 0]);
            ok &= set_bytes(CF_HDROP, &b);
        }
        CloseClipboard();
        ok
    }
}

use crate::EffectOutcome;
use alloy_ui_core::WindowEffect;
use windows_sys::Win32::{
    Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute},
    UI::Controls::MARGINS,
};

/// `DWMWA_USE_IMMERSIVE_DARK_MODE`.
const DWMWA_DARK: u32 = 20;
/// `DWMWA_SYSTEMBACKDROP_TYPE` (Windows 11 22H2 and later).
const DWMWA_BACKDROP: u32 = 38;

/// Windows 11 system backdrops through DWM: Mica, Acrylic ("transient window"
/// backdrop), or none. On older Windows the backdrop call fails and the
/// window stays opaque.
pub fn set_effect(window: &winit::window::Window, effect: WindowEffect) -> EffectOutcome {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let hwnd = match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get() as *mut std::ffi::c_void,
        _ => return EffectOutcome::Unsupported,
    };
    let dark = i32::from(window.theme() == Some(winit::window::Theme::Dark));
    let backdrop: i32 = match effect {
        WindowEffect::None | WindowEffect::Transparent => 1, // DWMSBT_NONE
        WindowEffect::Mica => 2,                              // DWMSBT_MAINWINDOW
        WindowEffect::Acrylic | WindowEffect::Blur => 3,      // DWMSBT_TRANSIENTWINDOW
    };
    let m = if effect == WindowEffect::None { 0 } else { -1 };
    let margins = MARGINS {
        cxLeftWidth: m,
        cxRightWidth: m,
        cyTopHeight: m,
        cyBottomHeight: m,
    };
    // SAFETY: `hwnd` is the live window; each attribute value is a 4-byte
    // integer that outlives its call; `margins` is a plain struct.
    let (dark_ok, backdrop_ok, extend_ok) = unsafe {
        (
            DwmSetWindowAttribute(hwnd, DWMWA_DARK, (&dark as *const i32).cast(), 4) == 0,
            DwmSetWindowAttribute(hwnd, DWMWA_BACKDROP, (&backdrop as *const i32).cast(), 4) == 0,
            DwmExtendFrameIntoClientArea(hwnd, &margins) == 0,
        )
    };
    let _ = dark_ok;
    match effect {
        WindowEffect::None => EffectOutcome::Applied,
        WindowEffect::Transparent if extend_ok => EffectOutcome::Applied,
        WindowEffect::Mica | WindowEffect::Acrylic | WindowEffect::Blur
            if backdrop_ok && extend_ok =>
        {
            EffectOutcome::Applied
        }
        _ => {
            // Undo the extended frame so an unsupported effect leaves no black glass.
            let zero = MARGINS {
                cxLeftWidth: 0,
                cxRightWidth: 0,
                cyTopHeight: 0,
                cyBottomHeight: 0,
            };
            // SAFETY: as above.
            unsafe { DwmExtendFrameIntoClientArea(hwnd, &zero) };
            EffectOutcome::Unsupported
        }
    }
}

/// Register `scheme` for the current user under `HKCU\Software\Classes`,
/// through the stock `reg` tool.
pub fn register_scheme(_app_id: &str, scheme: &str, exe: &str) -> Result<(), crate::PlatformError> {
    let key = format!(r"HKCU\Software\Classes\{scheme}");
    let run = |args: &[&str]| {
        std::process::Command::new("reg")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    let ok = run(&["add", &key, "/ve", "/d", &format!("URL:{scheme}"), "/f"])
        && run(&["add", &key, "/v", "URL Protocol", "/d", "", "/f"])
        && run(&[
            "add",
            &format!(r"{key}\shell\open\command"),
            "/ve",
            "/d",
            &format!("\"{exe}\" \"%1\""),
            "/f",
        ]);
    if ok {
        Ok(())
    } else {
        Err(crate::PlatformError(
            "register_scheme: `reg add` failed".into(),
        ))
    }
}
