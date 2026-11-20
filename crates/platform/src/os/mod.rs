//! Per-OS backends. Only this module tree contains `cfg(target_os)` code.
//! Backends shell out to each OS's own stock tools (no extra crates, no `unsafe`).

use crate::{Clipboard, Dialogs, PlatformError, SystemTheme};
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(target_os = "windows")]
use windows as backend;

/// Run a program and capture stdout when it succeeds. `None` if it is missing or fails.
pub(crate) fn capture(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run a program feeding `input` to stdin. Returns true on success.
pub(crate) fn feed(prog: &str, args: &[&str], input: &str) -> bool {
    let Ok(mut child) = Command::new(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut si) = child.stdin.take() {
        if si.write_all(input.as_bytes()).is_err() {
            return false;
        }
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

/// Clipboard for the current OS. Falls back to an in-process buffer when no
/// system tool is available, so copy/paste inside the app always works.
pub fn default_clipboard() -> Box<dyn Clipboard> {
    Box::new(SystemClipboard::default())
}

/// Native dialogs for the current OS.
pub fn default_dialogs() -> Box<dyn Dialogs> {
    Box::new(SystemDialogs)
}

/// Dark/light query for the current OS.
pub fn default_system_theme() -> Box<dyn SystemTheme> {
    Box::new(SystemThemeImpl)
}

#[derive(Default)]
struct SystemClipboard {
    fallback: Option<String>,
}

impl Clipboard for SystemClipboard {
    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        Ok(backend::clipboard_read().or_else(|| self.fallback.clone()))
    }
    fn write_text(&mut self, text: &str) -> Result<(), PlatformError> {
        self.fallback = Some(text.to_owned());
        if !backend::clipboard_write(text) {
            log::debug!("no system clipboard tool; using in-process clipboard");
        }
        Ok(())
    }
}

struct SystemDialogs;

impl Dialogs for SystemDialogs {
    fn open_file(&self) -> Result<Option<std::path::PathBuf>, PlatformError> {
        backend::open_file()
    }
    fn message(&self, title: &str, body: &str) -> Result<(), PlatformError> {
        backend::message(title, body)
    }
}

struct SystemThemeImpl;

impl SystemTheme for SystemThemeImpl {
    fn is_dark(&self) -> bool {
        backend::is_dark().unwrap_or(false)
    }
}

/// Error for "no dialog helper installed".
#[allow(dead_code)]
pub(crate) fn unavailable(what: &str) -> PlatformError {
    PlatformError(format!("{what}: no supported helper found on this system"))
}
