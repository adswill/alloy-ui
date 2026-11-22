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
    /// Rounded rect with a radial gradient; `dir` is the centre, `blur` the radius (5).
    Radial = 6,
    /// Rounded rect with one highlight band; `blur` is the band position, `aux` its half width (6).
    Sheen = 7,
    /// Shadow inside a rounded rect; `dir` is the offset (7).
    Inner = 8,
    /// Film grain; `aux` is the seed (8).
    Grain = 9,
    /// Blur of what is already drawn under the rect, then a tint (9). Needs the
    /// framebuffer, so back ends handle it specially.
    Backdrop = 10,
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

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

fn in_tri(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn in_tri_strict(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    (d1 > 1e-7 && d2 > 1e-7 && d3 > 1e-7) || (d1 < -1e-7 && d2 < -1e-7 && d3 < -1e-7)
}

/// Ear-clip a simple polygon into triangles `([i, j, k], flags)` over `pts`
/// (counter-clockwise in y-up terms, i.e. positive `cross`). Flag bit 0/1/2
/// marks edge ij/jk/ki as lying on the polygon outline rather than on an
/// inner diagonal.
pub(crate) fn triangulate(pts: &[[f32; 2]]) -> Vec<([usize; 3], u8)> {
    triangulate_with(pts, &[], false)
}

/// [`triangulate`] where `soft[i]` marks the edge `i -> i + 1` as an inner cut rather than
/// outline (the polygon must have positive area then, as the fill module's output does).
pub(crate) fn triangulate_with(
    pts: &[[f32; 2]],
    soft: &[bool],
    robust: bool,
) -> Vec<([usize; 3], u8)> {
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
    let mut outline: Vec<bool> = (0..n)
        .map(|i| !soft.get(i).copied().unwrap_or(false))
        .collect();
    if area < 0.0 {
        outline.reverse();
    }
    while idx.len() > 3 {
        let m = idx.len();
        let blocked = |i: usize| {
            let (ip, inx) = ((i + m - 1) % m, (i + 1) % m);
            let (a, b, c) = (pts[idx[ip]], pts[idx[i]], pts[idx[inx]]);
            idx.iter().enumerate().any(|(k, &v)| {
                let p = pts[v];
                k != ip
                    && k != i
                    && k != inx
                    && p != a
                    && p != b
                    && p != c
                    && if robust {
                        in_tri_strict(p, a, b, c)
                    } else {
                        in_tri(p, a, b, c)
                    }
            })
        };
        let convex = |i: usize| {
            let (ip, inx) = ((i + m - 1) % m, (i + 1) % m);
            cross(pts[idx[ip]], pts[idx[i]], pts[idx[inx]])
        };
        let mut ear = (0..m).find(|&i| convex(i) > 1e-6 && !blocked(i));
        if ear.is_none() && robust {
            // Nearly collinear runs can leave no clean ear: take the sharpest convex corner.
            ear = (0..m)
                .filter(|&i| convex(i) > 1e-9)
                .max_by(|&i, &j| convex(i).total_cmp(&convex(j)));
        }
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

fn tri_prim(a: [f32; 2], b: [f32; 2], c: [f32; 2], flags: u8, color: Color, clip: IRect) -> Prim {
    let (x0, x1) = (a[0].min(b[0]).min(c[0]), a[0].max(b[0]).max(c[0]));
    let (y0, y1) = (a[1].min(b[1]).min(c[1]), a[1].max(b[1]).max(c[1]));
    Prim {
        kind: Kind::Tri,
        rect: [x0, y0, x1 - x0, y1 - y0],
        pad: 2.0,
        radius: f32::from(flags),
        border_w: 0.0,
        color: rgba(color),
        border_color: [a[0], a[1], b[0], b[1]],
        color2: [c[0], c[1], 0.0, 0.0],
        dir: [0.0; 2],
        blur: 0.0,
        aux: 0.0,
        source: Source::None,
        clip,
    }
}

/// Where the edge `p -> q` crosses `t == bound`. The endpoints are ordered first so the two
/// triangles (and two bands) that share the edge compute bit-identical points.
fn crossing(p: [f32; 2], q: [f32; 2], t: &dyn Fn([f32; 2]) -> f32, bound: f32) -> [f32; 2] {
    let (p, q) = if (p[0], p[1]) <= (q[0], q[1]) {
        (p, q)
    } else {
        (q, p)
    };
    let (tp, tq) = (t(p), t(q));
    let k = ((bound - tp) / (tq - tp)).clamp(0.0, 1.0);
    [p[0] + (q[0] - p[0]) * k, p[1] + (q[1] - p[1]) * k]
}

/// The part of triangle `v` with `lo <= t <= hi`, as a convex polygon of `(point, flag of
/// the edge to the next point)`. Edges along the cut are inner (`false`).
fn band_polygon(
    v: &[[f32; 2]; 3],
    flags: [bool; 3],
    t: &dyn Fn([f32; 2]) -> f32,
    lo: f32,
    hi: f32,
) -> Vec<([f32; 2], bool)> {
    let mut out: Vec<([f32; 2], bool)> = Vec::new();
    let mut push = |p: [f32; 2], f: bool| {
        if let Some(last) = out.last_mut() {
            if last.0 == p {
                last.1 = f;
                return;
            }
        }
        out.push((p, f));
    };
    for i in 0..3 {
        let (p, q) = (v[i], v[(i + 1) % 3]);
        let (tp, tq) = (t(p), t(q));
        let (emin, emax) = (tp.min(tq), tp.max(tq));
        if emax < lo || emin > hi {
            continue;
        }
        // The entry and exit of the edge's walk through the band.
        let at = |tv: f32, end: [f32; 2]| {
            if tv < lo {
                crossing(p, q, t, lo)
            } else if tv > hi {
                crossing(p, q, t, hi)
            } else {
                end
            }
        };
        let (s, e) = (at(tp, p), at(tq, q));
        push(s, flags[i]);
        push(e, false);
    }
    if out.len() > 1 && out[0].0 == out[out.len() - 1].0 {
        out.pop(); // the walk closed on its start; the first point keeps its edge flag
    }
    out
}

/// Lower a filled path: resolve the fill rule, bridge holes, triangulate, and for a gradient
/// slice the triangles into thin bands whose shared cuts are inner edges (no seams).
fn lower_path(
    out: &mut Vec<Prim>,
    contours: &[Vec<alloy_ui_core::Point>],
    rule: alloy_ui_core::FillRule,
    paint: &alloy_ui_core::PathPaint,
    scale: f32,
    clip: IRect,
) {
    use alloy_ui_core::PathPaint;
    let dev: Vec<Vec<[f32; 2]>> = contours
        .iter()
        .map(|c| c.iter().map(|p| [p.x * scale, p.y * scale]).collect())
        .collect();
    // Triangles over their own vertex arrays, with outline flags.
    let mut tris: Vec<([[f32; 2]; 3], u8)> = Vec::new();
    let mut add = |pts: &[[f32; 2]], soft: &[bool]| {
        for ([i, j, k], f) in triangulate_with(pts, soft, true) {
            tris.push(([pts[i], pts[j], pts[k]], f));
        }
    };
    match crate::fill::resolve(&dev, rule) {
        Some(loops) => {
            for b in crate::fill::bridged(loops) {
                add(&b.points, &b.soft);
            }
        }
        None => {
            for c in &dev {
                add(c, &[]);
            }
        }
    }
    match paint {
        PathPaint::Solid(color) => {
            for (t, f) in tris {
                out.push(tri_prim(t[0], t[1], t[2], f, *color, clip));
            }
        }
        PathPaint::Linear { from, to, stops } => {
            let (a, b) = (
                [from.x * scale, from.y * scale],
                [to.x * scale, to.y * scale],
            );
            let d = [b[0] - a[0], b[1] - a[1]];
            let len2 = (d[0] * d[0] + d[1] * d[1]).max(1e-6);
            let t = |p: [f32; 2]| ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2;
            let bands = ((len2.sqrt() / 1.5).ceil() as usize).clamp(1, 192);
            let step = 1.0 / bands as f32;
            for (tri, flags) in tris {
                let fl = [flags & 1 != 0, flags & 2 != 0, flags & 4 != 0];
                let ts = [t(tri[0]), t(tri[1]), t(tri[2])];
                let (lo, hi) = (
                    ts.iter().copied().fold(f32::MAX, f32::min),
                    ts.iter().copied().fold(f32::MIN, f32::max),
                );
                let k0 = ((lo / step).floor() as i64).clamp(0, bands as i64 - 1) as usize;
                let k1 = ((hi / step).floor() as i64).clamp(0, bands as i64 - 1) as usize;
                for k in k0..=k1 {
                    let blo = if k == 0 { f32::MIN } else { k as f32 * step };
                    let bhi = if k + 1 == bands {
                        f32::MAX
                    } else {
                        (k + 1) as f32 * step
                    };
                    let piece = band_polygon(&tri, fl, &t, blo, bhi);
                    if piece.len() < 3 {
                        continue;
                    }
                    let color = alloy_ui_core::PathPaint::at(stops, (k as f32 + 0.5) * step);
                    let m = piece.len();
                    for j in 1..m - 1 {
                        let f = u8::from(j == 1 && piece[0].1)
                            | u8::from(piece[j].1) << 1
                            | u8::from(j + 1 == m - 1 && piece[m - 1].1) << 2;
                        out.push(tri_prim(
                            piece[0].0,
                            piece[j].0,
                            piece[j + 1].0,
                            f,
                            color,
                            clip,
                        ));
                    }
                }
            }
        }
    }
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
                        aux: 0.0,
                        source: Source::None,
                        clip,
                    });
                }
            }
            DrawCommand::PolygonPath {
                contours,
                rule,
                paint,
            } => lower_path(&mut out, contours, *rule, paint, scale, clip),
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
            DrawCommand::Image { bounds, id } | DrawCommand::ImageAlpha { bounds, id, .. } => out
                .push(Prim {
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
                    color: [
                        1.0,
                        1.0,
                        1.0,
                        match cmd {
                            DrawCommand::ImageAlpha { alpha, .. } => alpha.clamp(0.0, 1.0),
                            _ => 1.0,
                        },
                    ],
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

#[cfg(test)]
mod path_tests {
    use super::*;
    use alloy_ui_core::FillRule;

    fn tri_area(p: &[[f32; 2]; 3]) -> f32 {
        cross(p[0], p[1], p[2]).abs() / 2.0
    }

    /// The triangles must tile the resolved region exactly: same total area, no fallback fan.
    fn tiles(contours: Vec<Vec<[f32; 2]>>, rule: FillRule, expect: f64) {
        let loops = crate::fill::resolve(&contours, rule).unwrap();
        let mut sum = 0.0f64;
        for b in crate::fill::bridged(loops) {
            for ([i, j, k], _) in triangulate_with(&b.points, &b.soft, true) {
                sum += f64::from(tri_area(&[b.points[i], b.points[j], b.points[k]]));
            }
        }
        assert!(
            (sum - expect).abs() < expect * 1e-3,
            "triangles cover {sum}, want {expect}"
        );
    }

    fn star(cx: f32, cy: f32, r: f32) -> Vec<[f32; 2]> {
        (0..5)
            .map(|i| {
                let a = (i as f32 * 144.0 - 90.0).to_radians();
                [cx + r * a.cos(), cy + r * a.sin()]
            })
            .collect()
    }

    #[test]
    fn star_triangles_tile_the_region() {
        let nz = crate::fill::resolve(&[star(100.0, 100.0, 80.0)], FillRule::NonZero).unwrap();
        let area: f64 = nz.iter().map(|l| crate::fill::area(l)).sum();
        tiles(vec![star(100.0, 100.0, 80.0)], FillRule::NonZero, area);
        let eo = crate::fill::resolve(&[star(100.0, 100.0, 80.0)], FillRule::EvenOdd).unwrap();
        let area: f64 = eo.iter().map(|l| crate::fill::area(l)).sum();
        tiles(vec![star(100.0, 100.0, 80.0)], FillRule::EvenOdd, area);
    }
}
