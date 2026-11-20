//! macOS backend using the stock `pbcopy`/`pbpaste`/`osascript`/`defaults` tools.
//! Compiled by CI on macOS; not run on a Mac by the author.

use super::{capture, feed};
use crate::{theme, PlatformError};
use std::path::PathBuf;

pub fn clipboard_read() -> Option<String> {
    capture("pbpaste", &[])
}

pub fn clipboard_write(text: &str) -> bool {
    feed("pbcopy", &[], text)
}

/// Escape a string for use inside an AppleScript double-quoted literal.
pub(crate) fn applescript_quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    // osascript exits non-zero when the user cancels.
    Ok(capture("osascript", &["-e", "POSIX path of (choose file)"])
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty()))
}

pub fn message(title: &str, body: &str) -> Result<(), PlatformError> {
    let script = format!(
        "display dialog \"{}\" with title \"{}\" buttons {{\"OK\"}} default button \"OK\"",
        applescript_quote(body),
        applescript_quote(title)
    );
    capture("osascript", &["-e", &script]);
    Ok(())
}

pub fn is_dark() -> Option<bool> {
    // The key is absent in light mode (command fails), "Dark" in dark mode.
    Some(theme::parse_macos(
        capture("defaults", &["read", "-g", "AppleInterfaceStyle"]).as_deref(),
    ))
}
