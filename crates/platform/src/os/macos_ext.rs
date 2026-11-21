//! macOS extras: system settings, window effects, rich clipboard, URL
//! handling. Compiled by CI on macOS; never run on a Mac by the author.

use super::capture;
use crate::settings::{self, SystemSettings};

/// Dark mode, accent and reduced motion from the `defaults` database.
pub fn read_settings() -> SystemSettings {
    SystemSettings {
        dark: super::macos::is_dark().unwrap_or(false),
        reduced_motion: settings::parse_macos_flag(
            capture("defaults", &["read", "com.apple.universalaccess", "reduceMotion"]).as_deref(),
        ),
        high_contrast: settings::parse_macos_flag(
            capture("defaults", &["read", "com.apple.universalaccess", "increaseContrast"]).as_deref(),
        ),
        accent: settings::parse_macos_accent(
            capture("defaults", &["read", "-g", "AppleAccentColor"]).as_deref(),
        ),
    }
}
