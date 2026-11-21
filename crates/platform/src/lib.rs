//! Platform shell: windowing, input, clipboard, dialogs, menus, tray, fonts and
//! system theme. Contracts are traits; each OS supplies its backend in `os/`.
//!
//! Design: windowing and input go through `winit` on every OS (one code path,
//! X11 and Wayland both covered). Clipboard, dialogs and theme queries use the
//! OS's own tools or settings, selected by `cfg(target_os)` in `os/`, so this
//! crate contains no `unsafe` outside the native menu/tray code in `os/`
//! (each use carries a SAFETY comment).
#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod access;
pub mod clipboard;
pub mod convert;
pub mod deeplink;
pub mod dialog;
pub mod fonts;
pub mod hotkey;
pub mod instance;
pub mod menu;
pub mod notify;
mod os;
pub mod safe_area;
pub mod settings;
pub mod shell;
pub mod shortcut;
pub mod theme;
pub mod touch;
pub mod tray;

/// Android entry points (`android_main` hands its `AndroidApp` to [`android::init`]).
#[cfg(target_os = "android")]
pub mod android {
    pub use crate::os::android::init;
    pub use winit::platform::android::activity::AndroidApp;
}

use alloy_ui_core::Size;
use std::path::PathBuf;

pub use clipboard::{ClipContent, ClipImage};
pub use os::{app_data_dir, default_clipboard, default_dialogs, default_system_theme};
pub use shell::{request_window_effect, run, PlatformEvent, ShellContext, ShellHandler};

/// Error type for platform operations.
#[derive(Debug)]
pub struct PlatformError(pub String);

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PlatformError {}

/// What [`shell::ShellContext::set_effect`] managed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOutcome {
    /// The OS applies the material asked for (blur, Mica...).
    Applied,
    /// The window can be see-through, but the OS adds no material behind it:
    /// keep the page background mostly opaque so text stays readable.
    SeeThrough,
    /// Nothing happened; the window stays opaque.
    Unsupported,
}

/// Window creation options.
#[derive(Debug, Clone)]
pub struct WindowSettings {
    /// Title.
    pub title: String,
    /// Initial logical size.
    pub size: Size,
}

/// Clipboard access.
pub trait Clipboard {
    /// Read text.
    fn read_text(&self) -> Result<Option<String>, PlatformError>;
    /// Write text.
    fn write_text(&mut self, text: &str) -> Result<(), PlatformError>;
    /// Read everything the clipboard holds: text, HTML, an image, files.
    /// The default reads text only.
    fn read_rich(&self) -> Result<clipboard::ClipContent, PlatformError> {
        Ok(clipboard::ClipContent {
            text: self.read_text()?,
            ..clipboard::ClipContent::default()
        })
    }
    /// Put several formats on the clipboard at once (a paste target picks the
    /// richest one it understands). The default keeps only the text.
    fn write_rich(&mut self, content: &clipboard::ClipContent) -> Result<(), PlatformError> {
        match &content.text {
            Some(t) => self.write_text(t),
            None => Err(PlatformError("this clipboard only supports text".into())),
        }
    }
}

/// Native dialogs.
pub trait Dialogs {
    /// Show an open-file picker; `None` if cancelled.
    fn open_file(&self) -> Result<Option<PathBuf>, PlatformError>;
    /// Show an open-file picker that allows several selections. Empty if
    /// cancelled. The default forwards to [`Dialogs::open_file`].
    fn open_files(&self) -> Result<Vec<PathBuf>, PlatformError> {
        Ok(self.open_file()?.into_iter().collect())
    }
    /// Show a save-file picker pre-filled with `suggested_name`; `None` if
    /// cancelled. The OS dialog asks about overwriting existing files.
    fn save_file(&self, _suggested_name: &str) -> Result<Option<PathBuf>, PlatformError> {
        Err(PlatformError("save_file is not supported here".into()))
    }
    /// Show a message box.
    fn message(&self, title: &str, body: &str) -> Result<(), PlatformError>;
}

/// System appearance query.
pub trait SystemTheme {
    /// True when the OS is in dark mode.
    fn is_dark(&self) -> bool;
}
