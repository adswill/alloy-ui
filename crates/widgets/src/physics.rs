//! Gesture physics: momentum, rubber-banding and snap points. Pure maths with
//! no UI state, so it is unit-tested to the digit; [`crate::Ui`] and the
//! widgets own the clocks that drive it.
//!
//! Three ideas cover every drag in Alloy:
//!
//! * **Rubber band** ([`rubber_band`]): past an edge the content follows the
//!   pointer with growing resistance, so it can never be dragged far out.
//! * **Fling** ([`fling_step`]): on release the content keeps its velocity
//!   and decays with friction; if it runs into an edge it is caught by a
//!   critically damped spring and settles on the edge without bouncing back.
//! * **Snap** ([`snap_target`]): pick the snap point a release is *heading*
//!   for (where friction would carry it), not just the nearest one, so a
//!   short flick moves one step and a hard flick moves further.

/// Friction rate of a fling: velocity decays as `exp(-rate * t)`. Higher
/// stops sooner; `4.0` carries a 2000 px/s flick about 500 px.
pub const FRICTION: f32 = 4.0;
/// Stiffness of the spring that catches a fling at an edge.
pub const EDGE_STIFFNESS: f32 = 320.0;
/// How strongly an edge resists a drag; the classic 0.55.
const RUBBER: f32 = 0.55;

/// Displayed distance for a drag `over` pixels past an edge of a viewport
/// `dim` pixels long. Always below `dim`, always increasing, slope 0.55 at
/// the edge and falling to zero: the further you pull the harder it resists.
pub fn rubber_band(over: f32, dim: f32) -> f32 {
    let dim = dim.max(1.0);
    let o = over.abs();
    let shown = (1.0 - 1.0 / (o * RUBBER / dim + 1.0)) * dim;
    shown.copysign(over)
}

/// Inverse of [`rubber_band`]: the pointer travel that shows `shown` pixels.
pub fn rubber_band_inv(shown: f32, dim: f32) -> f32 {
    let dim = dim.max(1.0);
    let s = shown.abs().min(dim * 0.999);
    (dim * s / (dim - s) / RUBBER).copysign(shown)
}

/// How far a fling started at `velocity` (px/s) would travel before friction
/// stops it, ignoring edges.
pub fn fling_distance(velocity: f32) -> f32 {
    velocity / FRICTION
}

/// Advance a fling one step. `x` is the content position and `v` its
/// velocity in px/s, `[min, max]` the legal range. Inside the range velocity
/// decays by [`FRICTION`]; outside it a critically damped spring pulls the
/// position back to the edge. Returns `(x, v, at_rest)`.
pub fn fling_step(x: f32, v: f32, min: f32, max: f32, dt: f32) -> (f32, f32, bool) {
    let (mut x, mut v) = (x, v);
    let n = ((dt / 0.004).ceil() as usize).clamp(1, 64);
    let h = dt / n as f32;
    let c = 2.0 * EDGE_STIFFNESS.sqrt();
    for _ in 0..n {
        let edge = x.clamp(min, max);
        if (x - edge).abs() > 1e-4 {
            let a = -EDGE_STIFFNESS * (x - edge) - c * v;
            v += a * h;
        } else {
            v -= FRICTION * v * h;
        }
        x += v * h;
    }
    let edge = x.clamp(min, max);
    let inside = (x - edge).abs() <= 0.01;
    let rest =
        if inside { v.abs() < 8.0 } else { false } || ((x - edge).abs() < 0.05 && v.abs() < 1.0);
    if rest {
        return (edge, 0.0, true);
    }
    (x, v, false)
}

/// The snap point a release at `pos` with velocity `vel` (px/s) is heading
/// for: the point nearest to where friction would carry it. `points` need not
/// be sorted. Returns `pos` when there are none.
pub fn snap_target(pos: f32, vel: f32, points: &[f32]) -> f32 {
    let heading = pos + fling_distance(vel) * 0.35;
    points
        .iter()
        .copied()
        .min_by(|a, b| {
            (a - heading)
                .abs()
                .partial_cmp(&(b - heading).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(pos)
}

/// Estimates pointer velocity from recent positions, ignoring samples older
/// than a short window so a pause before release reads as zero.
#[derive(Debug, Clone, Default)]
pub struct Tracker {
    samples: Vec<(u64, f32)>,
}

/// Samples older than this (ms) do not count towards the velocity.
const WINDOW_MS: u64 = 90;

impl Tracker {
    /// An empty tracker.
    pub fn new() -> Self {
        Self::default()
    }
    /// Record that the pointer is at `pos` at time `t_ms`.
    pub fn push(&mut self, t_ms: u64, pos: f32) {
        self.samples.push((t_ms, pos));
        if self.samples.len() > 12 {
            self.samples.remove(0);
        }
    }
    /// Velocity in px/s as of `now_ms`; zero if the pointer has been still.
    pub fn velocity(&self, now_ms: u64) -> f32 {
        let recent: Vec<&(u64, f32)> = self
            .samples
            .iter()
            .filter(|(t, _)| now_ms.saturating_sub(*t) <= WINDOW_MS)
            .collect();
        let (Some(first), Some(last)) = (recent.first(), recent.last()) else {
            return 0.0;
        };
        let dt = last.0.saturating_sub(first.0);
        if dt == 0 {
            return 0.0;
        }
        (last.1 - first.1) / (dt as f32 / 1000.0)
    }
    /// Forget everything.
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

/// The phase of a scroll gesture, as inferred from the stream of wheel events.
/// Trackpads deliver a burst of small deltas per swipe, then (on macOS) the
/// system's own momentum tail; a gap with no events ends the gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    /// Nothing is happening.
    #[default]
    Idle,
    /// The first event of a gesture.
    Began,
    /// More events in the same gesture.
    Changed,
    /// The gesture is over and the content is coasting or springing back.
    Coasting,
}

/// How long (ms) without a wheel event ends a gesture.
pub const GESTURE_GAP_MS: u64 = 90;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rubber_band_is_monotone_bounded_and_invertible() {
        let dim = 400.0;
        let mut prev = 0.0;
        for i in 1..200 {
            let over = i as f32 * 10.0;
            let s = rubber_band(over, dim);
            assert!(s > prev && s < dim, "monotone and below dim: {s}");
            prev = s;
            let back = rubber_band_inv(s, dim);
            assert!((back - over).abs() / over < 0.01, "{back} vs {over}");
        }
        assert_eq!(rubber_band(0.0, dim), 0.0);
        assert!(rubber_band(-50.0, dim) < 0.0, "sign is kept");
        // Near the edge the slope is the classic 0.55.
        let k = rubber_band(1.0, dim);
        assert!((k - 0.55).abs() < 0.01, "{k}");
    }

    #[test]
    fn fling_decays_and_stops_inside() {
        let (mut x, mut v) = (0.0, 2000.0);
        let mut t = 0.0;
        for _ in 0..2000 {
            let (nx, nv, rest) = fling_step(x, v, -1e9, 1e9, 1.0 / 120.0);
            x = nx;
            v = nv;
            t += 1.0 / 120.0;
            if rest {
                break;
            }
        }
        assert!(t < 3.0, "friction stops it in under 3 s, took {t}");
        let want = fling_distance(2000.0);
        assert!(
            (x - want).abs() < want * 0.05,
            "travels ~v/rate: {x} vs {want}"
        );
    }

    #[test]
    fn fling_into_an_edge_overshoots_a_little_and_settles_on_it() {
        let (mut x, mut v) = (90.0, 3000.0);
        let mut peak: f32 = 0.0;
        let mut ended = false;
        for _ in 0..4000 {
            let (nx, nv, rest) = fling_step(x, v, 0.0, 100.0, 1.0 / 120.0);
            x = nx;
            v = nv;
            peak = peak.max(x);
            if rest {
                ended = true;
                break;
            }
        }
        assert!(ended, "comes to rest");
        assert_eq!(x, 100.0, "rests exactly on the edge");
        assert!(
            peak > 100.0 && peak < 100.0 + 120.0,
            "bounded overshoot {peak}"
        );
    }

    #[test]
    fn fling_never_bounces_back_past_the_edge() {
        let (mut x, mut v) = (50.0, 2500.0);
        let mut arrived = false;
        for _ in 0..4000 {
            let (nx, nv, rest) = fling_step(x, v, 0.0, 100.0, 1.0 / 120.0);
            x = nx;
            v = nv;
            arrived |= x >= 100.0;
            assert!(
                !arrived || x > 99.9,
                "critically damped: never crosses back, got {x}"
            );
            if rest {
                break;
            }
        }
    }

    #[test]
    fn snap_picks_where_the_release_is_heading() {
        let pts = [0.0, 200.0, 400.0];
        assert_eq!(snap_target(90.0, 0.0, &pts), 0.0);
        assert_eq!(snap_target(110.0, 0.0, &pts), 200.0);
        assert_eq!(
            snap_target(90.0, 1500.0, &pts),
            200.0,
            "a flick carries it on"
        );
        assert_eq!(snap_target(300.0, -1500.0, &pts), 200.0);
        assert_eq!(snap_target(5.0, 0.0, &[]), 5.0);
    }

    #[test]
    fn tracker_measures_speed_and_forgets_a_pause() {
        let mut t = Tracker::new();
        for i in 0..6u64 {
            t.push(i * 16, i as f32 * 8.0);
        }
        let v = t.velocity(80);
        assert!(
            (v - 500.0).abs() < 5.0,
            "8 px per 16 ms = 500 px/s, got {v}"
        );
        assert_eq!(t.velocity(400), 0.0, "a pause before release reads as rest");
    }
}
