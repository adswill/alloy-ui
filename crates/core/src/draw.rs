//! The renderer contract: widgets emit [`DrawCommand`]s into a [`DrawList`];
//! a renderer (GPU or headless) consumes the list. Widgets never see wgpu.

use crate::geometry::{Color, Point, Rectangle};

/// A border drawn inside a quad's bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    /// Border colour.
    pub color: Color,
    /// Width in logical pixels.
    pub width: f32,
}

/// A drop shadow beneath a quad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Shadow colour (include alpha).
    pub color: Color,
    /// Offset from the quad.
    pub offset: Point,
    /// Blur radius.
    pub blur: f32,
}

/// One primitive drawing operation, in logical pixels, painter's order.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// A filled rounded rectangle with optional border and shadow.
    Quad {
        /// Bounds.
        bounds: Rectangle,
        /// Fill colour.
        fill: Color,
        /// Corner radius.
        radius: f32,
        /// Optional border.
        border: Option<Border>,
        /// Optional shadow.
        shadow: Option<Shadow>,
    },
    /// A run of text; shaping happens in the renderer.
    Text {
        /// Top-left of the text box.
        origin: Point,
        /// Content.
        content: String,
        /// Font size in logical pixels.
        size: f32,
        /// Colour.
        color: Color,
        /// Maximum width before wrapping, if any.
        max_width: Option<f32>,
        /// Font weight, 100 to 900 (400 regular, 600 semibold).
        weight: u16,
        /// Line height in logical pixels; `None` uses the renderer default.
        line_height: Option<f32>,
    },
    /// A straight line segment with round caps, e.g. a checkbox tick stroke.
    Line {
        /// Start point.
        from: Point,
        /// End point.
        to: Point,
        /// Stroke width in logical pixels.
        width: f32,
        /// Colour.
        color: Color,
    },
    /// An image by handle id registered with the renderer.
    Image {
        /// Destination bounds.
        bounds: Rectangle,
        /// Renderer-assigned image id.
        id: u64,
    },
    /// Begin clipping to `bounds`; must be matched by [`DrawCommand::PopClip`].
    PushClip(Rectangle),
    /// End the innermost clip.
    PopClip,
}

/// An ordered list of draw commands for one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrawList(pub Vec<DrawCommand>);

impl DrawList {
    /// Append a command.
    pub fn push(&mut self, cmd: DrawCommand) {
        self.0.push(cmd);
    }
}
