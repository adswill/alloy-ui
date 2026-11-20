//! Alloy theming. Palette and type scale are defined in the theme workstream.
#![warn(missing_docs)]

use alloy_ui_core::Color;

/// Light or dark appearance, following the OS setting by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// Light.
    Light,
    /// Dark.
    Dark,
}

/// Resolved colour roles used by widgets.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    /// Window background.
    pub background: Color,
    /// Raised surface.
    pub surface: Color,
    /// Primary text.
    pub text: Color,
    /// Accent.
    pub accent: Color,
}
