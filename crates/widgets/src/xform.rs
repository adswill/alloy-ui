//! Moving, scaling and fading already-recorded draw commands. This is how a
//! group transitions without a render layer: every primitive is transformed on
//! its own, which is exact for content that does not overlap itself.

use alloy_ui_core::{Border, Color, DrawCommand, Point, Rectangle, Shadow, Size};

/// A uniform scale about `origin`, then a translation, with an opacity multiplier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xf {
    /// Multiplier for every colour's alpha.
    pub alpha: f32,
    /// Uniform scale factor.
    pub scale: f32,
    /// Fixed point of the scale.
    pub origin: Point,
    /// Translation applied after scaling.
    pub dx: f32,
    /// Translation applied after scaling.
    pub dy: f32,
}

impl Xf {
    /// Leaves everything as it is.
    pub const IDENTITY: Self = Self {
        alpha: 1.0,
        scale: 1.0,
        origin: Point { x: 0.0, y: 0.0 },
        dx: 0.0,
        dy: 0.0,
    };
    /// Opacity and translation only.
    pub fn fade_move(alpha: f32, dx: f32, dy: f32) -> Self {
        Self {
            alpha,
            dx,
            dy,
            ..Self::IDENTITY
        }
    }
    /// True when this changes nothing.
    pub fn is_identity(&self) -> bool {
        self.alpha >= 0.9999
            && (self.scale - 1.0).abs() < 1e-4
            && self.dx.abs() < 1e-3
            && self.dy.abs() < 1e-3
    }
    fn p(&self, q: Point) -> Point {
        Point::new(
            self.origin.x + (q.x - self.origin.x) * self.scale + self.dx,
            self.origin.y + (q.y - self.origin.y) * self.scale + self.dy,
        )
    }
    fn r(&self, b: Rectangle) -> Rectangle {
        Rectangle::new(
            self.p(b.origin),
            Size::new(b.size.width * self.scale, b.size.height * self.scale),
        )
    }
    fn c(&self, mut c: Color) -> Color {
        c.a *= self.alpha;
        c
    }
    fn border(&self, b: Option<Border>) -> Option<Border> {
        b.map(|b| Border {
            color: self.c(b.color),
            width: b.width * self.scale,
        })
    }
    fn shadow(&self, s: Option<Shadow>) -> Option<Shadow> {
        s.map(|s| Shadow {
            color: self.c(s.color),
            offset: Point::new(s.offset.x * self.scale, s.offset.y * self.scale),
            blur: s.blur * self.scale,
        })
    }
}

/// `cmd` under `xf`.
pub fn transform(cmd: DrawCommand, xf: &Xf) -> DrawCommand {
    let k = xf.scale;
    match cmd {
        DrawCommand::Quad {
            bounds,
            fill,
            radius,
            border,
            shadow,
        } => DrawCommand::Quad {
            bounds: xf.r(bounds),
            fill: xf.c(fill),
            radius: radius * k,
            border: xf.border(border),
            shadow: xf.shadow(shadow),
        },
        DrawCommand::GradientQuad {
            bounds,
            from,
            to,
            angle_deg,
            radius,
            border,
            shadow,
        } => DrawCommand::GradientQuad {
            bounds: xf.r(bounds),
            from: xf.c(from),
            to: xf.c(to),
            angle_deg,
            radius: radius * k,
            border: xf.border(border),
            shadow: xf.shadow(shadow),
        },
        DrawCommand::RadialGradient {
            bounds,
            center,
            gradient_radius,
            from,
            to,
            radius,
        } => DrawCommand::RadialGradient {
            bounds: xf.r(bounds),
            center: xf.p(center),
            gradient_radius: gradient_radius * k,
            from: xf.c(from),
            to: xf.c(to),
            radius: radius * k,
        },
        DrawCommand::Sheen {
            bounds,
            base,
            highlight,
            angle_deg,
            position,
            half_width,
            radius,
        } => DrawCommand::Sheen {
            bounds: xf.r(bounds),
            base: xf.c(base),
            highlight: xf.c(highlight),
            angle_deg,
            position,
            half_width,
            radius: radius * k,
        },
        DrawCommand::InnerShadow {
            bounds,
            radius,
            color,
            offset,
            blur,
        } => DrawCommand::InnerShadow {
            bounds: xf.r(bounds),
            radius: radius * k,
            color: xf.c(color),
            offset: Point::new(offset.x * k, offset.y * k),
            blur: blur * k,
        },
        DrawCommand::Grain {
            bounds,
            radius,
            amount,
            seed,
        } => DrawCommand::Grain {
            bounds: xf.r(bounds),
            radius: radius * k,
            amount: amount * xf.alpha,
            seed,
        },
        DrawCommand::BackdropBlur {
            bounds,
            radius,
            blur,
            tint,
        } => DrawCommand::BackdropBlur {
            bounds: xf.r(bounds),
            radius: radius * k,
            blur: blur * k,
            tint: xf.c(tint),
        },
        DrawCommand::Polygon { points, fill } => DrawCommand::Polygon {
            points: points.into_iter().map(|p| xf.p(p)).collect(),
            fill: xf.c(fill),
        },
        DrawCommand::PolygonPath {
            contours,
            rule,
            paint,
        } => DrawCommand::PolygonPath {
            contours: contours
                .into_iter()
                .map(|c| c.into_iter().map(|p| xf.p(p)).collect())
                .collect(),
            rule,
            paint: match paint {
                alloy_ui_core::PathPaint::Solid(c) => alloy_ui_core::PathPaint::Solid(xf.c(c)),
                alloy_ui_core::PathPaint::Linear { from, to, stops } => {
                    alloy_ui_core::PathPaint::Linear {
                        from: xf.p(from),
                        to: xf.p(to),
                        stops: stops.into_iter().map(|(o, c)| (o, xf.c(c))).collect(),
                    }
                }
            },
        },
        DrawCommand::Text {
            origin,
            content,
            size,
            color,
            max_width,
            weight,
            family,
            line_height,
        } => DrawCommand::Text {
            origin: xf.p(origin),
            content,
            size: size * k,
            color: xf.c(color),
            max_width: max_width.map(|w| w * k),
            weight,
            family,
            line_height: line_height.map(|l| l * k),
        },
        DrawCommand::Line {
            from,
            to,
            width,
            color,
        } => DrawCommand::Line {
            from: xf.p(from),
            to: xf.p(to),
            width: width * k,
            color: xf.c(color),
        },
        DrawCommand::Image { bounds, id } if xf.alpha >= 0.9999 => DrawCommand::Image {
            bounds: xf.r(bounds),
            id,
        },
        DrawCommand::Image { bounds, id } => DrawCommand::ImageAlpha {
            bounds: xf.r(bounds),
            id,
            alpha: xf.alpha,
        },
        DrawCommand::ImageAlpha { bounds, id, alpha } => DrawCommand::ImageAlpha {
            bounds: xf.r(bounds),
            id,
            alpha: alpha * xf.alpha,
        },
        DrawCommand::PushClip(b) => DrawCommand::PushClip(xf.r(b)),
        DrawCommand::PopClip => DrawCommand::PopClip,
    }
}
