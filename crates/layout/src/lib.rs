//! Alloy's own flexbox-style layout engine (row/column, padding, spacing,
//! alignment; fill/shrink/fixed sizing). Contract types only for now.
#![warn(missing_docs)]

use alloy_ui_core::Size;

/// How a widget wants to be sized along one axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// Take a share of leftover space; the number is the share weight.
    Fill(u16),
    /// Take only what content needs.
    Shrink,
    /// Exactly this many logical pixels.
    Fixed(f32),
}

/// Main-axis direction of a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Children flow left to right.
    Horizontal,
    /// Children flow top to bottom.
    Vertical,
}

/// Cross-axis alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Start edge.
    #[default]
    Start,
    /// Centred.
    Center,
    /// End edge.
    End,
}

/// Padding on each side.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Padding {
    /// Top.
    pub top: f32,
    /// Right.
    pub right: f32,
    /// Bottom.
    pub bottom: f32,
    /// Left.
    pub left: f32,
}

/// Min/max size constraints passed down during layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    /// Smallest allowed size.
    pub min: Size,
    /// Largest allowed size.
    pub max: Size,
}
