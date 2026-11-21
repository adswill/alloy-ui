//! Resolving a set of closed contours under a fill rule into simple outlines.
//!
//! Input contours may overlap, cross themselves and nest. The result is a list of simple
//! loops (outer boundaries have positive shoelace area, holes negative) with the inside of the
//! fill on the left of every edge. [`bridged`] then joins each hole to its enclosing outline
//! by a zero-width cut, giving one weakly simple polygon per filled island that the ear
//! clipper in `scene.rs` can triangulate. The cut edges are reported so the clipper treats
//! them as inner diagonals and does not anti-alias a hairline along them.
//!
//! Vertices snap to 1/256 pixel so edges that meet do so exactly. Work is done in `f64`.

use alloy_ui_core::FillRule;
use std::collections::HashMap;

/// A contour vertex.
pub(crate) type P = [f32; 2];

/// Above this many edges (after splitting) the fill falls back to drawing each contour as it
/// is, because resolving would cost more than a frame.
pub(crate) const MAX_EDGES: usize = 24_000;
const SNAP: f64 = 256.0;

/// A polygon with the flag of each edge `i -> i+1` being a bridge cut.
pub(crate) struct Bridged {
    pub points: Vec<P>,
    pub soft: Vec<bool>,
}

type Pt = (f64, f64);

fn snap(p: Pt) -> (i64, i64) {
    ((p.0 * SNAP).round() as i64, (p.1 * SNAP).round() as i64)
}

fn cross(o: Pt, a: Pt, b: Pt) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

struct Nodes {
    pts: Vec<Pt>,
    index: HashMap<(i64, i64), usize>,
}

impl Nodes {
    fn id(&mut self, p: Pt) -> usize {
        let k = snap(p);
        if let Some(&i) = self.index.get(&k) {
            return i;
        }
        let i = self.pts.len();
        self.pts.push((k.0 as f64 / SNAP, k.1 as f64 / SNAP));
        self.index.insert(k, i);
        i
    }
}

fn inside(rule: FillRule, w: i32) -> bool {
    match rule {
        FillRule::NonZero => w != 0,
        FillRule::EvenOdd => w.rem_euclid(2) == 1,
    }
}

/// Resolve `contours` into simple loops. `None` when the input is too large to resolve.
pub(crate) fn resolve(contours: &[Vec<P>], rule: FillRule) -> Option<Vec<Vec<P>>> {
    let mut nodes = Nodes {
        pts: Vec::new(),
        index: HashMap::new(),
    };
    // Directed edges between snapped nodes.
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for c in contours {
        let n = c.len();
        if n < 3 {
            continue;
        }
        let ids: Vec<usize> = c
            .iter()
            .map(|p| nodes.id((f64::from(p[0]), f64::from(p[1]))))
            .collect();
        for i in 0..n {
            let (a, b) = (ids[i], ids[(i + 1) % n]);
            if a != b {
                edges.push((a, b));
            }
        }
    }
    if edges.len() > MAX_EDGES {
        return None;
    }
    // Split every edge at its intersections with the others (sweep over x).
    let mut cuts: Vec<Vec<(f64, usize)>> = vec![Vec::new(); edges.len()];
    let bbox = |e: &(usize, usize), nodes: &Nodes| {
        let (a, b) = (nodes.pts[e.0], nodes.pts[e.1]);
        (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1))
    };
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by(|&i, &j| bbox(&edges[i], &nodes).0.total_cmp(&bbox(&edges[j], &nodes).0));
    for oi in 0..order.len() {
        let i = order[oi];
        let bi = bbox(&edges[i], &nodes);
        for &j in &order[oi + 1..] {
            let bj = bbox(&edges[j], &nodes);
            if bj.0 > bi.1 + 1e-9 {
                break;
            }
            if bj.2 > bi.3 + 1e-9 || bj.3 < bi.2 - 1e-9 {
                continue;
            }
            let (p1, p2) = (nodes.pts[edges[i].0], nodes.pts[edges[i].1]);
            let (p3, p4) = (nodes.pts[edges[j].0], nodes.pts[edges[j].1]);
            let d1 = (p2.0 - p1.0, p2.1 - p1.1);
            let d2 = (p4.0 - p3.0, p4.1 - p3.1);
            let den = d1.0 * d2.1 - d1.1 * d2.0;
            if den.abs() < 1e-12 {
                // Parallel: only collinear overlap matters, as T-junction cuts.
                if cross(p1, p2, p3).abs() < 1e-9 {
                    for (n, q) in [(edges[j].0, p3), (edges[j].1, p4)] {
                        let t = ((q.0 - p1.0) * d1.0 + (q.1 - p1.1) * d1.1)
                            / (d1.0 * d1.0 + d1.1 * d1.1);
                        if t > 0.0 && t < 1.0 {
                            cuts[i].push((t, n));
                        }
                    }
                    for (n, q) in [(edges[i].0, p1), (edges[i].1, p2)] {
                        let t = ((q.0 - p3.0) * d2.0 + (q.1 - p3.1) * d2.1)
                            / (d2.0 * d2.0 + d2.1 * d2.1);
                        if t > 0.0 && t < 1.0 {
                            cuts[j].push((t, n));
                        }
                    }
                }
                continue;
            }
            let t = ((p3.0 - p1.0) * d2.1 - (p3.1 - p1.1) * d2.0) / den;
            let u = ((p3.0 - p1.0) * d1.1 - (p3.1 - p1.1) * d1.0) / den;
            let eps = 1e-9;
            if t < -eps || t > 1.0 + eps || u < -eps || u > 1.0 + eps {
                continue;
            }
            let x = nodes.id((p1.0 + t * d1.0, p1.1 + t * d1.1));
            if t > 0.0 && t < 1.0 && x != edges[i].0 && x != edges[i].1 {
                cuts[i].push((t, x));
            }
            if u > 0.0 && u < 1.0 && x != edges[j].0 && x != edges[j].1 {
                cuts[j].push((u, x));
            }
        }
    }
    // One contour that crosses nothing is already a simple outline: keep it exactly as given.
    if contours.iter().filter(|c| c.len() >= 3).count() == 1 && cuts.iter().all(|c| c.is_empty())
    {
        let mut c = contours.iter().find(|c| c.len() >= 3).cloned().unwrap_or_default();
        c.dedup();
        if c.len() >= 3 && c.first() == c.last() {
            c.pop();
        }
        if area(&c) < 0.0 {
            c.reverse();
        }
        return Some(if c.len() >= 3 { vec![c] } else { Vec::new() });
    }
    // Sub-edges as undirected pairs with net multiplicity along low -> high node id.
    let mut net: HashMap<(usize, usize), i32> = HashMap::new();
    for (k, &(a, b)) in edges.iter().enumerate() {
        let mut c = std::mem::take(&mut cuts[k]);
        c.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut prev = a;
        for (_, n) in c.into_iter().chain(std::iter::once((1.0, b))) {
            if n != prev {
                let (lo, hi, s) = if prev < n { (prev, n, 1) } else { (n, prev, -1) };
                *net.entry((lo, hi)).or_insert(0) += s;
                prev = n;
            }
        }
    }
    let mut subs: Vec<((usize, usize), i32)> = net.into_iter().filter(|e| e.1 != 0).collect();
    // HashMap order varies per run; sort so output (and so rendering) is deterministic.
    subs.sort_by_key(|e| e.0);
    if subs.is_empty() {
        return Some(Vec::new());
    }
    let e_pts: Vec<(Pt, Pt, i32)> = subs
        .iter()
        .map(|&((a, b), m)| (nodes.pts[a], nodes.pts[b], m))
        .collect();

    // Winding queries by strips so each is cheap.
    let strips = Strips::new(&e_pts);
    let mut boundary: Vec<(usize, usize)> = Vec::new();
    for (k, &((u, v), m)) in subs.iter().enumerate() {
        let (a, b, _) = e_pts[k];
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let (wl, wr);
        if (b.1 - a.1).abs() > 1e-12 {
            let we = strips.east(mid, k);
            if b.1 > a.1 {
                (wr, wl) = (we, we + m);
            } else {
                (wl, wr) = (we, we - m);
            }
        } else {
            let wn = strips.north(mid, k);
            if b.0 > a.0 {
                (wl, wr) = (wn, wn - m);
            } else {
                (wr, wl) = (wn, wn + m);
            }
        }
        let (il, ir) = (inside(rule, wl), inside(rule, wr));
        if il != ir {
            boundary.push(if il { (u, v) } else { (v, u) });
        }
    }
    // Trace faces: at each node turn onto the first edge clockwise from the way back.
    let mut out_edges: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, e) in boundary.iter().enumerate() {
        out_edges.entry(e.0).or_default().push(i);
    }
    let mut used = vec![false; boundary.len()];
    let mut loops = Vec::new();
    for s in 0..boundary.len() {
        if used[s] {
            continue;
        }
        let mut ids = vec![boundary[s].0];
        let mut cur = s;
        loop {
            used[cur] = true;
            let (u, v) = boundary[cur];
            ids.push(v);
            let back = (nodes.pts[u].1 - nodes.pts[v].1).atan2(nodes.pts[u].0 - nodes.pts[v].0);
            let next = out_edges.get(&v).and_then(|c| {
                c.iter()
                    .copied()
                    .filter(|&i| !used[i] || i == s)
                    .min_by(|&i, &j| {
                        let ang = |e: usize| {
                            let w = nodes.pts[boundary[e].1];
                            let a = (back - (w.1 - nodes.pts[v].1).atan2(w.0 - nodes.pts[v].0))
                                .rem_euclid(std::f64::consts::TAU);
                            if a < 1e-12 {
                                std::f64::consts::TAU
                            } else {
                                a
                            }
                        };
                        ang(i).total_cmp(&ang(j))
                    })
            });
            match next {
                Some(n) if n == s => break,
                Some(n) => cur = n,
                None => break,
            }
        }
        ids.pop(); // the closing node repeats the first
        if ids.len() >= 3 {
            loops.push(
                tidy(ids.iter().map(|&i| nodes.pts[i]).collect())
                    .into_iter()
                    .map(|p| [p.0 as f32, p.1 as f32])
                    .collect::<Vec<P>>(),
            );
        }
    }
    loops.retain(|l: &Vec<P>| l.len() >= 3);
    Some(loops)
}

/// Drop vertices that sit on a straight run.
fn tidy(mut pts: Vec<Pt>) -> Vec<Pt> {
    loop {
        let n = pts.len();
        if n < 3 {
            return pts;
        }
        let mut keep = Vec::with_capacity(n);
        for i in 0..n {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let cr = cross(a, b, c);
            let dot = (b.0 - a.0) * (c.0 - b.0) + (b.1 - a.1) * (c.1 - b.1);
            // Drop b when it sits within a fiftieth of a pixel of the line from a to c.
            let ac = (c.0 - a.0).hypot(c.1 - a.1).max(1e-9);
            if !(cr.abs() / ac < 0.02 && dot > 0.0) {
                keep.push(b);
            }
        }
        if keep.len() == n {
            return pts;
        }
        pts = keep;
    }
}

/// Edges bucketed by y (for rays going east) and by x (for rays going north).
struct Strips<'a> {
    edges: &'a [(Pt, Pt, i32)],
    by_y: Vec<Vec<u32>>,
    by_x: Vec<Vec<u32>>,
    y0: f64,
    ys: f64,
    x0: f64,
    xs: f64,
}

impl<'a> Strips<'a> {
    fn new(edges: &'a [(Pt, Pt, i32)]) -> Self {
        let n = ((edges.len() as f64).sqrt() as usize).clamp(1, 512);
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for e in edges {
            for p in [e.0, e.1] {
                x0 = x0.min(p.0);
                x1 = x1.max(p.0);
                y0 = y0.min(p.1);
                y1 = y1.max(p.1);
            }
        }
        let ys = n as f64 / (y1 - y0).max(1e-9);
        let xs = n as f64 / (x1 - x0).max(1e-9);
        let mut by_y = vec![Vec::new(); n];
        let mut by_x = vec![Vec::new(); n];
        let bucket = |v: f64, o: f64, s: f64| (((v - o) * s) as usize).min(n - 1);
        for (i, e) in edges.iter().enumerate() {
            if (e.0 .1 - e.1 .1).abs() > 1e-12 {
                let (lo, hi) = (e.0 .1.min(e.1 .1), e.0 .1.max(e.1 .1));
                for b in bucket(lo, y0, ys)..=bucket(hi, y0, ys) {
                    by_y[b].push(i as u32);
                }
            }
            if (e.0 .0 - e.1 .0).abs() > 1e-12 {
                let (lo, hi) = (e.0 .0.min(e.1 .0), e.0 .0.max(e.1 .0));
                for b in bucket(lo, x0, xs)..=bucket(hi, x0, xs) {
                    by_x[b].push(i as u32);
                }
            }
        }
        Self {
            edges,
            by_y,
            by_x,
            y0,
            ys,
            x0,
            xs,
        }
    }
    /// Winding just east of `q`, ignoring edge `skip`.
    fn east(&self, q: Pt, skip: usize) -> i32 {
        let n = self.by_y.len();
        let b = (((q.1 - self.y0) * self.ys).max(0.0) as usize).min(n - 1);
        let mut w = 0;
        for &i in &self.by_y[b] {
            if i as usize == skip {
                continue;
            }
            let (a, c, m) = self.edges[i as usize];
            if (a.1 <= q.1) != (c.1 <= q.1) {
                let x = a.0 + (q.1 - a.1) / (c.1 - a.1) * (c.0 - a.0);
                if x > q.0 {
                    w += if c.1 > a.1 { m } else { -m };
                }
            }
        }
        w
    }
    /// Winding just north of `q`, ignoring edge `skip`.
    fn north(&self, q: Pt, skip: usize) -> i32 {
        let n = self.by_x.len();
        let b = (((q.0 - self.x0) * self.xs).max(0.0) as usize).min(n - 1);
        let mut w = 0;
        for &i in &self.by_x[b] {
            if i as usize == skip {
                continue;
            }
            let (a, c, m) = self.edges[i as usize];
            if (a.0 <= q.0) != (c.0 <= q.0) {
                let y = a.1 + (q.0 - a.0) / (c.0 - a.0) * (c.1 - a.1);
                if y > q.1 {
                    w += if c.0 < a.0 { m } else { -m };
                }
            }
        }
        w
    }
}

/// Signed shoelace area (positive for outer loops produced by [`resolve`]).
pub(crate) fn area(p: &[P]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1])
        })
        .sum::<f64>()
        / 2.0
}

fn pip(p: Pt, poly: &[P]) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (ax, ay, bx, by) = (
            f64::from(a[0]),
            f64::from(a[1]),
            f64::from(b[0]),
            f64::from(b[1]),
        );
        if (ay > p.1) != (by > p.1) && p.0 < ax + (p.1 - ay) / (by - ay) * (bx - ax) {
            c = !c;
        }
    }
    c
}

fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    // Proper or touching intersection, except when the segments only share an endpoint.
    let same = |p: Pt, q: Pt| (p.0 - q.0).abs() < 1e-7 && (p.1 - q.1).abs() < 1e-7;
    if same(a, c) || same(a, d) || same(b, c) || same(b, d) {
        return false;
    }
    let (d1, d2) = (cross(a, b, c), cross(a, b, d));
    let (d3, d4) = (cross(c, d, a), cross(c, d, b));
    ((d1 > 1e-9 && d2 < -1e-9) || (d1 < -1e-9 && d2 > 1e-9))
        && ((d3 > 1e-9 && d4 < -1e-9) || (d3 < -1e-9 && d4 > 1e-9))
}

fn pt64(p: P) -> Pt {
    (f64::from(p[0]), f64::from(p[1]))
}

/// Join holes to their enclosing outlines: one weakly simple polygon per outer loop.
pub(crate) fn bridged(loops: Vec<Vec<P>>) -> Vec<Bridged> {
    let (mut outers, mut holes): (Vec<Vec<P>>, Vec<Vec<P>>) =
        loops.into_iter().partition(|l| area(l) > 0.0);
    // Which outer encloses each hole: the smallest outer containing a point just inside the fill.
    let mut owner: Vec<Vec<Vec<P>>> = vec![Vec::new(); outers.len()];
    for h in holes.drain(..) {
        let n = h.len();
        let k = (0..n)
            .max_by(|&i, &j| {
                let l = |i: usize| {
                    let (a, b) = (h[i], h[(i + 1) % n]);
                    (a[0] - b[0]).hypot(a[1] - b[1])
                };
                l(i).total_cmp(&l(j))
            })
            .unwrap_or(0);
        let (a, b) = (pt64(h[k]), pt64(h[(k + 1) % n]));
        let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
        // Inside the fill is on the left of the edge.
        let q = (
            (a.0 + b.0) / 2.0 - (b.1 - a.1) / len * 1e-3,
            (a.1 + b.1) / 2.0 + (b.0 - a.0) / len * 1e-3,
        );
        let best = (0..outers.len())
            .filter(|&o| pip(q, &outers[o]))
            .min_by(|&x, &y| area(&outers[x]).total_cmp(&area(&outers[y])));
        if let Some(o) = best {
            owner[o].push(h);
        }
    }
    let mut out = Vec::new();
    for (o, mut hs) in outers.drain(..).zip(owner) {
        // Rightmost hole first, so earlier cuts never sit in the way of later ones.
        hs.sort_by(|a, b| {
            let mx = |h: &Vec<P>| h.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
            mx(b).total_cmp(&mx(a))
        });
        let mut poly = Bridged {
            soft: vec![false; o.len()],
            points: o,
        };
        for i in 0..hs.len() {
            let h = hs[i].clone();
            let rest: Vec<&Vec<P>> = hs[i + 1..].iter().collect();
            if !join(&mut poly, &h, &rest) {
                // No clear cut found: keep the hole as its own outline (drawn inverted
                // winding is ignored by the clipper, so skip it).
            }
        }
        out.push(poly);
    }
    out
}

/// Cut `hole` into `poly` along the shortest unobstructed segment.
fn join(poly: &mut Bridged, hole: &[P], others: &[&Vec<P>]) -> bool {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (hi, h) in hole.iter().enumerate() {
        for (pi, p) in poly.points.iter().enumerate() {
            let d = f64::from(h[0] - p[0]).powi(2) + f64::from(h[1] - p[1]).powi(2);
            pairs.push((d, hi, pi));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let blocked = |a: Pt, b: Pt| {
        let ring = |pts: &[P]| {
            let n = pts.len();
            (0..n).any(|i| seg_cross(a, b, pt64(pts[i]), pt64(pts[(i + 1) % n])))
        };
        ring(&poly.points) || ring(hole) || others.iter().any(|o| ring(o))
    };
    for (_, hi, pi) in pairs.into_iter().take(4000) {
        let (a, b) = (pt64(hole[hi]), pt64(poly.points[pi]));
        if blocked(a, b) {
            continue;
        }
        // The cut must run through filled area: its midpoint is inside the outline.
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        if !pip(mid, &poly.points) || hole.len() >= 3 && pip(mid, hole) {
            continue;
        }
        let n = hole.len();
        let mut pts = Vec::with_capacity(poly.points.len() + n + 2);
        let mut soft = Vec::with_capacity(pts.capacity());
        for i in 0..=pi {
            pts.push(poly.points[i]);
            soft.push(poly.soft[i]);
        }
        // Edge pi -> hole[hi] is the cut; around the hole; hole[hi] -> poly[pi] cuts back.
        *soft.last_mut().unwrap_or(&mut false) = true;
        for k in 0..n {
            pts.push(hole[(hi + k) % n]);
            soft.push(false);
        }
        *soft.last_mut().unwrap_or(&mut false) = false;
        pts.push(hole[hi]);
        soft.push(true);
        pts.push(poly.points[pi]);
        soft.push(poly.soft[pi]);
        for i in pi + 1..poly.points.len() {
            pts.push(poly.points[i]);
            soft.push(poly.soft[i]);
        }
        poly.points = pts;
        poly.soft = soft;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(x: f32, y: f32, s: f32) -> Vec<P> {
        vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]]
    }
    fn total(loops: &[Vec<P>]) -> f64 {
        loops.iter().map(|l| area(l)).sum()
    }

    #[test]
    fn nested_squares_even_odd_leave_a_hole() {
        let r = resolve(&[sq(0.0, 0.0, 10.0), sq(3.0, 3.0, 4.0)], FillRule::EvenOdd).unwrap();
        assert_eq!(r.len(), 2);
        assert!((total(&r) - (100.0 - 16.0)).abs() < 1e-3, "{}", total(&r));
    }

    #[test]
    fn nonzero_same_direction_fills_the_inner_square_opposite_makes_a_hole() {
        let same = resolve(&[sq(0.0, 0.0, 10.0), sq(3.0, 3.0, 4.0)], FillRule::NonZero).unwrap();
        assert!((total(&same) - 100.0).abs() < 1e-3);
        let mut rev = sq(3.0, 3.0, 4.0);
        rev.reverse();
        let hole = resolve(&[sq(0.0, 0.0, 10.0), rev], FillRule::NonZero).unwrap();
        assert!((total(&hole) - 84.0).abs() < 1e-3);
    }

    #[test]
    fn overlapping_squares_union() {
        let r = resolve(&[sq(0.0, 0.0, 10.0), sq(5.0, 5.0, 10.0)], FillRule::NonZero).unwrap();
        assert_eq!(r.len(), 1);
        assert!((total(&r) - 175.0).abs() < 1e-3, "{}", total(&r));
    }

    #[test]
    fn five_point_star_rules_differ_at_the_centre() {
        let star: Vec<P> = (0..5)
            .map(|i| {
                let a = (i as f32 * 144.0 - 90.0).to_radians();
                [50.0 + 40.0 * a.cos(), 50.0 + 40.0 * a.sin()]
            })
            .collect();
        let nz = resolve(&[star.clone()], FillRule::NonZero).unwrap();
        let eo = resolve(&[star], FillRule::EvenOdd).unwrap();
        // Non-zero fills the pentagon in the middle too; even-odd leaves it empty.
        assert!(total(&nz) > total(&eo) + 100.0, "{} {}", total(&nz), total(&eo));
        assert!(eo.iter().all(|l| area(l) > 0.0), "even-odd star is five tips");
        assert_eq!(eo.len(), 5);
        assert_eq!(nz.len(), 1);
    }

    #[test]
    fn bow_tie_resolves_into_two_triangles() {
        let bow = vec![[0.0, 0.0], [10.0, 10.0], [10.0, 0.0], [0.0, 10.0]];
        let r = resolve(&[bow], FillRule::NonZero).unwrap();
        assert_eq!(r.len(), 2);
        assert!((total(&r).abs() - 50.0).abs() < 1e-3, "{}", total(&r));
    }

    #[test]
    fn a_hole_is_bridged_into_one_polygon() {
        let mut inner = sq(3.0, 3.0, 4.0);
        inner.reverse();
        let r = resolve(&[sq(0.0, 0.0, 10.0), inner], FillRule::NonZero).unwrap();
        let b = bridged(r);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].points.len(), 4 + 4 + 2);
        assert_eq!(b[0].soft.iter().filter(|s| **s).count(), 2);
        assert!((area(&b[0].points) - 84.0).abs() < 1e-3);
    }
}
