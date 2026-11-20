//! System tray model and backend trait.
//!
//! Status: contract only. No native tray backend is implemented yet (see
//! `DECISIONS.md`); [`UnsupportedTray`] reports that honestly.

use crate::{menu::Menu, PlatformError};

/// Tray icon description.
#[derive(Debug, Clone, Default)]
pub struct TraySettings {
    /// Tooltip text.
    pub tooltip: String,
    /// RGBA8 icon pixels, row-major.
    pub icon_rgba: Vec<u8>,
    /// Icon width in pixels.
    pub icon_width: u32,
    /// Icon height in pixels.
    pub icon_height: u32,
    /// Menu shown when the icon is clicked.
    pub menu: Option<Menu>,
}

impl TraySettings {
    /// True when the icon buffer matches its stated dimensions.
    pub fn icon_valid(&self) -> bool {
        self.icon_rgba.len() == (self.icon_width as usize) * (self.icon_height as usize) * 4
    }
}

/// Creates the OS tray icon.
pub trait TrayBackend {
    /// Show the tray icon.
    fn show(&mut self, settings: &TraySettings) -> Result<(), PlatformError>;
    /// Remove it.
    fn hide(&mut self);
}

/// Placeholder backend.
pub struct UnsupportedTray;

impl TrayBackend for UnsupportedTray {
    fn show(&mut self, _: &TraySettings) -> Result<(), PlatformError> {
        Err(PlatformError("native tray not implemented yet".into()))
    }
    fn hide(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_validation() {
        let mut t = TraySettings {
            icon_width: 2,
            icon_height: 2,
            icon_rgba: vec![0; 16],
            ..Default::default()
        };
        assert!(t.icon_valid());
        t.icon_rgba.pop();
        assert!(!t.icon_valid());
    }
}
