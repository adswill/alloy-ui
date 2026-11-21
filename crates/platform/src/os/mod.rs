//! Per-OS backends. Only this module tree contains `cfg(target_os)` code.
//! Backends shell out to each OS's own stock tools (no extra crates, no `unsafe`).

use crate::{Clipboard, Dialogs, PlatformError, SystemTheme};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::{Command, Stdio};

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "linux")]
mod dbus;
#[cfg(target_os = "ios")]
#[allow(unsafe_code)]
mod ios;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_appmenu;
#[cfg(target_os = "linux")]
pub(crate) mod linux_atspi;
#[cfg(target_os = "linux")]
mod linux_ext;
#[cfg(target_os = "linux")]
mod linux_notify;
#[cfg(target_os = "linux")]
mod linux_tray;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos_access;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos_ext;
#[cfg(any(target_os = "android", target_os = "ios"))]
mod mobile;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_access;
#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows_ext;
#[cfg(target_os = "linux")]
mod wl_clip;
#[cfg(target_os = "linux")]
mod wl_hotkey;
#[cfg(target_os = "linux")]
mod x11_clip;
#[cfg(target_os = "linux")]
mod x11_hotkey;

#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "linux")]
use linux_ext as ext;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(target_os = "macos")]
use macos_ext as ext;
#[cfg(any(target_os = "android", target_os = "ios"))]
use mobile as backend;
#[cfg(any(target_os = "android", target_os = "ios"))]
use mobile::ext;
#[cfg(target_os = "windows")]
use windows as backend;
#[cfg(target_os = "windows")]
use windows_ext as ext;

#[cfg(any(target_os = "linux", target_os = "macos"))]
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

/// Register a URL scheme for the current user (see [`crate::deeplink::register_scheme`]).
pub(crate) fn register_scheme(app_id: &str, scheme: &str, exe: &str) -> Result<(), PlatformError> {
    ext::register_scheme(app_id, scheme, exe)
}

/// macOS only: start receiving "open URL / open files" Apple Events.
#[cfg(target_os = "macos")]
pub(crate) use ext::install_open_handler;

/// Apply a window effect to `window` (see [`crate::shell::ShellContext::set_effect`]).
pub(crate) fn set_effect(
    window: &winit::window::Window,
    effect: alloy_ui_core::WindowEffect,
) -> crate::EffectOutcome {
    ext::set_effect(window, effect)
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

#[cfg(target_os = "android")]
pub(crate) use android::app as android_app;
#[cfg(target_os = "macos")]
pub(crate) use backend::dialog_panel_summary;
#[cfg(any(target_os = "macos", windows))]
pub(crate) use backend::{activate_menu_item, activate_tray_item, menu_outline, tray_visible};
/// Native menu bar / tray entry points; each backend implements these.
pub(crate) use backend::{
    clear_tray, file_dialog, notify, register_hotkey, set_menu, set_tray, unregister_hotkeys,
};
/// Test hooks the Linux backend has no live-state readback for (its menus and tray live in
/// another process, reached over D-Bus).
#[cfg(not(any(target_os = "macos", windows)))]
mod no_readback {
    pub(crate) fn menu_outline() -> Vec<String> {
        Vec::new()
    }
    pub(crate) fn activate_menu_item(_: &str) -> bool {
        false
    }
    pub(crate) fn tray_visible() -> bool {
        false
    }
    pub(crate) fn activate_tray_item(_: &str) -> bool {
        false
    }
}
#[cfg(target_os = "linux")]
pub(crate) use linux::watch_theme;
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) use no_readback::{activate_menu_item, activate_tray_item, menu_outline, tray_visible};
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
    #[cfg(any(target_os = "macos", windows))]
    {
        backend::new_access_adapter(window, key, sink)
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // TalkBack/VoiceOver bridges are not written yet (see docs/MOBILE.md).
        let _ = (window, key, sink);
        None
    }
}

/// Insets the system chrome covers in `window`, in logical pixels. Zero on
/// desktop, where the window manager already keeps content clear of it.
pub(crate) fn safe_area(window: &winit::window::Window) -> crate::safe_area::Insets {
    #[cfg(target_os = "android")]
    {
        android::safe_area(window)
    }
    #[cfg(target_os = "ios")]
    {
        ios::safe_area(window)
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = window;
        crate::safe_area::Insets::ZERO
    }
}

/// A directory the app may write to that is not under `HOME`, where the OS
/// has no home directory (Android's internal storage). `None` elsewhere,
/// where the usual per-user config directories apply.
pub fn app_data_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "android")]
    {
        android::app()?.internal_data_path()
    }
    #[cfg(not(target_os = "android"))]
    {
        None
    }
}

/// Serialises the tests that talk to a D-Bus session and read the shared pending-event queue.
#[cfg(all(test, target_os = "linux"))]
pub(crate) static BUS_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());
