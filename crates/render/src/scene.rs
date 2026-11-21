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
    /// Anti-aliased triangle (5). Vertices: `rect` holds the bounding box,
    /// `border_color` = `[x0, y0, x1, y1]`, `color2` = `[x2, y2, _, _]`;
    /// `radius` carries the edge flags (bit i set = edge i is a shape
    /// boundary and is anti-aliased; clear = interior seam, bled by 1 px).
    Tri = 5,
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
        clip,
    });
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn in_tri(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// Ear-clip a simple polygon into triangles `([i, j, k], flags)` over `pts`
/// (counter-clockwise in y-up terms, i.e. positive `cross`). Flag bit 0/1/2
/// marks edge ij/jk/ki as lying on the polygon outline rather than on an
/// inner diagonal.
pub(crate) fn triangulate(pts: &[[f32; 2]]) -> Vec<([usize; 3], u8)> {
    let n = pts.len();
    let mut out = Vec::new();
    if n < 3 {
        return out;
    }
    let area: f32 = (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    let mut idx: Vec<usize> = if area >= 0.0 {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };
    // `outline[i]`: the edge from idx[i] to idx[i + 1] is an original edge.
    let mut outline = vec![true; n];
    while idx.len() > 3 {
        let m = idx.len();
        let ear = (0..m).find(|&i| {
            let (ip, inx) = ((i + m - 1) % m, (i + 1) % m);
            let (a, b, c) = (pts[idx[ip]], pts[idx[i]], pts[idx[inx]]);
            cross(a, b, c) > 1e-6
                && !idx.iter().enumerate().any(|(k, &v)| {
                    let p = pts[v];
                    k != ip
                        && k != i
                        && k != inx
                        && p != a
                        && p != b
                        && p != c
                        && in_tri(p, a, b, c)
                })
        });
        let Some(i) = ear else { break };
        let (ip, inx) = ((i + m - 1) % m, (i + 1) % m);
        let flags = outline[ip] as u8 | (outline[i] as u8) << 1;
        out.push(([idx[ip], idx[i], idx[inx]], flags));
        outline[ip] = false; // the new diagonal ip -> inx
        idx.remove(i);
        outline.remove(i);
    }
    let m = idx.len();
    if m == 3 {
        let flags = outline[0] as u8 | (outline[1] as u8) << 1 | (outline[2] as u8) << 2;
        out.push(([idx[0], idx[1], idx[2]], flags));
    } else {
        // Degenerate or self-intersecting input: fan what is left.
        for k in 1..m.saturating_sub(1) {
            out.push(([idx[0], idx[k], idx[k + 1]], 0b101));
        }
    }
    out
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
            DrawCommand::Polygon { points, fill } => {
                let pts: Vec<[f32; 2]> =
                    points.iter().map(|p| [p.x * scale, p.y * scale]).collect();
                for ([i, j, k], flags) in triangulate(&pts) {
                    let (a, b, c) = (pts[i], pts[j], pts[k]);
                    let (x0, x1) = (a[0].min(b[0]).min(c[0]), a[0].max(b[0]).max(c[0]));
                    let (y0, y1) = (a[1].min(b[1]).min(c[1]), a[1].max(b[1]).max(c[1]));
                    out.push(Prim {
                        kind: Kind::Tri,
                        rect: [x0, y0, x1 - x0, y1 - y0],
                        pad: 2.0,
                        radius: f32::from(flags),
                        border_w: 0.0,
                        color: rgba(*fill),
                        border_color: [a[0], a[1], b[0], b[1]],
                        color2: [c[0], c[1], 0.0, 0.0],
                        dir: [0.0; 2],
                        blur: 0.0,
                        source: Source::None,
                        clip,
                    });
                }
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
                color2: [0.0; 4],
                dir: [0.0; 2],
                source: Source::Image(*id),
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
                        clip,
                    });
                }
            }
        }
    }
    out.retain(|p| p.clip[2] > p.clip[0] && p.clip[3] > p.clip[1]);
    out
}
