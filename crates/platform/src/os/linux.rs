//! Linux backend (X11 and Wayland). Wayland tools are tried first when
//! `WAYLAND_DISPLAY` is set.

use super::{capture, feed, unavailable};
use crate::{dialog, menu::Menu, theme, tray::TraySettings, PlatformError, PlatformEvent};
use std::path::PathBuf;

fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn x11() -> bool {
    std::env::var_os("DISPLAY").is_some()
}

/// Order: Wayland helper (`wl-clipboard`) when on Wayland, our own X11
/// selection client (also covers XWayland), then `xclip`/`xsel`.
pub fn clipboard_read() -> Option<String> {
    if wayland() {
        if let Some(s) = capture("wl-paste", &["--no-newline"]) {
            return Some(s);
        }
    }
    if x11() {
        if let Some(s) = super::x11_clip::read() {
            return Some(s);
        }
    }
    capture("xclip", &["-selection", "clipboard", "-o"])
        .or_else(|| capture("xsel", &["--clipboard", "--output"]))
}

pub fn clipboard_write(text: &str) -> bool {
    (wayland() && feed("wl-copy", &[], text))
        || (x11() && super::x11_clip::write(text))
        || feed("xclip", &["-selection", "clipboard"], text)
        || feed("xsel", &["--clipboard", "--input"], text)
}

/// Run the first installed dialog helper. `Ok(None)` means it ran and the
/// user cancelled; `Err` means no helper exists.
fn run_dialog(
    what: &str,
    zenity: &[&str],
    kdialog: &[&str],
) -> Result<Option<String>, PlatformError> {
    for (prog, args) in [("zenity", zenity), ("kdialog", kdialog)] {
        if let Some(out) = capture(prog, args) {
            return Ok(Some(out));
        }
        if which(prog) {
            return Ok(None); // present, but cancelled
        }
    }
    Err(unavailable(&format!("{what} (needs zenity or kdialog)")))
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    let out = run_dialog("open_file", &["--file-selection"], &["--getopenfilename"])?;
    Ok(out.and_then(|o| dialog::parse_lines(&o).into_iter().next()))
}

pub fn open_files() -> Result<Vec<PathBuf>, PlatformError> {
    let out = run_dialog(
        "open_files",
        &["--file-selection", "--multiple", "--separator=\n"],
        &["--getopenfilename", ".", "--multiple", "--separate-output"],
    )?;
    Ok(out.map(|o| dialog::parse_lines(&o)).unwrap_or_default())
}

pub fn save_file(name: &str) -> Result<Option<PathBuf>, PlatformError> {
    let zname = format!("--filename={name}");
    let out = run_dialog(
        "save_file",
        &["--file-selection", "--save", "--confirm-overwrite", &zname],
        &["--getsavefilename", name],
    )?;
    Ok(out.and_then(|o| dialog::parse_lines(&o).into_iter().next()))
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

// winit has no menu bar on Linux. The tray is a StatusNotifierItem (see linux_tray.rs).
pub fn set_menu(_: &winit::window::Window, _: &[Menu]) -> Result<(), PlatformError> {
    Err(PlatformError(
        "native menu bar is not available on Linux".into(),
    ))
}
pub fn set_tray(_: &winit::window::Window, t: &TraySettings) -> Result<(), PlatformError> {
    super::linux_tray::start(t, true)
}
pub fn clear_tray() {
    super::linux_tray::stop();
}

/// Parse one line of `gsettings monitor` output, e.g. `color-scheme: 'prefer-dark'`.
/// Returns the dark flag, or None for unrelated or `default` values.
pub(crate) fn parse_monitor_line(line: &str) -> Option<bool> {
    let value = line.strip_prefix("color-scheme:")?;
    theme::parse_linux(Some(value), None)
}

/// Watch the desktop colour scheme and queue `ThemeChanged` when it flips.
/// Does nothing if `gsettings` is not installed. The helper process is killed
/// by the OS when the app exits (its stdout pipe closes).
pub fn watch_theme() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new("gsettings")
        .args(["monitor", "org.gnome.desktop.interface", "color-scheme"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
    else {
        return;
    };
    let Some(out) = child.stdout.take() else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("alloy-theme".into())
        .spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(dark) = parse_monitor_line(&line) {
                    super::push_pending(PlatformEvent::ThemeChanged { dark });
                }
            }
            let _ = child.wait();
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_lines() {
        assert_eq!(
            parse_monitor_line("color-scheme: 'prefer-dark'"),
            Some(true)
        );
        assert_eq!(
            parse_monitor_line("color-scheme: 'prefer-light'"),
            Some(false)
        );
        assert_eq!(parse_monitor_line("color-scheme: 'default'"), None);
        assert_eq!(parse_monitor_line("other: 'x'"), None);
    }
}
