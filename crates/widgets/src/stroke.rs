//! Turns a polyline and a [`Stroke`] style into outline pieces (segments, joins, caps) that
//! the renderer unions under the non-zero rule into one watertight shape.

use alloy_ui_core::Point;

/// How an open stroke ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Ends square at the endpoint.
    #[default]
    Butt,
    /// Ends in a half circle.
    Round,
    /// Ends square, half the width past the endpoint.
    Square,
}

/// How two segments meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// A sharp corner, cut to a bevel past the miter limit.
    #[default]
    Miter,
    /// A rounded corner.
    Round,
    /// A flat cut across the corner.
    Bevel,
}

/// A stroke style: width, caps, joins and an optional dash pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// Line width in pixels.
    pub width: f32,
    /// Cap at the ends of open sub-paths.
    pub cap: LineCap,
    /// Join between segments.
    pub join: LineJoin,
    /// Longest miter, as a multiple of the width, before a miter join becomes a bevel.
    pub miter_limit: f32,
    /// Alternating dash and gap lengths; empty is a solid line.
    pub dash: Vec<f32>,
    /// How far into the dash pattern the line starts.
    pub dash_offset: f32,
}

impl Stroke {
    /// A solid stroke of `width` with butt caps and mitered joins.
    pub fn new(width: f32) -> Self {
        Self {
            width,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
            dash: Vec::new(),
            dash_offset: 0.0,
        }
    }
    /// Set the cap.
    pub fn cap(mut self, c: LineCap) -> Self {
        self.cap = c;
        self
    }
    /// Set the join.
    pub fn join(mut self, j: LineJoin) -> Self {
        self.join = j;
        self
    }
    /// Set the miter limit.
    pub fn miter_limit(mut self, m: f32) -> Self {
        self.miter_limit = m.max(1.0);
        self
    }
    /// Dash with alternating `on`/`off` lengths (repeated), starting `offset` into the pattern.
    pub fn dashed(mut self, pattern: &[f32], offset: f32) -> Self {
        self.dash = pattern.iter().map(|d| d.max(0.0)).collect();
        self.dash_offset = offset;
        self
    }
}

type V = (f32, f32);

fn sub(a: Point, b: Point) -> V {
    (a.x - b.x, a.y - b.y)
}
fn pt(p: Point, v: V, k: f32) -> Point {
    Point::new(p.x + v.0 * k, p.y + v.1 * k)
}
fn unit(v: V) -> V {
    let l = v.0.hypot(v.1).max(1e-9);
    (v.0 / l, v.1 / l)
}
fn left(d: V) -> V {
    (-d.1, d.0)
}

fn area(p: &[Point]) -> f32 {
    let n = p.len();
    (0..n)
        .map(|i| p[i].x * p[(i + 1) % n].y - p[(i + 1) % n].x * p[i].y)
        .sum()
}

fn arc(c: Point, r: f32, from: f32, sweep: f32, out: &mut Vec<Point>) {
    let step = 2.0 * (1.0 - 0.2 / r.max(0.5)).clamp(-1.0, 1.0).acos();
    let n = ((sweep.abs() / step.max(0.05)).ceil() as usize).clamp(2, 180);
    for i in 0..=n {
        let a = from + sweep * i as f32 / n as f32;
        out.push(Point::new(c.x + r * a.cos(), c.y + r * a.sin()));
    }
}

/// Split polylines into dashes. A closed polyline is walked once around, closing segment included.
fn dashes(lines: &[(Vec<Point>, bool)], pattern: &[f32], offset: f32) -> Vec<Vec<Point>> {
    let total: f32 = pattern.iter().sum();
    let mut out = Vec::new();
    for (pts, closed) in lines {
        let mut p = pts.clone();
        if *closed {
            if let Some(f) = p.first().copied() {
                p.push(f);
            }
        }
        // Where in the pattern we start.
        let mut idx = 0usize;
        let mut left_in = {
            let mut o = offset.rem_euclid(total.max(1e-3));
            while o >= pattern[idx % pattern.len()] && pattern[idx % pattern.len()] >= 0.0 {
                o -= pattern[idx % pattern.len()];
                idx += 1;
                if idx > 10_000 {
                    break;
                }
            }
            pattern[idx % pattern.len()] - o
        };
        let mut cur: Vec<Point> = Vec::new();
        if idx % 2 == 0 {
            cur.push(p[0]);
        }
        for w in p.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = (b.x - a.x).hypot(b.y - a.y);
            let d = unit(sub(b, a));
            let mut pos = 0.0;
            while len - pos > left_in {
                pos += left_in;
                let at = pt(a, d, pos);
                if idx % 2 == 0 {
                    cur.push(at);
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur = vec![at];
                }
                idx += 1;
                left_in = pattern[idx % pattern.len()].max(1e-3);
            }
            left_in -= len - pos;
            if idx % 2 == 0 {
                cur.push(b);
            }
        }
        if cur.len() > 1 {
            out.push(cur);
        }
    }
    out
}

/// One side of the stroke: the offset of `p` to the left of travel, with a join at every
/// vertex. On the inner side of a turn the path pivots through the vertex itself so the
/// winding stays positive and a non-zero fill needs no cleanup.
fn side(p: &[Point], closed: bool, hw: f32, style: &Stroke) -> Vec<Point> {
    let n = p.len();
    let dir = |i: usize| unit(sub(p[(i + 1) % n], p[i]));
    let mut out = Vec::with_capacity(n * 2);
    if !closed {
        out.push(pt(p[0], left(dir(0)), hw));
    }
    let verts: Vec<usize> = if closed {
        (0..n).collect()
    } else {
        (1..n.saturating_sub(1)).collect()
    };
    for i in verts {
        let v = p[i];
        let (d0, d1) = (dir((i + n - 1) % n), dir(i));
        let (n0, n1) = (left(d0), left(d1));
        let (a, b) = (pt(v, n0, hw), pt(v, n1, hw));
        let cr = d0.0 * d1.1 - d0.1 * d1.0;
        let dot = d0.0 * d1.0 + d0.1 * d1.1;
        if cr.abs() < 1e-6 && dot > 0.0 {
            out.push(a);
        } else if cr > 0.0 {
            out.extend([a, v, b]); // inner side of the turn
        } else {
            match style.join {
                LineJoin::Bevel => out.extend([a, b]),
                LineJoin::Miter => {
                    let k = 1.0 + (n0.0 * n1.0 + n0.1 * n1.1);
                    let m = ((n0.0 + n1.0) / k.max(1e-6), (n0.1 + n1.1) / k.max(1e-6));
                    if k > 1e-3 && m.0.hypot(m.1) <= style.miter_limit {
                        out.extend([a, pt(v, m, hw), b]);
                    } else {
                        out.extend([a, b]);
                    }
                }
                LineJoin::Round => {
                    let a0 = (a.y - v.y).atan2(a.x - v.x);
                    let mut sweep = (b.y - v.y).atan2(b.x - v.x) - a0;
                    while sweep > std::f32::consts::PI {
                        sweep -= std::f32::consts::TAU;
                    }
                    while sweep < -std::f32::consts::PI {
                        sweep += std::f32::consts::TAU;
                    }
                    arc(v, hw, a0, sweep, &mut out);
                }
            }
        }
    }
    if !closed {
        out.push(pt(p[n - 1], left(dir(n - 2)), hw));
    }
    out
}

/// The cap at `end`, travelling in direction `d`: from the left offset round to the right.
fn cap(end: Point, d: V, hw: f32, kind: LineCap, out: &mut Vec<Point>) {
    match kind {
        LineCap::Butt => {}
        LineCap::Square => {
            let l = left(d);
            out.push(pt(pt(end, l, hw), d, hw));
            out.push(pt(pt(end, l, -hw), d, hw));
        }
        LineCap::Round => {
            let l = left(d);
            let a0 = l.1.atan2(l.0);
            arc(end, hw, a0, -std::f32::consts::PI, out);
        }
    }
}

/// Outline loops for stroking `lines` (`(points, closed)`): one loop per open sub-path and
/// two (outer, inner) per closed one. All outer loops have positive area, inner ones negative,
/// so a non-zero fill gives the stroke exactly.
pub fn pieces(lines: &[(Vec<Point>, bool)], style: &Stroke) -> Vec<Vec<Point>> {
    let hw = (style.width / 2.0).max(0.05);
    let dashed: Vec<(Vec<Point>, bool)>;
    let lines: &[(Vec<Point>, bool)] =
        if !style.dash.is_empty() && style.dash.iter().sum::<f32>() > 0.01 {
            dashed = dashes(lines, &style.dash, style.dash_offset)
                .into_iter()
                .map(|d| (d, false))
                .collect();
            &dashed
        } else {
            lines
        };
    let mut out: Vec<Vec<Point>> = Vec::new();
    for (raw, closed) in lines {
        let mut p: Vec<Point> = Vec::with_capacity(raw.len());
        for q in raw {
            if p.last().is_none_or(|l| (l.x - q.x).hypot(l.y - q.y) > 1e-3) {
                p.push(*q);
            }
        }
        let mut closed = *closed;
        if closed && p.len() > 1 {
            let (f, l) = (p[0], p[p.len() - 1]);
            if (f.x - l.x).hypot(f.y - l.y) <= 1e-3 {
                p.pop();
            }
        }
        if p.len() == 1 {
            // A dot: only round and square caps draw anything.
            match style.cap {
                LineCap::Round => {
                    let mut c = Vec::new();
                    arc(p[0], hw, 0.0, std::f32::consts::TAU, &mut c);
                    out.push(c);
                }
                LineCap::Square => {
                    let q = p[0];
                    out.push(vec![
                        Point::new(q.x - hw, q.y - hw),
                        Point::new(q.x + hw, q.y - hw),
                        Point::new(q.x + hw, q.y + hw),
                        Point::new(q.x - hw, q.y + hw),
                    ]);
                }
                LineCap::Butt => {}
            }
            continue;
        }
        if p.len() < 2 {
            continue;
        }
        if p.len() == 2 {
            closed = false;
        }
        let n = p.len();
        let rev: Vec<Point> = p.iter().rev().copied().collect();
        if closed {
            // The larger loop is the outer one: positive; the inner one cuts it out.
            let (mut a, mut b) = (side(&p, true, hw, style), side(&rev, true, hw, style));
            if area(&a).abs() < area(&b).abs() {
                std::mem::swap(&mut a, &mut b);
            }
            if area(&a) < 0.0 {
                a.reverse();
            }
            if area(&b) > 0.0 {
                b.reverse();
            }
            out.push(a);
            out.push(b);
        } else {
            let mut o = side(&p, false, hw, style);
            let d_end = unit(sub(p[n - 1], p[n - 2]));
            cap(p[n - 1], d_end, hw, style.cap, &mut o);
            o.extend(side(&rev, false, hw, style));
            let d_start = unit(sub(p[0], p[1]));
            cap(p[0], d_start, hw, style.cap, &mut o);
            if area(&o) < 0.0 {
                o.reverse();
            }
            out.push(o);
        }
    }
    out.retain(|p| p.len() >= 3 && area(p).abs() > 1e-6);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn butt_and_square_caps_differ_by_the_extension() {
        let line = [(vec![p(0.0, 0.0), p(10.0, 0.0)], false)];
        let butt = pieces(&line, &Stroke::new(4.0));
        let sq = pieces(&line, &Stroke::new(4.0).cap(LineCap::Square));
        let a = |v: &Vec<Vec<Point>>| v.iter().map(|x| area(x) / 2.0).sum::<f32>();
        assert!((a(&butt) - 40.0).abs() < 1e-3);
        assert!((a(&sq) - 56.0).abs() < 1e-3);
    }

    #[test]
    fn miter_limit_turns_sharp_corners_into_bevels() {
        let zig = [(vec![p(0.0, 0.0), p(10.0, 0.0), p(0.5, 1.0)], false)];
        let long = pieces(&zig, &Stroke::new(2.0).miter_limit(100.0));
        let cut = pieces(&zig, &Stroke::new(2.0).miter_limit(1.5));
        let far = |v: &Vec<Vec<Point>>| v.iter().flatten().map(|q| q.x).fold(f32::MIN, f32::max);
        assert!(far(&long) > far(&cut) + 1.0, "{} {}", far(&long), far(&cut));
    }

    #[test]
    fn dashes_split_a_line_by_the_pattern() {
        let line = [(vec![p(0.0, 0.0), p(100.0, 0.0)], false)];
        let d = dashes(&line, &[10.0, 10.0], 0.0);
        assert_eq!(d.len(), 5);
        assert!((d[1][0].x - 20.0).abs() < 1e-3 && (d[1][1].x - 30.0).abs() < 1e-3);
    }
}
