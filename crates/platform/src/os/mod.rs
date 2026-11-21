//! Per-OS backends. Only this module tree contains `cfg(target_os)` code.
//! Backends shell out to each OS's own stock tools (no extra crates, no `unsafe`).

use crate::{Clipboard, Dialogs, PlatformError, SystemTheme};
#[cfg(unix)]
use std::process::{Command, Stdio};

#[cfg(target_os = "linux")]
mod dbus;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) mod linux_atspi;
#[cfg(target_os = "linux")]
mod linux_ext;
#[cfg(target_os = "linux")]
mod linux_tray;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos_ext;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos_access;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_ext;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_access;
#[cfg(target_os = "linux")]
mod wl_clip;
#[cfg(target_os = "linux")]
mod x11_clip;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "linux")]
use linux_ext as ext;
#[cfg(target_os = "macos")]
use macos_ext as ext;
#[cfg(target_os = "windows")]
use windows_ext as ext;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(target_os = "windows")]
use windows as backend;

#[cfg(unix)]
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

#[cfg(target_os = "linux")]
/// Run a program feeding `input` to stdin. Returns true on success.
pub(crate) fn feed(prog: &str, args: &[&str], input: &str) -> bool {
    use std::io::Write;
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

/// Current appearance and accessibility settings (see [`crate::settings`]).
pub(crate) fn read_settings() -> crate::settings::SystemSettings {
    ext::read_settings()
}

/// Dark/light query for the current OS.
pub fn default_system_theme() -> Box<dyn SystemTheme> {
    Box::new(SystemThemeImpl)
}

#[derive(Default)]
struct SystemClipboard {
    fallback: Option<String>,
    fallback_rich: Option<crate::ClipContent>,
}

impl Clipboard for SystemClipboard {
    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        Ok(backend::clipboard_read().or_else(|| self.fallback.clone()))
    }
    fn read_rich(&self) -> Result<crate::ClipContent, PlatformError> {
        if let Some(c) = ext::rich_read().filter(|c| !c.is_empty()) {
            return Ok(c);
        }
        let text = self.read_text()?;
        // Nothing richer on the system clipboard: hand back what this process
        // copied last, if it still matches the text that is there.
        Ok(match &self.fallback_rich {
            Some(r) if r.text == text || text.is_none() => r.clone(),
            _ => crate::ClipContent {
                text,
                ..crate::ClipContent::default()
            },
        })
    }
    fn write_rich(&mut self, c: &crate::ClipContent) -> Result<(), PlatformError> {
        let text_only = c.html.is_none() && c.image.is_none() && c.files.is_empty();
        self.fallback_rich = Some(c.clone());
        if text_only {
            return match &c.text {
                Some(t) => self.write_text(t),
                None => Ok(()),
            };
        }
        if let Some(t) = &c.text {
            self.fallback = Some(t.clone());
        }
        if !ext::rich_write(c) {
            log::debug!("no rich system clipboard; using in-process clipboard");
        }
        Ok(())
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
    fn open_files(&self) -> Result<Vec<std::path::PathBuf>, PlatformError> {
        backend::open_files()
    }
    fn save_file(&self, suggested_name: &str) -> Result<Option<std::path::PathBuf>, PlatformError> {
        backend::save_file(suggested_name)
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

/// Native menu bar / tray entry points; each backend implements these.
pub(crate) use backend::{clear_tray, set_menu, set_tray};
#[cfg(target_os = "linux")]
pub(crate) use linux::watch_theme;
#[cfg(windows)]
pub(crate) use windows::msg_hook;

static PENDING: std::sync::Mutex<Vec<crate::PlatformEvent>> = std::sync::Mutex::new(Vec::new());
static WAKER: std::sync::OnceLock<crate::shell::ShellProxy> = std::sync::OnceLock::new();

/// Register the proxy used to wake the loop when native callbacks queue events.
pub(crate) fn set_waker(p: crate::shell::ShellProxy) {
    let _ = WAKER.set(p);
}

/// Queue an event produced by a native callback (menu click, tray click) and
/// wake the loop so the shell delivers it on the main thread.
#[allow(dead_code)] // unused on Linux, which has no native callbacks yet
pub(crate) fn push_pending(e: crate::PlatformEvent) {
    if let Ok(mut q) = PENDING.lock() {
        q.push(e);
    }
    if let Some(w) = WAKER.get() {
        w.wake();
    }
}

/// Drain events queued by native callbacks.
pub(crate) fn take_pending() -> Vec<crate::PlatformEvent> {
    PENDING
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

/// Connect `window` to the OS accessibility service. `None` where the backend
/// is unavailable (the app then simply is not exposed to assistive technology).
pub(crate) fn new_access_adapter(
    window: &std::sync::Arc<winit::window::Window>,
    key: crate::shell::WindowKey,
    sink: crate::access::ActionSink,
) -> Option<Box<dyn crate::access::AccessAdapter>> {
    #[cfg(target_os = "linux")]
    {
        let w = window.clone();
        let origin: linux_atspi::Origin = std::sync::Arc::new(move || linux_atspi::ScreenOrigin {
            // `inner_position` fails on Wayland, where clients cannot know
            // where they are; screen coordinates are then window-relative.
            x: w.inner_position().map_or(0, |p| p.x),
            y: w.inner_position().map_or(0, |p| p.y),
            scale: w.scale_factor(),
        });
        Some(Box::new(linux_atspi::AtspiAdapter::new(
            key.0, origin, sink,
        )))
    }
    #[cfg(not(target_os = "linux"))]
    {
        backend::new_access_adapter(window, key, sink)
    }
}
