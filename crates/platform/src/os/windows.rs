//! Windows backend using PowerShell and `reg.exe`, both present on every
//! supported Windows. Compiled by CI on Windows; not run by the author.

use super::{capture, feed};
use crate::{
    menu::{Menu, MenuItem},
    theme,
    tray::TraySettings,
    PlatformError, PlatformEvent,
};
use std::path::PathBuf;
use std::{ffi::c_void, sync::Mutex};
use windows_sys::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Shell::{
            Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
        },
        WindowsAndMessaging::*,
    },
};

fn ps(script: &str) -> Option<String> {
    capture(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )
}

pub fn clipboard_read() -> Option<String> {
    ps("Get-Clipboard -Raw").map(|s| s.trim_end_matches(['\r', '\n']).to_owned())
}

pub fn clipboard_write(text: &str) -> bool {
    feed("clip", &[], text)
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    let script = "Add-Type -AssemblyName System.Windows.Forms; \
        $d = New-Object System.Windows.Forms.OpenFileDialog; \
        if ($d.ShowDialog() -eq 'OK') { $d.FileName }";
    Ok(ps(script)
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty()))
}

pub fn message(title: &str, body: &str) -> Result<(), PlatformError> {
    let q = |s: &str| s.replace('\'', "''");
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         [void][System.Windows.Forms.MessageBox]::Show('{}','{}')",
        q(body),
        q(title)
    );
    ps(&script);
    Ok(())
}

pub fn is_dark() -> Option<bool> {
    let out = capture(
        "reg",
        &[
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ],
    );
    theme::parse_windows(out.as_deref())
}

/// Window message the tray icon posts to our window.
const WM_TRAY: u32 = WM_APP + 1;
const TRAY_ID: u32 = 1;

#[derive(Default)]
struct State {
    hwnd: isize,
    /// Menu-item command id (index + 1) -> action id.
    ids: Vec<String>,
    tray_menu: isize,
    tray_icon: isize,
}

static STATE: Mutex<State> = Mutex::new(State {
    hwnd: 0,
    ids: Vec::new(),
    tray_menu: 0,
    tray_icon: 0,
});

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn hwnd_of(w: &winit::window::Window) -> Result<isize, PlatformError> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match w.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => Ok(h.hwnd.get()),
        _ => Err(PlatformError("no Win32 window handle".into())),
    }
}

/// Build an HMENU tree, registering action ids in `ids`.
fn build(items: &[MenuItem], ids: &mut Vec<String>) -> isize {
    // SAFETY: plain Win32 calls; the returned handle is owned by the caller
    // (or by the window once attached with SetMenu / used as a popup).
    unsafe {
        let m = CreatePopupMenu();
        for item in items {
            match item {
                MenuItem::Separator => {
                    AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
                }
                MenuItem::Submenu(sub) => {
                    let h = build(&sub.items, ids);
                    AppendMenuW(
                        m,
                        MF_POPUP | MF_STRING,
                        h as usize,
                        wide(&sub.title).as_ptr(),
                    );
                }
                MenuItem::Action {
                    id,
                    label,
                    shortcut,
                    enabled,
                } => {
                    ids.push(id.clone());
                    let mut text = label.clone();
                    if let Some(s) = shortcut {
                        text.push('\t');
                        text.push_str(&crate::menu::format_shortcut(s));
                    }
                    let flags = MF_STRING | if *enabled { MF_ENABLED } else { MF_GRAYED };
                    AppendMenuW(m, flags, ids.len(), wide(&text).as_ptr());
                }
            }
        }
        m as isize
    }
}

pub fn set_menu(w: &winit::window::Window, menus: &[Menu]) -> Result<(), PlatformError> {
    let hwnd = hwnd_of(w)?;
    let mut st = STATE
        .lock()
        .map_err(|_| PlatformError("state poisoned".into()))?;
    st.hwnd = hwnd;
    st.ids.clear();
    // SAFETY: hwnd is a live window owned by winit on this (main) thread.
    unsafe {
        let bar = CreateMenu();
        for m in menus {
            let h = build(&m.items, &mut st.ids);
            AppendMenuW(
                bar,
                MF_POPUP | MF_STRING,
                h as usize,
                wide(&m.title).as_ptr(),
            );
        }
        SetMenu(hwnd as HWND, bar);
        DrawMenuBar(hwnd as HWND);
    }
    Ok(())
}

pub fn set_tray(w: &winit::window::Window, t: &TraySettings) -> Result<(), PlatformError> {
    if !t.icon_valid() || t.icon_width == 0 {
        return Err(PlatformError(
            "tray icon buffer does not match its size".into(),
        ));
    }
    let hwnd = hwnd_of(w)?;
    clear_tray();
    let mut st = STATE
        .lock()
        .map_err(|_| PlatformError("state poisoned".into()))?;
    st.hwnd = hwnd;
    // RGBA -> BGRA, which is what CreateIcon expects for 32-bit colour.
    let mut bgra = t.icon_rgba.clone();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let and_mask = vec![0u8; (t.icon_width as usize * t.icon_height as usize).div_ceil(8)];
    if let Some(menu) = &t.menu {
        st.tray_menu = build(&menu.items, &mut st.ids);
    }
    // SAFETY: buffers outlive the calls (CreateIcon copies them); NOTIFYICONDATAW
    // is zero-initialised then filled with valid fields before Shell_NotifyIconW.
    unsafe {
        let icon = CreateIcon(
            0 as _,
            t.icon_width as i32,
            t.icon_height as i32,
            1,
            32,
            and_mask.as_ptr(),
            bgra.as_ptr(),
        );
        if icon.is_null() {
            return Err(PlatformError("CreateIcon failed".into()));
        }
        st.tray_icon = icon as isize;
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd as HWND;
        nid.uID = TRAY_ID;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = icon;
        for (d, c) in nid.szTip.iter_mut().zip(t.tooltip.encode_utf16().take(127)) {
            *d = c;
        }
        if Shell_NotifyIconW(NIM_ADD, &nid) == 0 {
            return Err(PlatformError("Shell_NotifyIconW(NIM_ADD) failed".into()));
        }
    }
    Ok(())
}

pub fn clear_tray() {
    let Ok(mut st) = STATE.lock() else { return };
    if st.hwnd == 0 || st.tray_icon == 0 {
        return;
    }
    // SAFETY: removes the icon we added; handles were created by set_tray.
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = st.hwnd as HWND;
        nid.uID = TRAY_ID;
        Shell_NotifyIconW(NIM_DELETE, &nid);
        DestroyIcon(st.tray_icon as _);
        if st.tray_menu != 0 {
            DestroyMenu(st.tray_menu as _);
        }
    }
    st.tray_icon = 0;
    st.tray_menu = 0;
}

/// winit message hook: turns menu commands and tray callbacks into pending events.
/// Returns false so winit still processes the message normally.
///
/// # Safety contract
/// winit passes a pointer to a live `MSG` for the duration of the call.
pub fn msg_hook(msg: *const c_void) -> bool {
    // SAFETY: see above; we only read the struct.
    let m = unsafe { &*(msg as *const MSG) };
    let Ok(st) = STATE.lock() else { return false };
    if m.hwnd as isize != st.hwnd || st.hwnd == 0 {
        return false;
    }
    match m.message {
        WM_COMMAND if (m.wParam >> 16) & 0xffff == 0 => {
            let cmd = m.wParam & 0xffff;
            if let Some(id) = cmd.checked_sub(1).and_then(|i| st.ids.get(i)) {
                super::push_pending(PlatformEvent::MenuAction(id.clone()));
            }
        }
        WM_TRAY => match (m.lParam & 0xffff) as u32 {
            WM_LBUTTONUP => super::push_pending(PlatformEvent::TrayClicked),
            WM_RBUTTONUP if st.tray_menu != 0 => {
                let menu = st.tray_menu;
                let hwnd = st.hwnd;
                drop(st);
                // SAFETY: popup menu handle created by set_tray and still alive;
                // SetForegroundWindow is required so the menu dismisses on outside click.
                let cmd = unsafe {
                    let mut p: POINT = std::mem::zeroed();
                    GetCursorPos(&mut p);
                    SetForegroundWindow(hwnd as HWND);
                    TrackPopupMenu(
                        menu as _,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                        p.x,
                        p.y,
                        0,
                        hwnd as HWND,
                        std::ptr::null(),
                    )
                };
                if cmd > 0 {
                    if let Ok(st) = STATE.lock() {
                        if let Some(id) = st.ids.get(cmd as usize - 1) {
                            super::push_pending(PlatformEvent::MenuAction(id.clone()));
                        }
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
    false
}
