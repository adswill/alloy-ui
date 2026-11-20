//! Basic geometry and colour types. All units are logical pixels.

/// A point in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position.
    pub y: f32,
}

/// A 2D offset.
pub type Vector = Point;

/// A width and height.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rectangle {
    /// Top-left corner.
    pub origin: Point,
    /// Extent.
    pub size: Size,
}

impl Point {
    /// Create a point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Size {
    /// Create a size.
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl Rectangle {
    /// Create a rectangle from position and size.
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// True when `p` lies inside (right and bottom edges exclusive).
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.origin.x
            && p.y >= self.origin.y
            && p.x < self.origin.x + self.size.width
            && p.y < self.origin.y + self.size.height
    }
}

/// A linear-light-agnostic sRGB colour with straight alpha, components 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha.
    pub a: f32,
}

impl Color {
    /// Opaque colour from 0xRRGGBB.
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xff) as f32 / 255.0,
            g: ((rgb >> 8) & 0xff) as f32 / 255.0,
            b: (rgb & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Same colour with a different alpha.
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Fully transparent.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
}
