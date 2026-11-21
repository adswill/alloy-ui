//! Backend pieces shared by Android and iOS: no desktop-style dialogs, menu
//! bar or tray. Those report an honest error so apps can fall back to
//! in-window UI (sheets, tab bars), which is the mobile convention anyway.

use crate::{menu::Menu, tray::TraySettings, PlatformError};
use std::path::PathBuf;

fn unsupported(what: &str) -> PlatformError {
    PlatformError(format!("{what} is not available on mobile"))
}

#[cfg(target_os = "ios")]
pub use super::ios::{clipboard_read, clipboard_write};

/// Android keeps copy/paste inside the app until the JNI `ClipboardManager`
/// bridge exists; `SystemClipboard` falls back to its in-process buffer.
#[cfg(target_os = "android")]
pub fn clipboard_read() -> Option<String> {
    None
}
/// See [`clipboard_read`].
#[cfg(target_os = "android")]
pub fn clipboard_write(_text: &str) -> bool {
    false
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    Err(unsupported("the file picker"))
}
pub fn open_files() -> Result<Vec<PathBuf>, PlatformError> {
    Err(unsupported("the file picker"))
}
pub fn save_file(_name: &str) -> Result<Option<PathBuf>, PlatformError> {
    Err(unsupported("the save dialog"))
}
pub fn message(_title: &str, _body: &str) -> Result<(), PlatformError> {
    Err(unsupported("native message boxes"))
}
/// The window system's theme is read from `winit` by the shell; no extra query.
pub fn is_dark() -> Option<bool> {
    None
}
pub fn set_menu(_w: &winit::window::Window, _menus: &[Menu]) -> Result<(), PlatformError> {
    Err(unsupported("a menu bar"))
}
pub fn set_tray(_w: &winit::window::Window, _t: &TraySettings) -> Result<(), PlatformError> {
    Err(unsupported("a tray icon"))
}
pub fn clear_tray() {}
