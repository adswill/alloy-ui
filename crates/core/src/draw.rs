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
    /// A rounded rectangle filled with a radial gradient: `from` at `center`,
    /// `to` at distance `radius` and beyond. Border and shadow as for
    /// [`DrawCommand::Quad`]. Ripples, spotlights and soft highlights.
    RadialGradient {
        /// Bounds of the painted (rounded) shape; the gradient is clipped to it.
        bounds: Rectangle,
        /// Gradient centre, absolute logical pixels (may lie outside `bounds`).
        center: Point,
        /// Distance at which the gradient reaches `to`.
        gradient_radius: f32,
        /// Colour at the centre.
        from: Color,
        /// Colour at `gradient_radius` and beyond.
        to: Color,
        /// Corner radius of the shape.
        radius: f32,
    },
    /// A rounded rectangle with one soft highlight band travelling across a
    /// base colour: the primitive behind shimmer and sheen. The band is a
    /// smooth bump, so there is never a hard edge.
    Sheen {
        /// Bounds.
        bounds: Rectangle,
        /// Colour outside the band.
        base: Color,
        /// Colour at the band's centre.
        highlight: Color,
        /// Direction in degrees as for [`DrawCommand::GradientQuad`].
        angle_deg: f32,
        /// Band centre along the gradient line: 0 is the start, 1 the end;
        /// values outside 0..=1 place it off the shape.
        position: f32,
        /// Half the band's width as a fraction of the gradient line.
        half_width: f32,
        /// Corner radius.
        radius: f32,
    },
    /// A shadow cast *inside* a rounded rectangle, for wells and pressed
    /// states. Transparent in the middle, `color` at the edge shifted by
    /// `offset`, softened over `blur`.
    InnerShadow {
        /// Bounds of the shape the shadow stays inside.
        bounds: Rectangle,
        /// Corner radius of the shape.
        radius: f32,
        /// Shadow colour.
        color: Color,
        /// Where the light comes from: the shadow is thickest on the opposite edge.
        offset: Point,
        /// Softness in logical pixels.
        blur: f32,
    },
    /// Fine monochrome film grain over a rounded rectangle, each pixel
    /// lightened or darkened by a hash of its position and `seed`. Static for
    /// a given seed, so it never shimmers.
    Grain {
        /// Bounds.
        bounds: Rectangle,
        /// Corner radius.
        radius: f32,
        /// Peak strength, 0 to 1; 0.02 to 0.05 is the usable range.
        amount: f32,
        /// Selects the pattern.
        seed: u32,
    },
    /// Replace the pixels already drawn inside a rounded rectangle with a
    /// gaussian-blurred copy of themselves, then lay `tint` over them:
    /// frosted glass. Only content drawn *before* this command is blurred.
    BackdropBlur {
        /// Bounds.
        bounds: Rectangle,
        /// Corner radius.
        radius: f32,
        /// Gaussian sigma in logical pixels.
        blur: f32,
        /// Colour laid over the blurred pixels (usually the surface colour at
        /// 55 to 75% alpha).
        tint: Color,
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
    /// A filled simple polygon (convex or concave, either winding), edges
    /// anti-aliased. This is the primitive under pies, areas, canvas paths and
    /// 3D meshes. The outline must be simple: for holes, overlaps and self-crossing outlines
    /// use [`DrawCommand::PolygonPath`].
    Polygon {
        /// Outline vertices in order; fewer than three draws nothing.
        points: Vec<Point>,
        /// Fill colour (include alpha).
        fill: Color,
    },
    /// A filled path: any number of closed contours resolved under a fill rule, so
    /// holes, overlaps and self-intersections fill correctly, with an optional linear
    /// gradient. Edges are anti-aliased only on the outline of the filled region.
    PolygonPath {
        /// The closed contours (the last vertex joins the first).
        contours: Vec<Vec<Point>>,
        /// Which side of the contours is inside.
        rule: FillRule,
        /// Solid colour or linear gradient.
        paint: PathPaint,
    },
    /// An image by handle id registered with the renderer.
    Image {
        /// Destination bounds.
        bounds: Rectangle,
        /// Renderer-assigned image id.
        id: u64,
    },
    /// An image drawn at `alpha` opacity (0 to 1), so pictures can fade like
    /// everything else.
    ImageAlpha {
        /// Destination bounds.
        bounds: Rectangle,
        /// Renderer-assigned image id.
        id: u64,
        /// Opacity multiplier, 0 to 1.
        alpha: f32,
    },
    /// Begin clipping to `bounds`; must be matched by [`DrawCommand::PopClip`].
    PushClip(Rectangle),
    /// End the innermost clip.
    PopClip,
}

/// How a [`DrawCommand::PolygonPath`] decides what is inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillRule {
    /// Inside where the contours wind around a point a non-zero net number of times.
    #[default]
    NonZero,
    /// Inside where a ray from the point crosses an odd number of edges.
    EvenOdd,
}

/// The paint of a [`DrawCommand::PolygonPath`].
#[derive(Debug, Clone, PartialEq)]
pub enum PathPaint {
    /// One colour (include alpha).
    Solid(Color),
    /// Colours blended along the line `from` to `to`; before `from` the first stop
    /// holds and after `to` the last.
    Linear {
        /// Where the first stop sits.
        from: Point,
        /// Where the last stop sits.
        to: Point,
        /// `(offset 0..=1, colour)` stops in increasing offset order.
        stops: Vec<(f32, Color)>,
    },
}

impl PathPaint {
    /// The colour at `t` (0 to 1) along a linear gradient's stops.
    pub fn at(stops: &[(f32, Color)], t: f32) -> Color {
        let Some(first) = stops.first() else {
            return Color::TRANSPARENT;
        };
        if t <= first.0 {
            return first.1;
        }
        for w in stops.windows(2) {
            if t <= w[1].0 {
                let span = (w[1].0 - w[0].0).max(1e-6);
                let k = (t - w[0].0) / span;
                let (a, b) = (w[0].1, w[1].1);
                return Color {
                    r: a.r + (b.r - a.r) * k,
                    g: a.g + (b.g - a.g) * k,
                    b: a.b + (b.b - a.b) * k,
                    a: a.a + (b.a - a.a) * k,
                };
            }
        }
        stops[stops.len() - 1].1
    }
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
