//! Touch input and gesture recognition.
//!
//! The recognizer is a pure state machine: it takes finger events with a
//! timestamp and returns [`Gesture`]s, so it is unit-tested on any OS and
//! drives every touchscreen (phones, tablets, touch laptops). The shell feeds
//! it from `winit` touch events and turns the result into the mouse-style
//! events widgets already understand (see `shell.rs`).

use alloy_ui_core::{Event, MouseButton, Point};

/// Movement (logical px) a finger may drift before a press becomes a pan.
pub const TAP_SLOP: f32 = 10.0;
/// Hold time (ms) that turns a press into a long press.
pub const LONG_PRESS_MS: u64 = 500;
/// Longest press (ms) that still counts as a tap.
pub const TAP_MAX_MS: u64 = 400;
/// Two taps within this time (ms) and [`DOUBLE_TAP_DIST`] make a double tap.
pub const DOUBLE_TAP_MS: u64 = 300;
/// Max distance (logical px) between the taps of a double tap.
pub const DOUBLE_TAP_DIST: f32 = 40.0;
/// Velocity is measured over this many recent milliseconds.
const VELOCITY_WINDOW_MS: u64 = 100;

/// Where a finger is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    /// Finger touched down.
    Started,
    /// Finger moved.
    Moved,
    /// Finger lifted.
    Ended,
    /// The system took the touch away (palm rejection, incoming call).
    Cancelled,
}

/// A raw finger event, positions in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchEvent {
    /// Finger id, stable from `Started` to `Ended`/`Cancelled`.
    pub id: u64,
    /// Phase.
    pub phase: TouchPhase,
    /// Position.
    pub pos: Point,
}

/// A recognised high-level gesture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    /// A quick touch and release.
    Tap(Point),
    /// A second tap close to the first.
    DoubleTap(Point),
    /// A finger held still; fires once while still down.
    LongPress(Point),
    /// One finger dragging. `delta` is the movement since the last event.
    Pan {
        /// Finger movement since the previous `Pan`.
        delta: (f32, f32),
        /// Current finger position.
        pos: Point,
    },
    /// The panning finger lifted; `velocity` is in logical px per second.
    PanEnd {
        /// Release velocity, for momentum scrolling.
        velocity: (f32, f32),
    },
    /// Two fingers moving apart (>1) or together (<1), relative to the last event.
    Pinch {
        /// Midpoint between the fingers.
        center: Point,
        /// Ratio of the finger distance now to before.
        scale: f32,
    },
}

#[derive(Debug, Clone, Copy)]
struct Finger {
    id: u64,
    pos: Point,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    /// No finger down, or an earlier multi-touch is still winding down.
    Idle,
    /// One finger down, undecided.
    Pressed,
    /// One finger dragging.
    Panning,
    /// Long press fired; the rest of this touch is inert.
    Held,
    /// Two (or more) fingers.
    Pinching,
}

/// Turns finger events into gestures. See the module docs.
#[derive(Debug)]
pub struct GestureRecognizer {
    fingers: Vec<Finger>,
    mode: Mode,
    down_pos: Point,
    down_at: u64,
    last_pan: Point,
    pinch_dist: f32,
    last_tap: Option<(Point, u64)>,
    /// Recent (time, position) samples of the primary finger.
    samples: Vec<(u64, Point)>,
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

fn dist(a: Point, b: Point) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

impl GestureRecognizer {
    /// A recognizer with no fingers down.
    pub fn new() -> Self {
        GestureRecognizer {
            fingers: Vec::new(),
            mode: Mode::Idle,
            down_pos: Point::default(),
            down_at: 0,
            last_pan: Point::default(),
            pinch_dist: 0.0,
            last_tap: None,
            samples: Vec::new(),
        }
    }

    /// Number of fingers currently down.
    pub fn finger_count(&self) -> usize {
        self.fingers.len()
    }

    /// When [`poll`](Self::poll) next has something to say (a long press),
    /// as a timestamp in the same clock as `touch`. `None` if nothing is pending.
    pub fn next_deadline(&self) -> Option<u64> {
        (self.mode == Mode::Pressed).then_some(self.down_at + LONG_PRESS_MS)
    }

    /// Check timers. Call when [`next_deadline`](Self::next_deadline) passes.
    pub fn poll(&mut self, now_ms: u64) -> Vec<Gesture> {
        if self.mode == Mode::Pressed && now_ms >= self.down_at + LONG_PRESS_MS {
            self.mode = Mode::Held;
            return vec![Gesture::LongPress(self.down_pos)];
        }
        Vec::new()
    }

    fn velocity(&self) -> (f32, f32) {
        let Some(&(t1, p1)) = self.samples.last() else {
            return (0.0, 0.0);
        };
        let first = self
            .samples
            .iter()
            .find(|(t, _)| t1 - t <= VELOCITY_WINDOW_MS)
            .copied()
            .unwrap_or((t1, p1));
        let dt = (t1 - first.0) as f32 / 1000.0;
        if dt <= 0.0 {
            return (0.0, 0.0);
        }
        ((p1.x - first.1.x) / dt, (p1.y - first.1.y) / dt)
    }

    /// Feed one finger event at time `now_ms` (any monotonic clock).
    pub fn touch(&mut self, e: TouchEvent, now_ms: u64) -> Vec<Gesture> {
        let mut out = Vec::new();
        match e.phase {
            TouchPhase::Started => {
                self.fingers.retain(|f| f.id != e.id);
                self.fingers.push(Finger {
                    id: e.id,
                    pos: e.pos,
                });
                if self.fingers.len() == 1 {
                    self.mode = Mode::Pressed;
                    self.down_pos = e.pos;
                    self.down_at = now_ms;
                    self.last_pan = e.pos;
                    self.samples.clear();
                    self.samples.push((now_ms, e.pos));
                } else {
                    // A second finger turns the touch into a pinch.
                    if self.mode == Mode::Panning {
                        out.push(Gesture::PanEnd {
                            velocity: (0.0, 0.0),
                        });
                    }
                    self.mode = Mode::Pinching;
                    self.pinch_dist = self.pinch_distance();
                }
            }
            TouchPhase::Moved => {
                let Some(f) = self.fingers.iter_mut().find(|f| f.id == e.id) else {
                    return out;
                };
                f.pos = e.pos;
                match self.mode {
                    Mode::Pressed | Mode::Panning if self.fingers.len() == 1 => {
                        self.samples.push((now_ms, e.pos));
                        self.samples
                            .retain(|(t, _)| now_ms.saturating_sub(*t) <= VELOCITY_WINDOW_MS * 2);
                        if self.mode == Mode::Pressed && dist(e.pos, self.down_pos) > TAP_SLOP {
                            self.mode = Mode::Panning;
                            // Do not jump by the slop: pan from where the finger is.
                            self.last_pan = e.pos;
                        }
                        if self.mode == Mode::Panning {
                            let delta = (e.pos.x - self.last_pan.x, e.pos.y - self.last_pan.y);
                            self.last_pan = e.pos;
                            if delta != (0.0, 0.0) {
                                out.push(Gesture::Pan { delta, pos: e.pos });
                            }
                        }
                    }
                    Mode::Pinching if self.fingers.len() >= 2 => {
                        let d = self.pinch_distance();
                        if self.pinch_dist > 1.0 && d > 1.0 {
                            out.push(Gesture::Pinch {
                                center: self.pinch_center(),
                                scale: d / self.pinch_dist,
                            });
                        }
                        self.pinch_dist = d;
                    }
                    _ => {}
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let ended = e.phase == TouchPhase::Ended;
                self.fingers.retain(|f| f.id != e.id);
                if self.mode == Mode::Pinching {
                    // Stay inert until every finger is up.
                    if self.fingers.is_empty() {
                        self.mode = Mode::Idle;
                    }
                    return out;
                }
                match self.mode {
                    Mode::Pressed if ended => {
                        if now_ms.saturating_sub(self.down_at) <= TAP_MAX_MS {
                            let double = self.last_tap.is_some_and(|(p, t)| {
                                now_ms.saturating_sub(t) <= DOUBLE_TAP_MS
                                    && dist(p, e.pos) <= DOUBLE_TAP_DIST
                            });
                            if double {
                                self.last_tap = None;
                                out.push(Gesture::DoubleTap(e.pos));
                            } else {
                                self.last_tap = Some((e.pos, now_ms));
                                out.push(Gesture::Tap(e.pos));
                            }
                        }
                    }
                    Mode::Panning => {
                        let velocity = if ended {
                            self.samples.push((now_ms, e.pos));
                            self.velocity()
                        } else {
                            (0.0, 0.0)
                        };
                        out.push(Gesture::PanEnd { velocity });
                    }
                    _ => {}
                }
                self.mode = Mode::Idle;
                self.samples.clear();
            }
        }
        out
    }

    fn pinch_distance(&self) -> f32 {
        match self.fingers.as_slice() {
            [a, b, ..] => dist(a.pos, b.pos),
            _ => 0.0,
        }
    }

    fn pinch_center(&self) -> Point {
        match self.fingers.as_slice() {
            [a, b, ..] => Point::new((a.pos.x + b.pos.x) / 2.0, (a.pos.y + b.pos.y) / 2.0),
            _ => Point::default(),
        }
    }
}

/// Lets the first finger stand in for the mouse, so every widget works by touch.
///
/// Pure: it returns the [`Event`]s to deliver, in order. A touch that becomes
/// a pan, a long press or a pinch releases the button *outside* the widget
/// first (`CursorLeft`, then release), so it can never end as a click.
#[derive(Debug, Default)]
pub struct PointerEmulation {
    primary: Option<u64>,
    down: bool,
    /// The press turned into a pan: it must end without hovering anything.
    panned: bool,
}

impl PointerEmulation {
    /// No finger down.
    pub fn new() -> Self {
        Self::default()
    }

    fn cancel(&mut self, out: &mut Vec<Event>) {
        if std::mem::take(&mut self.down) {
            out.push(Event::CursorLeft);
            out.push(Event::MouseReleased(MouseButton::Left));
        }
    }

    /// Call *before* feeding `e` to the recognizer; `first` is true when no
    /// finger was down until now. Handles finger down and move.
    pub fn before(&mut self, e: TouchEvent, first: bool) -> Vec<Event> {
        let mut out = Vec::new();
        match e.phase {
            TouchPhase::Started if first => {
                self.primary = Some(e.id);
                self.down = true;
                self.panned = false;
                out.push(Event::CursorMoved(e.pos));
                out.push(Event::MousePressed(MouseButton::Left));
            }
            // A second finger: this is no longer a click.
            TouchPhase::Started => self.cancel(&mut out),
            TouchPhase::Moved if self.primary == Some(e.id) && self.down => {
                out.push(Event::CursorMoved(e.pos));
            }
            _ => {}
        }
        out
    }

    /// Built-in effect of a recognised gesture: pans scroll, a long press is
    /// a secondary click. Call for every gesture the recognizer returns.
    pub fn gesture(&mut self, g: Gesture) -> Vec<Event> {
        let mut out = Vec::new();
        match g {
            // Dragging up reveals later content, like a wheel scrolled down.
            Gesture::Pan { delta, .. } => {
                self.panned = true;
                out.push(Event::Scrolled {
                    dx: -delta.0,
                    dy: -delta.1,
                });
            }
            Gesture::LongPress(at) => {
                self.cancel(&mut out);
                out.push(Event::CursorMoved(at));
                out.push(Event::MousePressed(MouseButton::Right));
                out.push(Event::MouseReleased(MouseButton::Right));
            }
            Gesture::PanEnd { .. }
            | Gesture::Tap(_)
            | Gesture::DoubleTap(_)
            | Gesture::Pinch { .. } => {}
        }
        out
    }

    /// Call *after* feeding `e` to the recognizer. Handles finger up.
    pub fn after(&mut self, e: TouchEvent) -> Vec<Event> {
        let mut out = Vec::new();
        if matches!(e.phase, TouchPhase::Ended | TouchPhase::Cancelled)
            && self.primary == Some(e.id)
        {
            self.primary = None;
            if self.down {
                if e.phase == TouchPhase::Ended && !self.panned {
                    self.down = false;
                    out.push(Event::CursorMoved(e.pos));
                    out.push(Event::MouseReleased(MouseButton::Left));
                    out.push(Event::CursorLeft);
                } else {
                    // Cancelled, or a pan: leave first so the release cannot click.
                    self.cancel(&mut out);
                }
            }
        }
        out
    }
}

/// Momentum after a pan: decays the release velocity exponentially.
///
/// Call [`step`](Fling::step) once per frame; it returns the scroll delta for
/// that frame until the speed falls below a visible threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fling {
    vx: f32,
    vy: f32,
}

impl Fling {
    /// Time constant of the decay, in seconds (velocity falls to 1/e).
    pub const TAU: f32 = 0.35;
    /// Below this speed (logical px/s) the fling stops.
    pub const MIN_SPEED: f32 = 30.0;
    /// Flings slower than this are ignored (a slow drag release is not a flick).
    pub const START_SPEED: f32 = 120.0;

    /// A fling from a release velocity, or `None` if it is too slow.
    pub fn new(velocity: (f32, f32)) -> Option<Fling> {
        let speed = (velocity.0.powi(2) + velocity.1.powi(2)).sqrt();
        (speed >= Self::START_SPEED).then_some(Fling {
            vx: velocity.0,
            vy: velocity.1,
        })
    }

    /// Advance by `dt` seconds. Returns the finger-space delta to apply
    /// (px), or `None` once the fling is over.
    pub fn step(&mut self, dt: f32) -> Option<(f32, f32)> {
        let delta = (self.vx * dt, self.vy * dt);
        let k = (-dt / Self::TAU).exp();
        self.vx *= k;
        self.vy *= k;
        let speed = (self.vx.powi(2) + self.vy.powi(2)).sqrt();
        (speed >= Self::MIN_SPEED).then_some(delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: u64, phase: TouchPhase, x: f32, y: f32) -> TouchEvent {
        TouchEvent {
            id,
            phase,
            pos: Point::new(x, y),
        }
    }

    #[test]
    fn quick_touch_is_a_tap() {
        let mut r = GestureRecognizer::new();
        assert!(r
            .touch(ev(1, TouchPhase::Started, 50.0, 50.0), 0)
            .is_empty());
        let g = r.touch(ev(1, TouchPhase::Ended, 52.0, 51.0), 80);
        assert_eq!(g, vec![Gesture::Tap(Point::new(52.0, 51.0))]);
        assert_eq!(r.finger_count(), 0);
    }

    #[test]
    fn two_quick_taps_make_a_double_tap() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 10.0, 10.0), 0);
        r.touch(ev(1, TouchPhase::Ended, 10.0, 10.0), 50);
        r.touch(ev(2, TouchPhase::Started, 14.0, 12.0), 150);
        let g = r.touch(ev(2, TouchPhase::Ended, 14.0, 12.0), 200);
        assert_eq!(g, vec![Gesture::DoubleTap(Point::new(14.0, 12.0))]);
        // A third tap starts over as a single tap.
        r.touch(ev(3, TouchPhase::Started, 14.0, 12.0), 260);
        let g = r.touch(ev(3, TouchPhase::Ended, 14.0, 12.0), 300);
        assert!(matches!(g[0], Gesture::Tap(_)));
    }

    #[test]
    fn slow_second_tap_is_not_double() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 10.0, 10.0), 0);
        r.touch(ev(1, TouchPhase::Ended, 10.0, 10.0), 50);
        r.touch(ev(2, TouchPhase::Started, 10.0, 10.0), 900);
        let g = r.touch(ev(2, TouchPhase::Ended, 10.0, 10.0), 950);
        assert!(matches!(g[0], Gesture::Tap(_)));
    }

    #[test]
    fn long_hold_fires_long_press_once_and_no_tap() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 5.0, 5.0), 100);
        assert_eq!(r.next_deadline(), Some(100 + LONG_PRESS_MS));
        assert!(r.poll(300).is_empty());
        assert_eq!(
            r.poll(100 + LONG_PRESS_MS),
            vec![Gesture::LongPress(Point::new(5.0, 5.0))]
        );
        assert_eq!(r.next_deadline(), None);
        assert!(r.poll(2000).is_empty());
        assert!(r.touch(ev(1, TouchPhase::Ended, 5.0, 5.0), 2100).is_empty());
    }

    #[test]
    fn drag_past_slop_pans_without_jumping() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 0.0, 100.0), 0);
        // Within slop: nothing.
        assert!(r.touch(ev(1, TouchPhase::Moved, 0.0, 96.0), 10).is_empty());
        // Crosses slop at y=80: pan starts from there, delta 0 is not emitted.
        assert!(r.touch(ev(1, TouchPhase::Moved, 0.0, 80.0), 20).is_empty());
        let g = r.touch(ev(1, TouchPhase::Moved, 0.0, 70.0), 30);
        assert_eq!(
            g,
            vec![Gesture::Pan {
                delta: (0.0, -10.0),
                pos: Point::new(0.0, 70.0)
            }]
        );
        assert_eq!(r.next_deadline(), None, "panning cancels the long press");
    }

    #[test]
    fn pan_release_reports_velocity() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 0.0, 500.0), 0);
        for i in 1..=10u64 {
            r.touch(
                ev(1, TouchPhase::Moved, 0.0, 500.0 - 10.0 * i as f32),
                i * 10,
            );
        }
        let g = r.touch(ev(1, TouchPhase::Ended, 0.0, 400.0), 100);
        match g.as_slice() {
            [Gesture::PanEnd { velocity }] => {
                assert!((velocity.1 - -1000.0).abs() < 50.0, "{velocity:?}");
                assert_eq!(velocity.0, 0.0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cancelled_pan_has_no_momentum() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 0.0, 0.0), 0);
        r.touch(ev(1, TouchPhase::Moved, 0.0, 50.0), 10);
        let g = r.touch(ev(1, TouchPhase::Cancelled, 0.0, 50.0), 20);
        assert_eq!(
            g,
            vec![Gesture::PanEnd {
                velocity: (0.0, 0.0)
            }]
        );
    }

    #[test]
    fn two_fingers_pinch_and_stay_inert_afterwards() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 100.0, 100.0), 0);
        r.touch(ev(2, TouchPhase::Started, 200.0, 100.0), 5);
        let g = r.touch(ev(2, TouchPhase::Moved, 300.0, 100.0), 10);
        match g.as_slice() {
            [Gesture::Pinch { center, scale }] => {
                assert!((scale - 2.0).abs() < 1e-5);
                assert_eq!(*center, Point::new(200.0, 100.0));
            }
            other => panic!("{other:?}"),
        }
        // Lifting one finger and moving the other must not start a pan or tap.
        assert!(r
            .touch(ev(2, TouchPhase::Ended, 300.0, 100.0), 20)
            .is_empty());
        assert!(r
            .touch(ev(1, TouchPhase::Moved, 100.0, 300.0), 30)
            .is_empty());
        assert!(r
            .touch(ev(1, TouchPhase::Ended, 100.0, 300.0), 40)
            .is_empty());
        // A fresh touch works again.
        r.touch(ev(3, TouchPhase::Started, 1.0, 1.0), 500);
        assert!(matches!(
            r.touch(ev(3, TouchPhase::Ended, 1.0, 1.0), 540)[0],
            Gesture::Tap(_)
        ));
    }

    #[test]
    fn second_finger_ends_a_pan() {
        let mut r = GestureRecognizer::new();
        r.touch(ev(1, TouchPhase::Started, 0.0, 0.0), 0);
        r.touch(ev(1, TouchPhase::Moved, 0.0, 40.0), 10);
        let g = r.touch(ev(2, TouchPhase::Started, 50.0, 50.0), 20);
        assert!(matches!(g[0], Gesture::PanEnd { .. }));
    }

    #[test]
    fn fling_decays_and_stops() {
        assert!(Fling::new((0.0, 50.0)).is_none(), "too slow to fling");
        let mut f = Fling::new((0.0, -1000.0)).expect("fast enough");
        let mut total = 0.0;
        let mut frames = 0;
        while let Some((_, dy)) = f.step(1.0 / 60.0) {
            total += dy;
            frames += 1;
            assert!(frames < 1000, "never stops");
        }
        // Distance of an exponential decay is about v * tau.
        assert!((total - -350.0).abs() < 40.0, "total {total}");
        assert!((60..=300).contains(&frames), "frames {frames}");
    }

    /// Drive recognizer and emulation together, as the shell does.
    fn run(steps: &[(TouchEvent, u64)]) -> Vec<Event> {
        let (mut r, mut p) = (GestureRecognizer::new(), PointerEmulation::new());
        let mut out = Vec::new();
        for &(e, t) in steps {
            let first = r.finger_count() == 0;
            out.extend(p.before(e, first));
            for g in r.touch(e, t) {
                out.extend(p.gesture(g));
            }
            out.extend(p.after(e));
        }
        out
    }

    #[test]
    fn tap_clicks_the_left_button_and_clears_hover() {
        let at = Point::new(30.0, 40.0);
        let out = run(&[
            (ev(1, TouchPhase::Started, 30.0, 40.0), 0),
            (ev(1, TouchPhase::Ended, 30.0, 40.0), 60),
        ]);
        assert_eq!(
            out,
            vec![
                Event::CursorMoved(at),
                Event::MousePressed(MouseButton::Left),
                Event::CursorMoved(at),
                Event::MouseReleased(MouseButton::Left),
                Event::CursorLeft,
            ]
        );
    }

    #[test]
    fn pan_scrolls_and_never_clicks() {
        let out = run(&[
            (ev(1, TouchPhase::Started, 0.0, 200.0), 0),
            (ev(1, TouchPhase::Moved, 0.0, 150.0), 16),
            (ev(1, TouchPhase::Moved, 0.0, 120.0), 32),
            (ev(1, TouchPhase::Ended, 0.0, 120.0), 48),
        ]);
        assert!(out.contains(&Event::Scrolled { dx: 0.0, dy: 30.0 }));
        // Finger moved up, content moves up: positive dy, like a wheel scrolled down.
        let release = out
            .iter()
            .position(|e| *e == Event::MouseReleased(MouseButton::Left))
            .expect("button is released");
        // ...and the pointer has already left the widget when it is.
        assert_eq!(out[release - 1], Event::CursorLeft);
    }

    #[test]
    fn long_press_is_a_right_click_and_swallows_the_left() {
        let (mut r, mut p) = (GestureRecognizer::new(), PointerEmulation::new());
        let down = ev(1, TouchPhase::Started, 9.0, 9.0);
        let mut out = p.before(down, true);
        r.touch(down, 0);
        for g in r.poll(LONG_PRESS_MS) {
            out.extend(p.gesture(g));
        }
        let up = ev(1, TouchPhase::Ended, 9.0, 9.0);
        r.touch(up, 900);
        out.extend(p.after(up));
        let lefts = out
            .iter()
            .filter(|e| matches!(e, Event::MouseReleased(MouseButton::Left)))
            .count();
        assert_eq!(lefts, 1, "released once, by the long press: {out:?}");
        assert!(out.contains(&Event::MousePressed(MouseButton::Right)));
        assert!(out.contains(&Event::MouseReleased(MouseButton::Right)));
        // After the long press, lifting the finger sends nothing more.
        assert_eq!(out.last(), Some(&Event::MouseReleased(MouseButton::Right)));
    }

    #[test]
    fn second_finger_cancels_the_click() {
        let out = run(&[
            (ev(1, TouchPhase::Started, 10.0, 10.0), 0),
            (ev(2, TouchPhase::Started, 60.0, 10.0), 20),
            (ev(2, TouchPhase::Ended, 60.0, 10.0), 60),
            (ev(1, TouchPhase::Ended, 10.0, 10.0), 80),
        ]);
        let n = |want: &Event| out.iter().filter(|e| *e == want).count();
        assert_eq!(n(&Event::MousePressed(MouseButton::Left)), 1);
        assert_eq!(n(&Event::MouseReleased(MouseButton::Left)), 1);
        // The only release is the cancel: CursorLeft comes right before it.
        let i = out
            .iter()
            .position(|e| *e == Event::MouseReleased(MouseButton::Left))
            .unwrap();
        assert_eq!(out[i - 1], Event::CursorLeft);
        assert_eq!(i, out.len() - 1, "nothing after the cancel: {out:?}");
    }

    #[test]
    fn cancelled_touch_does_not_click() {
        let out = run(&[
            (ev(1, TouchPhase::Started, 10.0, 10.0), 0),
            (ev(1, TouchPhase::Cancelled, 10.0, 10.0), 20),
        ]);
        assert_eq!(
            &out[2..],
            &[Event::CursorLeft, Event::MouseReleased(MouseButton::Left)]
        );
    }
}
