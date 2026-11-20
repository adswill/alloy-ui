//! Layout vocabulary shared by widgets and the layout engine.

use crate::geometry::Size;

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

impl Padding {
    /// No padding.
    pub const ZERO: Self = Self::all(0.0);

    /// The same padding on all four sides.
    pub const fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    /// `h` on left and right, `v` on top and bottom.
    pub const fn symmetric(h: f32, v: f32) -> Self {
        Self {
            top: v,
            right: h,
            bottom: v,
            left: h,
        }
    }

    /// Sum of left and right (sanitised: NaN and negatives count as 0).
    pub fn horizontal(&self) -> f32 {
        clean(self.left) + clean(self.right)
    }

    /// Sum of top and bottom (sanitised: NaN and negatives count as 0).
    pub fn vertical(&self) -> f32 {
        clean(self.top) + clean(self.bottom)
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::NONE
    }
}

impl Limits {
    /// No constraint: min zero, max infinite.
    pub const NONE: Self = Self {
        min: Size::new(0.0, 0.0),
        max: Size::new(f32::INFINITY, f32::INFINITY),
    };

    /// Limits from explicit minimum and maximum sizes.
    pub const fn new(min: Size, max: Size) -> Self {
        Self { min, max }
    }

    /// Limits that force exactly `size`.
    pub const fn exact(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    /// Clamp `size` into the limits. Negative or NaN values count as 0, a NaN
    /// maximum as unbounded, and a maximum below the minimum yields the
    /// minimum, so the result is always finite-or-infinite but never NaN.
    pub fn clamp(&self, size: Size) -> Size {
        Size::new(
            clamp_axis(size.width, self.min.width, self.max.width),
            clamp_axis(size.height, self.min.height, self.max.height),
        )
    }
}

/// Replace NaN and negatives with 0.
#[doc(hidden)]
pub fn clean(v: f32) -> f32 {
    if v.is_nan() || v < 0.0 {
        0.0
    } else {
        v
    }
}

#[doc(hidden)]
pub fn clamp_axis(v: f32, min: f32, max: f32) -> f32 {
    let (lo, hi) = ordered(min, max);
    clean(v).clamp(lo, hi)
}

/// Sanitised (min, max) pair with `min <= max`.
#[doc(hidden)]
pub fn ordered(min: f32, max: f32) -> (f32, f32) {
    let lo = clean(min);
    let hi = if max.is_nan() { f32::INFINITY } else { max };
    (lo, if hi < lo { lo } else { hi })
}

/// Layout style of a `Node`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Main axis along which children flow.
    pub axis: Axis,
    /// Space between the node's border box and its children.
    pub padding: Padding,
    /// Gap between consecutive children along the main axis.
    pub spacing: f32,
    /// Default cross-axis alignment of children.
    pub align: Align,
    /// This node's own cross-axis alignment, overriding the parent's `align`.
    pub align_self: Option<Align>,
    /// Width sizing.
    pub width: Length,
    /// Height sizing.
    pub height: Length,
    /// Min/max size of this node's border box.
    pub limits: Limits,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            axis: Axis::Horizontal,
            padding: Padding::ZERO,
            spacing: 0.0,
            align: Align::Start,
            align_self: None,
            width: Length::Shrink,
            height: Length::Shrink,
            limits: Limits::NONE,
        }
    }
}
