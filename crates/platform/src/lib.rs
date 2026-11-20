//! Platform shell contracts. Each OS implements these in its own module
//! (`macos`, `linux`, `windows`); nothing outside this crate is platform-specific.
#![warn(missing_docs)]

use alloy_ui_core::Size;

/// Error type for platform operations.
#[derive(Debug)]
pub struct PlatformError(pub String);

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
}

/// Native dialogs.
pub trait Dialogs {
    /// Show an open-file picker; `None` if cancelled.
    fn open_file(&self) -> Result<Option<std::path::PathBuf>, PlatformError>;
    /// Show a message box.
    fn message(&self, title: &str, body: &str) -> Result<(), PlatformError>;
}

/// System appearance query.
pub trait SystemTheme {
    /// True when the OS is in dark mode.
    fn is_dark(&self) -> bool;
}
