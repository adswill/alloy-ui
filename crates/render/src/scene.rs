//! Flattens a [`DrawList`] into resolved physical-pixel primitives. Both the
//! GPU and CPU back ends execute the same primitive list, which keeps them in
//! agreement (the WGSL shader and `cpu.rs` implement identical formulas).

use alloy_ui_core::{Color, DrawCommand, DrawList, Rectangle};
use cosmic_text::CacheKey;

use crate::text::TextSystem;

/// Integer pixel rectangle `[x0, y0, x1, y1)`.
pub(crate) type IRect = [i32; 4];

/// What a primitive draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Soft shadow (kind id 0 in the shader).
    Shadow = 0,
    /// Rounded rect with optional border (1).
    Quad = 1,
    /// Glyph coverage mask tinted by colour (2).
    Mask = 2,
    /// Premultiplied texture (image or colour glyph) (3).
    Texture = 3,
    /// Rounded rect filled with a linear gradient, optional border (4).
    Gradient = 4,
    /// Rounded rect with a radial gradient; `dir` is the centre, `blur` the radius (5).
    Radial = 5,
    /// Rounded rect with one highlight band; `blur` is the band position, `aux` its half width (6).
    Sheen = 6,
    /// Shadow inside a rounded rect; `dir` is the offset (7).
    Inner = 7,
    /// Film grain; `aux` is the seed (8).
    Grain = 8,
    /// Blur of what is already drawn under the rect, then a tint (9). Needs the
    /// framebuffer, so back ends handle it specially.
    Backdrop = 9,
}

/// Where a primitive's texels come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    None,
    Glyph(CacheKey),
    Image(u64),
}

/// One resolved primitive in physical pixels.
#[derive(Debug, Clone)]
pub(crate) struct Prim {
    pub kind: Kind,
    /// Shape rect `[x, y, w, h]`.
    pub rect: [f32; 4],
    /// Extra pixels beyond `rect` that the primitive may touch (AA / blur).
    pub pad: f32,
    pub radius: f32,
    pub border_w: f32,
    /// Straight RGBA.
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    /// Gradient end colour (straight RGBA); `color` is the start.
    pub color2: [f32; 4],
    /// Unit gradient direction in physical pixels (y down); gradients only.
    pub dir: [f32; 2],
    pub blur: f32,
    /// Kind-specific extra (sheen half width, grain seed).
    pub aux: f32,
    pub source: Source,
    pub clip: IRect,
}

fn rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

fn clip_of(r: Rectangle, s: f32) -> IRect {
    let x0 = (r.origin.x * s).round() as i32;
    let y0 = (r.origin.y * s).round() as i32;
    let x1 = ((r.origin.x + r.size.width) * s).round() as i32;
    let y1 = ((r.origin.y + r.size.height) * s).round() as i32;
    [x0, y0, x1.max(x0), y1.max(y0)]
}

fn intersect(a: IRect, b: IRect) -> IRect {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    [x0, y0, a[2].min(b[2]).max(x0), a[3].min(b[3]).max(y0)]
}

/// Emit the shadow (if any) and body of a quad or gradient quad.
#[allow(clippy::too_many_arguments)]
fn push_box(
    out: &mut Vec<Prim>,
    bounds: Rectangle,
    fill: Color,
    gradient: Option<(Color, f32)>,
    radius: f32,
    border: &Option<alloy_ui_core::Border>,
    shadow: &Option<alloy_ui_core::Shadow>,
    scale: f32,
    clip: IRect,
) {
    let rect = [
        bounds.origin.x * scale,
        bounds.origin.y * scale,
        bounds.size.width * scale,
        bounds.size.height * scale,
    ];
    let radius = radius.max(0.0) * scale;
    if let Some(s) = shadow {
        let blur = s.blur.max(0.0) * scale;
        out.push(Prim {
            kind: Kind::Shadow,
            rect: [
                rect[0] + s.offset.x * scale,
                rect[1] + s.offset.y * scale,
                rect[2],
                rect[3],
            ],
            pad: blur * 0.5 + 1.0,
            radius,
            border_w: 0.0,
            color: rgba(s.color),
            border_color: [0.0; 4],
            color2: [0.0; 4],
            dir: [0.0; 2],
            blur,
            source: Source::None,
            aux: 0.0,
            clip,
        });
    }
    let (bw, bc) = match border {
        Some(b) if b.width > 0.0 => (b.width * scale, rgba(b.color)),
        _ => (0.0, rgba(fill)),
    };
    let (kind, color2, dir) = match gradient {
        Some((to, deg)) => {
            let a = deg.to_radians();
            (Kind::Gradient, rgba(to), [a.sin(), a.cos()])
        }
        None => (Kind::Quad, [0.0; 4], [0.0; 2]),
    };
    out.push(Prim {
        kind,
        rect,
        pad: 1.0,
        radius,
        border_w: bw,
        color: rgba(fill),
        border_color: bc,
        color2,
        dir,
        blur: 0.0,
        source: Source::None,
        aux: 0.0,
        clip,
    });
}

/// A primitive with every field defaulted except the ones given.
fn blank(kind: Kind, rect: [f32; 4], clip: IRect) -> Prim {
    Prim {
        kind,
        rect,
        pad: 1.0,
        radius: 0.0,
        border_w: 0.0,
        color: [0.0; 4],
        border_color: [0.0; 4],
        color2: [0.0; 4],
        dir: [0.0; 2],
        blur: 0.0,
        aux: 0.0,
        source: Source::None,
        clip,
    }
}

fn phys(b: Rectangle, s: f32) -> [f32; 4] {
    [
        b.origin.x * s,
        b.origin.y * s,
        b.size.width * s,
        b.size.height * s,
    ]
}

/// Flatten `list` for a `width`x`height` target at `scale`.
pub(crate) fn flatten(
    list: &DrawList,
    scale: f32,
    width: u32,
    height: u32,
    text: &mut TextSystem,
) -> Vec<Prim> {
    let full = [0, 0, width as i32, height as i32];
    let mut stack = vec![full];
    let mut out = Vec::new();
    for cmd in &list.0 {
        let clip = *stack.last().unwrap_or(&full);
        match cmd {
            DrawCommand::PushClip(r) => stack.push(intersect(clip, clip_of(*r, scale))),
            DrawCommand::PopClip => {
                if stack.len() > 1 {
                    stack.pop();
                } else {
                    log::warn!("PopClip without matching PushClip");
                }
            }
            DrawCommand::Quad {
                bounds,
                fill,
                radius,
                border,
                shadow,
            } => push_box(
                &mut out, *bounds, *fill, None, *radius, border, shadow, scale, clip,
            ),
            DrawCommand::GradientQuad {
                bounds,
                from,
                to,
                angle_deg,
                radius,
                border,
                shadow,
            } => push_box(
                &mut out,
                *bounds,
                *from,
                Some((*to, *angle_deg)),
                *radius,
                border,
                shadow,
                scale,
                clip,
            ),
            DrawCommand::RadialGradient {
                bounds,
                center,
                gradient_radius,
                from,
                to,
                radius,
            } => out.push(Prim {
                radius: radius.max(0.0) * scale,
                color: rgba(*from),
                color2: rgba(*to),
                dir: [center.x * scale, center.y * scale],
                blur: gradient_radius.max(0.01) * scale,
                ..blank(Kind::Radial, phys(*bounds, scale), clip)
            }),
            DrawCommand::Sheen {
                bounds,
                base,
                highlight,
                angle_deg,
                position,
                half_width,
                radius,
            } => {
                let a = angle_deg.to_radians();
                out.push(Prim {
                    radius: radius.max(0.0) * scale,
                    color: rgba(*base),
                    color2: rgba(*highlight),
                    dir: [a.sin(), a.cos()],
                    blur: *position,
                    aux: half_width.max(0.001),
                    ..blank(Kind::Sheen, phys(*bounds, scale), clip)
                });
            }
            DrawCommand::InnerShadow {
                bounds,
                radius,
                color,
                offset,
                blur,
            } => out.push(Prim {
                radius: radius.max(0.0) * scale,
                color: rgba(*color),
                dir: [offset.x * scale, offset.y * scale],
                blur: blur.max(0.0) * scale,
                ..blank(Kind::Inner, phys(*bounds, scale), clip)
            }),
            DrawCommand::Grain {
                bounds,
                radius,
                amount,
                seed,
            } => out.push(Prim {
                radius: radius.max(0.0) * scale,
                color: [1.0, 1.0, 1.0, amount.clamp(0.0, 1.0)],
                aux: (*seed & 0x00FF_FFFF) as f32,
                ..blank(Kind::Grain, phys(*bounds, scale), clip)
            }),
            DrawCommand::BackdropBlur {
                bounds,
                radius,
                blur,
                tint,
            } => out.push(Prim {
                radius: radius.max(0.0) * scale,
                color: rgba(*tint),
                blur: (blur.max(0.0) * scale).min(40.0),
                ..blank(Kind::Backdrop, phys(*bounds, scale), clip)
            }),
            DrawCommand::Image { bounds, id } => out.push(Prim {
                kind: Kind::Texture,
                rect: [
                    bounds.origin.x * scale,
                    bounds.origin.y * scale,
                    bounds.size.width * scale,
                    bounds.size.height * scale,
                ],
                pad: 1.0,
                radius: 0.0,
                border_w: 0.0,
                color: [1.0; 4],
                border_color: [0.0; 4],
                blur: 0.0,
                color2: [0.0; 4],
                dir: [0.0; 2],
                source: Source::Image(*id),
                aux: 0.0,
                clip,
            }),
            DrawCommand::Line {
                from,
                to,
                width: stroke,
                color,
            } => {
                // A stroke is a chain of round dots spaced under half a pixel apart.
                let (dx, dy) = ((to.x - from.x) * scale, (to.y - from.y) * scale);
                let len = (dx * dx + dy * dy).sqrt();
                let steps = (len * 2.0).ceil().max(1.0) as u32;
                let d = (stroke * scale).max(1.0);
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    out.push(Prim {
                        kind: Kind::Quad,
                        rect: [
                            from.x * scale + dx * t - d / 2.0,
                            from.y * scale + dy * t - d / 2.0,
                            d,
                            d,
                        ],
                        pad: 1.0,
                        radius: d / 2.0,
                        border_w: 0.0,
                        color: rgba(*color),
                        border_color: rgba(*color),
                        blur: 0.0,
                        color2: [0.0; 4],
                        dir: [0.0; 2],
                        source: Source::None,
                        aux: 0.0,
                        clip,
                    });
                }
            }
            DrawCommand::Text {
                weight,
                family,
                line_height,
                origin,
                content,
                size,
                color,
                max_width,
            } => {
                let glyphs = text.layout(
                    content,
                    size * scale,
                    *weight,
                    *family,
                    line_height.map(|l| l * scale),
                    max_width.map(|w| w * scale),
                    (origin.x * scale, origin.y * scale),
                );
                for g in glyphs {
                    let Some(bm) = text.bitmap(g.key) else {
                        continue;
                    };
                    out.push(Prim {
                        kind: if bm.color { Kind::Texture } else { Kind::Mask },
                        rect: [
                            (g.x + bm.left) as f32,
                            (g.y - bm.top) as f32,
                            bm.width as f32,
                            bm.height as f32,
                        ],
                        pad: 0.0,
                        radius: 0.0,
                        border_w: 0.0,
                        color: if bm.color { [1.0; 4] } else { rgba(*color) },
                        border_color: [0.0; 4],
                        blur: 0.0,
                        color2: [0.0; 4],
                        dir: [0.0; 2],
                        source: Source::Glyph(g.key),
                        aux: 0.0,
                        clip,
                    });
                }
            }
        }
    }
    out.retain(|p| p.clip[2] > p.clip[0] && p.clip[3] > p.clip[1]);
    out
}
