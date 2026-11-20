//! Windows backend on Win32 via windows-sys (clipboard, dialogs, theme registry
//! value, menu bar, tray). Type-checked from Linux and built by CI; never run by the author.

use crate::{
    menu::{Menu, MenuItem},
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

/// CF_UNICODETEXT clipboard format id.
const CF_UNICODETEXT: u32 = 13;

pub fn clipboard_read() -> Option<String> {
    use windows_sys::Win32::System::{
        DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard},
        Memory::{GlobalLock, GlobalUnlock},
    };
    // SAFETY: the clipboard is opened for the duration of the block and always
    // closed; the HGLOBAL is locked before reading and the NUL-terminated UTF-16
    // buffer is only read up to its terminator.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let mut out = None;
        let h = GetClipboardData(CF_UNICODETEXT);
        if !h.is_null() {
            let p = GlobalLock(h) as *const u16;
            if !p.is_null() {
                let mut len = 0;
                while *p.add(len) != 0 {
                    len += 1;
                }
                out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)));
                GlobalUnlock(h);
            }
        }
        CloseClipboard();
        out
    }
}

pub fn clipboard_write(text: &str) -> bool {
    use windows_sys::Win32::System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
    };
    let w = wide(text);
    // SAFETY: allocates a movable global block of exactly the buffer size, copies
    // into it while locked, and hands ownership to the clipboard on success
    // (freed ourselves on failure).
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let h = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2);
        let mut ok = false;
        if !h.is_null() {
            let p = GlobalLock(h) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(w.as_ptr(), p, w.len());
                GlobalUnlock(h);
                ok = !SetClipboardData(CF_UNICODETEXT, h).is_null();
            }
            if !ok {
                windows_sys::Win32::Foundation::GlobalFree(h);
            }
        }
        CloseClipboard();
        ok
    }
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    Ok(file_dialog(false, false, "")?.into_iter().next())
}

pub fn open_files() -> Result<Vec<PathBuf>, PlatformError> {
    file_dialog(true, false, "")
}

pub fn save_file(name: &str) -> Result<Option<PathBuf>, PlatformError> {
    Ok(file_dialog(false, true, name)?.into_iter().next())
}

/// Shared IFileOpenDialog / IFileSaveDialog call (the Vista+ common item
/// dialog: places bar, recent folders, long paths). Empty result = cancelled.
fn file_dialog(multi: bool, save: bool, name: &str) -> Result<Vec<PathBuf>, PlatformError> {
    com_dialog(multi, save, name).map_err(|e| PlatformError(format!("file dialog: {e}")))
}

/// Balances a successful `CoInitializeEx` with `CoUninitialize` on drop.
struct ComGuard(bool);

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: only constructed with `true` after CoInitializeEx
            // succeeded (S_OK or S_FALSE) on this thread, so this is its pair.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

fn com_dialog(multi: bool, save: bool, name: &str) -> windows::core::Result<Vec<PathBuf>> {
    use std::os::windows::ffi::OsStringExt;
    use windows::{
        core::{Interface, HRESULT, HSTRING},
        Win32::{
            Foundation::ERROR_CANCELLED,
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER,
                COINIT_APARTMENTTHREADED,
            },
            UI::Shell::{
                FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IShellItem,
                FOS_ALLOWMULTISELECT, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST,
                SIGDN_FILESYSPATH,
            },
        },
    };
    /// Read a shell item's file-system path.
    fn path_of(item: &IShellItem) -> windows::core::Result<PathBuf> {
        // SAFETY: only called while COM is initialised (see below). The PWSTR
        // returned by the shell is valid and NUL-terminated until freed with
        // CoTaskMemFree, which happens once, after the copy.
        unsafe {
            let p = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = PathBuf::from(std::ffi::OsString::from_wide(p.as_wide()));
            CoTaskMemFree(Some(p.0 as *const c_void));
            Ok(path)
        }
    }

    // SAFETY: COM is initialised on this thread before any interface is used
    // (an already-initialised apartment, as winit sets up, is fine: that
    // returns S_FALSE or RPC_E_CHANGED_MODE and the dialog still works). All
    // interface pointers are owned `windows` smart pointers released on drop,
    // and the shell-allocated path strings are freed in `path_of`.
    unsafe {
        let _com = ComGuard(CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok());
        let dialog: IFileDialog = if save {
            CoCreateInstance::<_, windows::Win32::UI::Shell::IFileSaveDialog>(
                &FileSaveDialog,
                None,
                CLSCTX_INPROC_SERVER,
            )?
            .cast()?
        } else {
            CoCreateInstance::<_, IFileOpenDialog>(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?
                .cast()?
        };
        let mut opts = dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
        if save {
            opts |= FOS_OVERWRITEPROMPT;
            if !name.is_empty() {
                dialog.SetFileName(&HSTRING::from(name))?;
            }
        } else if multi {
            opts |= FOS_ALLOWMULTISELECT;
        }
        dialog.SetOptions(opts)?;
        match dialog.Show(None) {
            Ok(()) => {}
            // The user dismissed the dialog: not an error.
            Err(e) if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        }
        if multi && !save {
            let items = dialog.cast::<IFileOpenDialog>()?.GetResults()?;
            (0..items.GetCount()?)
                .map(|i| path_of(&items.GetItemAt(i)?))
                .collect()
        } else {
            Ok(vec![path_of(&dialog.GetResult()?)?])
        }
    }
}

pub fn message(title: &str, body: &str) -> Result<(), PlatformError> {
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the modal call.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(body).as_ptr(),
            wide(title).as_ptr(),
            MB_OK,
        );
    }
    Ok(())
}

pub fn is_dark() -> Option<bool> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut data: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: out-pointers reference live locals of the stated size.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    (rc == 0).then_some(data == 0)
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

/// Connect `window` to UI Automation (see `windows_access`).
pub fn new_access_adapter(
    window: &std::sync::Arc<winit::window::Window>,
    _key: crate::shell::WindowKey,
    sink: crate::access::ActionSink,
) -> Option<Box<dyn crate::access::AccessAdapter>> {
    let a = super::windows_access::WinAdapter::new(window, sink)?;
    Some(Box::new(a))
}
