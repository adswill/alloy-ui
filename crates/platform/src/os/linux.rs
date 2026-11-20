//! Linux backend (X11 and Wayland). Wayland tools are tried first when
//! `WAYLAND_DISPLAY` is set.

use super::{capture, feed, unavailable};
use crate::{menu::Menu, theme, tray::TraySettings, PlatformError};
use std::path::PathBuf;

fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

pub fn clipboard_read() -> Option<String> {
    if wayland() {
        if let Some(s) = capture("wl-paste", &["--no-newline"]) {
            return Some(s);
        }
    }
    capture("xclip", &["-selection", "clipboard", "-o"])
        .or_else(|| capture("xsel", &["--clipboard", "--output"]))
}

pub fn clipboard_write(text: &str) -> bool {
    (wayland() && feed("wl-copy", &[], text))
        || feed("xclip", &["-selection", "clipboard"], text)
        || feed("xsel", &["--clipboard", "--input"], text)
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    // zenity exits 1 on cancel and prints the path otherwise; kdialog likewise.
    for (prog, args) in [
        ("zenity", &["--file-selection"][..]),
        ("kdialog", &["--getopenfilename"][..]),
    ] {
        if let Some(out) = capture(prog, args) {
            let p = out.trim();
            return Ok((!p.is_empty()).then(|| PathBuf::from(p)));
        }
        if which(prog) {
            return Ok(None); // present, but cancelled
        }
    }
    Err(unavailable("open_file (needs zenity or kdialog)"))
}

pub fn message(title: &str, body: &str) -> Result<(), PlatformError> {
    if which("zenity") {
        capture("zenity", &["--info", "--title", title, "--text", body]);
        return Ok(());
    }
    if which("kdialog") {
        capture("kdialog", &["--title", title, "--msgbox", body]);
        return Ok(());
    }
    Err(unavailable("message (needs zenity or kdialog)"))
}

pub fn is_dark() -> Option<bool> {
    let scheme = capture(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "color-scheme"],
    );
    let gtk = std::env::var("GTK_THEME").ok();
    theme::parse_linux(scheme.as_deref(), gtk.as_deref())
}

fn which(prog: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(prog).is_file()))
        .unwrap_or(false)
}

// winit has no menu bar on Linux and a tray needs D-Bus StatusNotifierItem,
// which is not implemented yet.
pub fn set_menu(_: &winit::window::Window, _: &[Menu]) -> Result<(), PlatformError> {
    Err(PlatformError(
        "native menu bar is not available on Linux".into(),
    ))
}
pub fn set_tray(_: &winit::window::Window, _: &TraySettings) -> Result<(), PlatformError> {
    Err(PlatformError("tray not implemented on Linux".into()))
}
pub fn clear_tray() {}
