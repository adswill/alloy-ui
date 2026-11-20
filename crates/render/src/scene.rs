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
    pub blur: f32,
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
            } => {
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
                        blur,
                        source: Source::None,
                        clip,
                    });
                }
                let (bw, bc) = match border {
                    Some(b) if b.width > 0.0 => (b.width * scale, rgba(b.color)),
                    _ => (0.0, rgba(*fill)),
                };
                out.push(Prim {
                    kind: Kind::Quad,
                    rect,
                    pad: 1.0,
                    radius,
                    border_w: bw,
                    color: rgba(*fill),
                    border_color: bc,
                    blur: 0.0,
                    source: Source::None,
                    clip,
                });
            }
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
                source: Source::Image(*id),
                clip,
            }),
            DrawCommand::Text {
                origin,
                content,
                size,
                color,
                max_width,
            } => {
                let glyphs = text.layout(
                    content,
                    size * scale,
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
                        source: Source::Glyph(g.key),
                        clip,
                    });
                }
            }
        }
    }
    out.retain(|p| p.clip[2] > p.clip[0] && p.clip[3] > p.clip[1]);
    out
}
