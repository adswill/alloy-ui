//! Animation primitives: springs, eased tweens, and the [`Animatable`] values
//! they move. Pure maths, no UI state; [`crate::Ui`] owns the running clocks.
//!
//! Design stance (see `DESIGN.md`): motion is short, eased out and never
//! bouncy by default. Springs are therefore *critically damped* unless you
//! ask otherwise, so they settle without overshoot.

use alloy_ui_core::{Color, Point};
use alloy_ui_theme::Easing;

/// A damped spring, `x'' = -k (x - target) - c x'` per unit mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// Stiffness `k`. Higher is faster.
    pub stiffness: f32,
    /// Damping `c`. `2 * sqrt(k)` is critical damping (no overshoot).
    pub damping: f32,
}

impl Spring {
    /// Quick and settled: about 180 ms to rest, no overshoot. The default.
    pub const SNAPPY: Self = Self {
        stiffness: 700.0,
        damping: 52.915, // 2 * sqrt(700)
    };
    /// Slower, for larger movements such as panels: about 320 ms.
    pub const GENTLE: Self = Self {
        stiffness: 220.0,
        damping: 29.665, // 2 * sqrt(220)
    };
    /// A hint of overshoot (damping ratio 0.7). Opt-in only.
    pub const LIVELY: Self = Self {
        stiffness: 520.0,
        damping: 31.93, // 0.7 * 2 * sqrt(520)
    };

    /// A critically damped spring for any stiffness.
    pub fn new_critical(stiffness: f32) -> Self {
        Self {
            stiffness,
            damping: 2.0 * stiffness.sqrt(),
        }
    }

    /// Advance `(x, v)` toward `target` by `dt` seconds. Sub-stepped
    /// semi-implicit Euler: stable for any frame time we will see.
    pub fn step(self, x: f32, v: f32, target: f32, dt: f32) -> (f32, f32) {
        let (mut x, mut v) = (x, v);
        let n = ((dt / 0.004).ceil() as usize).clamp(1, 64);
        let h = dt / n as f32;
        for _ in 0..n {
            let a = -self.stiffness * (x - target) - self.damping * v;
            v += a * h;
            x += v * h;
        }
        (x, v)
    }

    /// True once position and velocity are both negligible.
    pub fn settled(x: f32, v: f32, target: f32) -> bool {
        (x - target).abs() < 0.001 && v.abs() < 0.01
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self::SNAPPY
    }
}

/// How a value travels to its target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transition {
    /// A timed curve.
    Tween {
        /// Duration in milliseconds. Zero jumps.
        ms: u32,
        /// The curve.
        easing: Easing,
    },
    /// A physical spring; retargeting keeps velocity.
    Spring(Spring),
}

impl Transition {
    /// An eased tween with the standard ease-out curve.
    pub const fn ms(ms: u32) -> Self {
        Self::Tween {
            ms,
            easing: Easing::EASE_OUT,
        }
    }
    /// A tween with an explicit curve.
    pub const fn eased(ms: u32, easing: Easing) -> Self {
        Self::Tween { ms, easing }
    }
    /// The default spring.
    pub const fn spring() -> Self {
        Self::Spring(Spring::SNAPPY)
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::ms(150)
    }
}

/// A value that can be blended, and therefore animated.
pub trait Animatable: Copy {
    /// Components, for blending.
    fn to_parts(self) -> [f32; 4];
    /// Rebuild from components.
    fn from_parts(p: [f32; 4]) -> Self;
}

impl Animatable for f32 {
    fn to_parts(self) -> [f32; 4] {
        [self, 0.0, 0.0, 0.0]
    }
    fn from_parts(p: [f32; 4]) -> Self {
        p[0]
    }
}

impl Animatable for Point {
    fn to_parts(self) -> [f32; 4] {
        [self.x, self.y, 0.0, 0.0]
    }
    fn from_parts(p: [f32; 4]) -> Self {
        Point::new(p[0], p[1])
    }
}

impl Animatable for (f32, f32) {
    fn to_parts(self) -> [f32; 4] {
        [self.0, self.1, 0.0, 0.0]
    }
    fn from_parts(p: [f32; 4]) -> Self {
        (p[0], p[1])
    }
}

impl Animatable for Color {
    /// Premultiplied, so fading to transparent does not drag through grey.
    fn to_parts(self) -> [f32; 4] {
        [self.r * self.a, self.g * self.a, self.b * self.a, self.a]
    }
    fn from_parts(p: [f32; 4]) -> Self {
        let a = p[3].clamp(0.0, 1.0);
        if a <= 1e-5 {
            return Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
        }
        Color {
            r: (p[0] / a).clamp(0.0, 1.0),
            g: (p[1] / a).clamp(0.0, 1.0),
            b: (p[2] / a).clamp(0.0, 1.0),
            a,
        }
    }
}

/// How far a [`crate::Ui::presence`] has travelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    /// 0 fully hidden, 1 fully shown; eases in both directions.
    pub progress: f32,
    /// False once an exit has finished: stop drawing and hit-testing.
    pub mounted: bool,
}

/// A value that passes through several stops over one run: `0.0` is the start
/// and `1.0` the end. Each segment eases into its stop with that stop's curve.
///
/// ```ignore
/// let k = Keyframes::new().at(0.0, 0.0).at(0.35, 1.0).at(1.0, 0.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Keyframes<T: Animatable> {
    stops: Vec<(f32, [f32; 4], Easing)>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Animatable> Default for Keyframes<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Animatable> Keyframes<T> {
    /// No stops yet.
    pub fn new() -> Self {
        Self {
            stops: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }
    /// A stop at `t` (0 to 1), reached with the standard ease-out.
    pub fn at(self, t: f32, v: T) -> Self {
        self.at_eased(t, v, Easing::EASE_OUT)
    }
    /// A stop at `t`, reached with `easing`.
    pub fn at_eased(mut self, t: f32, v: T, easing: Easing) -> Self {
        self.stops.push((t.clamp(0.0, 1.0), v.to_parts(), easing));
        self.stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        self
    }
    /// The value at progress `t` (clamped). Before the first stop it holds the
    /// first value, after the last it holds the last; with no stops, zeros.
    pub fn sample(&self, t: f32) -> T {
        let t = t.clamp(0.0, 1.0);
        let (Some(first), Some(last)) = (self.stops.first(), self.stops.last()) else {
            return T::from_parts([0.0; 4]);
        };
        if t <= first.0 {
            return T::from_parts(first.1);
        }
        if t >= last.0 {
            return T::from_parts(last.1);
        }
        for w in self.stops.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if t >= a.0 && t <= b.0 {
                let span = (b.0 - a.0).max(1e-6);
                let k = b.2.at((t - a.0) / span);
                return T::from_parts([0, 1, 2, 3].map(|i| a.1[i] + (b.1[i] - a.1[i]) * k));
            }
        }
        T::from_parts(last.1)
    }
}

/// Where a choreographed sequence is. Ask it for the progress of each part:
/// parts may overlap, start late or finish early.
///
/// ```ignore
/// let tl = ui.timeline(key, step, 900);
/// let card = tl.eased(0, 400, Easing::QUART_OUT);    // 0 to 1 over the first 400 ms
/// let badge = tl.eased(250, 700, Easing::QUART_OUT); // starts at 250 ms
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    /// Milliseconds since the sequence started (capped at its length).
    pub elapsed_ms: f32,
    /// Length of the whole sequence.
    pub total_ms: f32,
}

impl Timeline {
    /// Linear progress of the part running from `from_ms` to `to_ms`, 0 to 1.
    pub fn span(&self, from_ms: u32, to_ms: u32) -> f32 {
        let (a, b) = (from_ms as f32, to_ms as f32);
        if b <= a {
            return (self.elapsed_ms >= a) as u8 as f32;
        }
        ((self.elapsed_ms - a) / (b - a)).clamp(0.0, 1.0)
    }
    /// Like [`Timeline::span`], shaped by `easing`.
    pub fn eased(&self, from_ms: u32, to_ms: u32, easing: Easing) -> f32 {
        easing.at(self.span(from_ms, to_ms))
    }
    /// Progress of the whole sequence, 0 to 1.
    pub fn progress(&self) -> f32 {
        if self.total_ms <= 0.0 {
            1.0
        } else {
            (self.elapsed_ms / self.total_ms).clamp(0.0, 1.0)
        }
    }
    /// True once the sequence has finished.
    pub fn done(&self) -> bool {
        self.elapsed_ms >= self.total_ms
    }
}

/// Pack a 64-bit trigger into four exactly representable floats.
pub(crate) fn pack(trigger: u64) -> [f32; 4] {
    let t = trigger ^ 0xA5A5_5A5A_C3C3_3C3C;
    [0, 1, 2, 3].map(|i| ((t >> (16 * i)) & 0xFFFF) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: Spring, target: f32) -> (f32, f32, f32) {
        let (mut x, mut v, mut peak) = (0.0, 0.0, 0.0f32);
        let mut t = 0.0;
        while t < 2.0 {
            (x, v) = s.step(x, v, target, 1.0 / 60.0);
            peak = peak.max(x);
            t += 1.0 / 60.0;
            if Spring::settled(x, v, target) {
                return (x, peak, t);
            }
        }
        (x, peak, t)
    }

    #[test]
    fn critical_springs_settle_without_overshoot() {
        for s in [Spring::SNAPPY, Spring::GENTLE, Spring::new_critical(400.0)] {
            let (x, peak, t) = run(s, 1.0);
            assert!((x - 1.0).abs() < 0.01, "{s:?} ended at {x}");
            assert!(peak <= 1.002, "{s:?} overshot to {peak}");
            assert!(t < 0.7, "{s:?} took {t}s");
        }
    }

    #[test]
    fn lively_spring_overshoots_a_little_and_settles() {
        let (x, peak, _) = run(Spring::LIVELY, 1.0);
        assert!(peak > 1.0 && peak < 1.2, "peak {peak}");
        assert!((x - 1.0).abs() < 0.01);
    }

    #[test]
    fn spring_is_frame_rate_independent() {
        let a = Spring::SNAPPY.step(0.0, 0.0, 1.0, 0.1);
        let mut b = (0.0, 0.0);
        for _ in 0..10 {
            b = Spring::SNAPPY.step(b.0, b.1, 1.0, 0.01);
        }
        assert!((a.0 - b.0).abs() < 0.01, "{a:?} vs {b:?}");
    }

    #[test]
    fn colour_blend_is_premultiplied() {
        let c = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let t = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        let mid = Color::from_parts({
            let (p, q) = (c.to_parts(), t.to_parts());
            [0, 1, 2, 3].map(|i| (p[i] + q[i]) * 0.5)
        });
        assert!((mid.r - 1.0).abs() < 1e-4 && (mid.a - 0.5).abs() < 1e-4);
    }

    #[test]
    fn keyframes_pass_through_stops_and_hold_the_ends() {
        let k = Keyframes::<f32>::new()
            .at(0.0, 0.0)
            .at_eased(0.5, 10.0, Easing::LINEAR)
            .at_eased(1.0, 0.0, Easing::LINEAR);
        assert_eq!(k.sample(-1.0), 0.0);
        assert!((k.sample(0.25) - 5.0).abs() < 0.05);
        assert!((k.sample(0.5) - 10.0).abs() < 1e-3);
        assert!((k.sample(0.75) - 5.0).abs() < 0.05);
        assert_eq!(k.sample(2.0), 0.0);
    }

    #[test]
    fn timeline_spans_overlap_and_clamp() {
        let tl = Timeline {
            elapsed_ms: 300.0,
            total_ms: 600.0,
        };
        assert_eq!(tl.span(0, 200), 1.0);
        assert!((tl.span(200, 400) - 0.5).abs() < 1e-5);
        assert_eq!(tl.span(400, 600), 0.0);
        assert!(!tl.done() && (tl.progress() - 0.5).abs() < 1e-5);
    }
}
