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

/// Which bundled font family a text run uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontFamily {
    /// The proportional UI font.
    #[default]
    Sans,
    /// The monospace font, for code and editors.
    Mono,
    /// The proportional UI font in its true italic face.
    SansItalic,
    /// The monospace font in its true italic face.
    MonoItalic,
    /// The bundled Lucide icon font; text is icon codepoints (see `alloy_ui_widgets::Icon`).
    Lucide,
    /// The bundled Phosphor Regular icon font.
    Phosphor,
}

impl FontFamily {
    /// The italic counterpart (icon fonts have none and return themselves).
    pub const fn italic(self) -> Self {
        match self {
            Self::Sans | Self::SansItalic => Self::SansItalic,
            Self::Mono | Self::MonoItalic => Self::MonoItalic,
            other => other,
        }
    }
    /// True for the monospace faces.
    pub const fn is_mono(self) -> bool {
        matches!(self, Self::Mono | Self::MonoItalic)
    }
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
    /// A rounded rectangle filled with a two-stop linear gradient, with the
    /// same border and shadow behaviour as [`DrawCommand::Quad`].
    ///
    /// The gradient runs through the centre of `bounds` at `angle_deg`
    /// (0 = top to bottom, 90 = left to right) and is sized so `from` sits
    /// exactly on the start corner/edge and `to` on the opposite one (CSS
    /// `linear-gradient` semantics). Interpolation is in premultiplied space.
    GradientQuad {
        /// Bounds.
        bounds: Rectangle,
        /// Colour at the start of the gradient.
        from: Color,
        /// Colour at the end of the gradient.
        to: Color,
        /// Gradient direction in degrees: 0 = top to bottom, 90 = left to right.
        angle_deg: f32,
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
        /// Font family.
        family: FontFamily,
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

impl DrawCommand {
    /// A [`DrawCommand::GradientQuad`] with no border or shadow.
    pub fn gradient_quad(
        bounds: Rectangle,
        from: Color,
        to: Color,
        angle_deg: f32,
        radius: f32,
    ) -> Self {
        Self::GradientQuad {
            bounds,
            from,
            to,
            angle_deg,
            radius,
            border: None,
            shadow: None,
        }
    }

    /// Add a border to a [`DrawCommand::GradientQuad`] or [`DrawCommand::Quad`];
    /// other commands are returned unchanged.
    #[must_use]
    pub fn with_border(mut self, color: Color, width: f32) -> Self {
        if let Self::Quad { border, .. } | Self::GradientQuad { border, .. } = &mut self {
            *border = Some(Border { color, width });
        }
        self
    }

    /// Add a shadow to a [`DrawCommand::GradientQuad`] or [`DrawCommand::Quad`];
    /// other commands are returned unchanged.
    #[must_use]
    pub fn with_shadow(mut self, color: Color, offset: Point, blur: f32) -> Self {
        if let Self::Quad { shadow, .. } | Self::GradientQuad { shadow, .. } = &mut self {
            *shadow = Some(Shadow {
                color,
                offset,
                blur,
            });
        }
        self
    }
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
