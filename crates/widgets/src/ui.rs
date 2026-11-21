//! Persistent UI state shared by widgets, text metrics and the clipboard seam.

use crate::{
    a11y::{A11yNode, Role},
    motion::{Animatable, Presence, Spring, Transition},
    physics,
    text_edit::{Edit, History},
    util,
};
use alloy_ui_core::{
    DrawCommand, DrawList, Element, Event, Key, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_theme::{Easing, Theme};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Measures text. The renderer will provide real metrics; until then
/// [`ApproxMetrics`] gives deterministic estimates so layout and tests work
/// without a font.
pub trait TextMetrics {
    /// Width in logical pixels of `text` (single line) at `size`.
    fn width(&self, text: &str, size: f32) -> f32;
    /// Width at a given weight and family. Defaults to [`TextMetrics::width`];
    /// renderer-backed metrics override it so bold and monospace measure exactly.
    fn width_styled(
        &self,
        text: &str,
        size: f32,
        _weight: u16,
        _family: alloy_ui_core::FontFamily,
    ) -> f32 {
        self.width(text, size)
    }
}

/// Deterministic per-character width estimates. Not exact; never used for
/// final pixels once a renderer-backed implementation is installed.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApproxMetrics;

impl TextMetrics for ApproxMetrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| {
                let em = match c {
                    ' ' => 0.28,
                    'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '\'' | '!' | '|' => 0.28,
                    'm' | 'w' | 'M' | 'W' => 0.85,
                    c if (c as u32) >= 0x2E80 => 1.0,
                    c if c.is_uppercase() => 0.64,
                    _ => 0.54,
                };
                em * size
            })
            .sum()
    }
}

/// Memoises a [`TextMetrics`]: shaping a string costs far more than a hash
/// lookup, and a no-op event re-measures every visible label.
struct CachedMetrics {
    inner: Rc<dyn TextMetrics>,
    seen: RefCell<HashMap<u64, f32>>,
}

impl CachedMetrics {
    const CAP: usize = 8192;
    fn memo(&self, key: impl std::hash::Hash, f: impl FnOnce() -> f32) -> f32 {
        use std::hash::Hasher;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        let k = h.finish();
        if let Some(w) = self.seen.borrow().get(&k) {
            return *w;
        }
        let w = f();
        let mut m = self.seen.borrow_mut();
        if m.len() >= Self::CAP {
            m.clear();
        }
        m.insert(k, w);
        w
    }
}

impl TextMetrics for CachedMetrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.memo((text, size.to_bits()), || self.inner.width(text, size))
    }
    fn width_styled(
        &self,
        text: &str,
        size: f32,
        weight: u16,
        family: alloy_ui_core::FontFamily,
    ) -> f32 {
        let key = (text, size.to_bits(), weight, family as u8);
        self.memo(key, || self.inner.width_styled(text, size, weight, family))
    }
}

/// Clipboard seam. The platform crate supplies the real one.
pub trait Clipboard {
    /// Current text contents.
    fn get(&self) -> Option<String>;
    /// Replace contents.
    fn set(&self, text: String);
}

/// In-process clipboard used until the platform provides one (and in tests).
#[derive(Debug, Default)]
pub struct MemoryClipboard(RefCell<Option<String>>);

impl Clipboard for MemoryClipboard {
    fn get(&self) -> Option<String> {
        self.0.borrow().clone()
    }
    fn set(&self, text: String) {
        *self.0.borrow_mut() = Some(text);
    }
}

/// Pixels per notch of a mouse wheel, matching the platform layer's line height.
const WHEEL_LINE: f32 = 40.0;

/// Spring that carries scroll offsets: critically damped, so it never overshoots.
const SCROLL_SPRING: Spring = Spring {
    stiffness: 620.0,
    damping: 49.8, // 2 * sqrt(620)
};

struct Inner {
    theme: Rc<Theme>,
    metrics: Rc<dyn TextMetrics>,
    clipboard: Rc<dyn Clipboard>,
    focused: Option<u64>,
    order: Vec<u64>,
    /// Same ids as `order`, for O(1) duplicate checks while registering.
    order_seen: std::collections::HashSet<u64>,
    cursor: Point,
    pressed: Option<(i32, i32)>,
    scroll: HashMap<u64, f64>,
    edits: HashMap<u64, Edit>,
    popup: Option<u64>,
    now: u64,
    anims: HashMap<u64, Anim>,
    clocks: HashMap<u64, Clock>,
    memo: HashMap<u64, [f32; 4]>,
    frame: u64,
    snaps: HashMap<u64, Snapshot>,
    outgoing: HashMap<u64, Snapshot>,
    reduced_motion: bool,
    icon_pack: crate::IconPack,
    overlay: Vec<DrawCommand>,
    regions: Vec<(u64, Rectangle)>,
    hit: Option<(u64, Point)>,
    clip: Option<Rectangle>,
    histories: HashMap<u64, History>,
    slots: HashMap<u64, i64>,
    states: HashMap<u64, Box<dyn std::any::Any>>,
    mods: alloy_ui_core::Modifiers,
    a11y: Vec<A11yNode>,
    viewport: Rectangle,
    drag: Option<crate::dnd::DragState>,
    /// Gesture physics of each scrollable: overscroll and coasting.
    phys: HashMap<u64, Body>,
    /// Use the tick clock as wall time (tests); otherwise the real clock.
    tick_clock: bool,
    epoch: std::time::Instant,
    /// Rubber-band small (trackpad-sized) wheel deltas past the edges.
    elastic: bool,
    /// Shared elements ([`crate::Hero`]) drawn so far by the page being recorded.
    hero_collect: Option<Vec<HeroRec>>,
}

/// A shared element recorded while a page was drawn: where it was and what it drew.
#[derive(Clone)]
pub(crate) struct HeroRec {
    pub key: u64,
    pub rect: Rectangle,
    pub cmds: Rc<Vec<DrawCommand>>,
}

/// Gesture state of one scrollable. `raw` is how far (px, signed) the pointer
/// has pulled past an edge; what is drawn is [`physics::rubber_band`] of it.
#[derive(Clone, Copy, Default)]
struct Body {
    /// Velocity of the content position, px/s, while coasting.
    v: f32,
    /// Coasting on friction or springing back from an edge.
    flying: bool,
    raw: f32,
    /// Wall time of the last wheel or drag input, ms.
    last_input: u64,
    /// Largest legal offset and viewport length as of the last draw.
    max: f32,
    dim: f32,
    last_tick: u64,
}

/// One value (up to four components) moving toward a target, by timed tween
/// or by spring. `start` is `None` until the next tick so a tween begun by an
/// event always runs its full duration.
struct Anim {
    from: [f32; 4],
    to: [f32; 4],
    cur: [f32; 4],
    vel: [f32; 4],
    start: Option<u64>,
    last: u64,
    tr: Transition,
}

impl Anim {
    fn value(&self, now: u64) -> [f32; 4] {
        match self.tr {
            Transition::Spring(_) => self.cur,
            Transition::Tween { ms, easing } => {
                let Some(start) = self.start else {
                    return self.from;
                };
                if ms == 0 || now >= start + ms as u64 {
                    return self.to;
                }
                let k = easing.at((now - start) as f32 / ms as f32);
                [0, 1, 2, 3].map(|i| self.from[i] + (self.to[i] - self.from[i]) * k)
            }
        }
    }
    fn running(&self, now: u64) -> bool {
        match self.tr {
            Transition::Spring(_) => {
                !(0..4).all(|i| Spring::settled(self.cur[i], self.vel[i], self.to[i]))
            }
            Transition::Tween { ms, .. } => match self.start {
                _ if self.from == self.to => false,
                None => true,
                Some(s) => now < s + ms as u64,
            },
        }
    }
    fn advance(&mut self, now: u64) {
        match self.tr {
            Transition::Tween { .. } => {
                if self.start.is_none() {
                    self.start = Some(now);
                }
            }
            Transition::Spring(sp) => {
                let dt = (now.saturating_sub(self.last)) as f32 / 1000.0;
                self.last = now;
                if dt > 0.0 {
                    for i in 0..4 {
                        let (x, v) = sp.step(self.cur[i], self.vel[i], self.to[i], dt.min(0.1));
                        self.cur[i] = x;
                        self.vel[i] = v;
                    }
                }
                if !self.running(now) {
                    self.cur = self.to;
                    self.vel = [0.0; 4];
                }
            }
        }
    }
}

impl Inner {
    /// Advance every coasting scrollable to `now`.
    fn step_bodies(&mut self, now: u64) {
        let reduced = self.reduced_motion;
        let ids: Vec<u64> = self
            .phys
            .iter()
            .filter(|(_, b)| b.flying)
            .map(|(i, _)| *i)
            .collect();
        for id in ids {
            let b = self.phys[&id];
            let dt = (now.saturating_sub(b.last_tick)) as f32 / 1000.0;
            let off = self.scroll.get(&id).copied().unwrap_or(0.0) as f32;
            let x = off + b.raw;
            let (x, v, rest) = if reduced {
                (x.clamp(0.0, b.max), 0.0, true)
            } else if dt > 0.0 {
                physics::fling_step(x, b.v, 0.0, b.max, dt.min(0.05))
            } else {
                (x, b.v, false)
            };
            let off = x.clamp(0.0, b.max);
            self.scroll.insert(id, off as f64);
            self.anims.remove(&(0x5C01_0000_0000 ^ id));
            let b = self.phys.get_mut(&id).expect("listed above");
            b.last_tick = now;
            b.raw = if rest { 0.0 } else { x - off };
            b.v = if rest { 0.0 } else { v };
            b.flying = !rest;
        }
    }
}

/// A page's recorded draw commands, kept so it can be drawn on its way out.
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub page: u64,
    pub cmds: Rc<Vec<DrawCommand>>,
    /// Shared elements the page held, drawn apart from `cmds` so they can fly.
    pub heroes: Rc<Vec<HeroRec>>,
}

/// A one-shot timer started the first time its key is seen.
#[derive(Clone, Copy)]
struct Clock {
    /// Set by the first tick after the clock is created, like tween starts.
    start: Option<u64>,
    dur: u64,
}

/// Cheaply clonable handle to per-window widget state.
#[derive(Clone)]
pub struct Ui(Rc<RefCell<Inner>>);

impl Ui {
    /// New state using `theme`, approximate text metrics and an in-memory clipboard.
    pub fn new(theme: Theme) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            theme: Rc::new(theme),
            metrics: Rc::new(ApproxMetrics),
            clipboard: Rc::new(MemoryClipboard::default()),
            focused: None,
            order: Vec::new(),
            order_seen: Default::default(),
            // Off-screen until the first pointer event, so nothing starts hovered.
            cursor: Point::new(-1.0e6, -1.0e6),
            pressed: None,
            scroll: HashMap::new(),
            edits: HashMap::new(),
            popup: None,
            now: 0,
            anims: HashMap::new(),
            icon_pack: crate::IconPack::default(),
            clocks: HashMap::new(),
            memo: HashMap::new(),
            frame: 0,
            snaps: HashMap::new(),
            outgoing: HashMap::new(),
            reduced_motion: std::env::var("ALLOY_REDUCE_MOTION").is_ok_and(|v| v != "0"),
            overlay: Vec::new(),
            regions: Vec::new(),
            hit: None,
            clip: None,
            histories: HashMap::new(),
            slots: HashMap::new(),
            states: HashMap::new(),
            mods: Default::default(),
            a11y: Vec::new(),
            viewport: Rectangle::new(
                Point::new(0.0, 0.0),
                Size::new(f32::INFINITY, f32::INFINITY),
            ),
            drag: None,
            phys: HashMap::new(),
            tick_clock: false,
            epoch: std::time::Instant::now(),
            elastic: true,
            hero_collect: None,
        })))
    }

    /// The current theme.
    pub fn theme(&self) -> Rc<Theme> {
        self.0.borrow().theme.clone()
    }
    /// Swap the theme (e.g. when the system appearance changes).
    pub fn set_theme(&self, theme: Theme) {
        self.0.borrow_mut().theme = Rc::new(theme);
    }
    /// Text metrics in use.
    pub fn metrics(&self) -> Rc<dyn TextMetrics> {
        self.0.borrow().metrics.clone()
    }
    /// Install renderer-backed text metrics.
    /// Widths are cached, so install it once, after fonts are loaded.
    pub fn set_metrics(&self, m: Rc<dyn TextMetrics>) {
        self.0.borrow_mut().metrics = Rc::new(CachedMetrics {
            inner: m,
            seen: RefCell::default(),
        });
    }
    /// Clipboard in use.
    pub fn clipboard(&self) -> Rc<dyn Clipboard> {
        self.0.borrow().clipboard.clone()
    }
    /// Install the platform clipboard.
    pub fn set_clipboard(&self, c: Rc<dyn Clipboard>) {
        self.0.borrow_mut().clipboard = c;
    }

    /// Id of the focused widget, if any.
    pub fn focused(&self) -> Option<u64> {
        self.0.borrow().focused
    }
    /// Whether `id` has keyboard focus.
    pub fn is_focused(&self, id: Option<u64>) -> bool {
        id.is_some() && self.focused() == id
    }
    /// Move keyboard focus (or clear it with `None`).
    pub fn set_focus(&self, id: Option<u64>) {
        self.0.borrow_mut().focused = id;
    }
    /// Focus the next widget in draw order, wrapping around.
    pub fn focus_next(&self) {
        self.step_focus(1);
    }
    /// Focus the previous widget in draw order, wrapping around.
    pub fn focus_prev(&self) {
        self.step_focus(-1);
    }
    fn step_focus(&self, dir: i32) {
        let mut s = self.0.borrow_mut();
        let n = s.order.len() as i32;
        if n == 0 {
            return;
        }
        let cur = s
            .focused
            .and_then(|f| s.order.iter().position(|&x| x == f))
            .map(|i| i as i32);
        let next = match cur {
            Some(i) => (i + dir).rem_euclid(n),
            None if dir > 0 => 0,
            None => n - 1,
        };
        s.focused = Some(s.order[next as usize]);
    }
    /// Record an accessibility node for the widget being drawn; `f` fills in
    /// the details. Focus state is filled from `Ui`.
    pub(crate) fn describe(
        &self,
        role: Role,
        label: &str,
        bounds: &Rectangle,
        f: impl FnOnce(&mut A11yNode),
    ) {
        if self.culled(bounds) {
            return;
        }
        let mut n = A11yNode::new(role, label, *bounds);
        f(&mut n);
        n.focused = n.id.is_some() && self.focused() == n.id;
        self.0.borrow_mut().a11y.push(n);
    }
    /// The accessibility nodes of the last drawn frame, in reading order.
    pub fn accessibility(&self) -> Vec<A11yNode> {
        self.0.borrow().a11y.clone()
    }
    /// Focusable widgets register themselves while drawing, in tab order.
    pub(crate) fn register(&self, id: u64) {
        let mut s = self.0.borrow_mut();
        if s.order_seen.insert(id) {
            s.order.push(id);
        }
    }
    pub(crate) fn begin_frame(&self) {
        let mut s = self.0.borrow_mut();
        s.frame += 1;
        s.order.clear();
        s.order_seen.clear();
        s.overlay.clear();
        s.regions.clear();
        s.a11y.clear();
    }

    /// The innermost scroll viewport being drawn, if any. Widgets that are
    /// entirely outside it can skip emitting commands.
    pub(crate) fn clip(&self) -> Option<Rectangle> {
        self.0.borrow().clip
    }
    pub(crate) fn set_clip(&self, c: Option<Rectangle>) {
        self.0.borrow_mut().clip = c;
    }
    /// True when `r` lies wholly outside the current scroll viewport.
    pub(crate) fn culled(&self, r: &Rectangle) -> bool {
        self.clip().is_some_and(|c| {
            r.origin.x >= c.origin.x + c.size.width
                || r.origin.y >= c.origin.y + c.size.height
                || r.origin.x + r.size.width <= c.origin.x
                || r.origin.y + r.size.height <= c.origin.y
        })
    }

    /// Monotonic milliseconds of the last `Event::Tick`.
    pub fn now(&self) -> u64 {
        self.0.borrow().now
    }
    /// Advance the animation clock (called by [`Root`] on `Event::Tick`).
    pub(crate) fn tick(&self, now: u64) {
        let mut s = self.0.borrow_mut();
        s.now = now;
        for a in s.anims.values_mut() {
            a.advance(now);
        }
        for c in s.clocks.values_mut() {
            c.start.get_or_insert(now);
        }
        s.step_bodies(now);
    }
    /// True while any transition is still running; the runtime then keeps ticking.
    pub fn animating(&self) -> bool {
        let s = self.0.borrow();
        s.anims.values().any(|a| a.running(s.now))
            || s.phys.values().any(|b| b.flying || b.raw != 0.0)
            || s.clocks
                .values()
                .any(|c| c.start.map_or(true, |st| s.now < st + c.dur))
    }
    /// Current value of the transition `key`, moving toward `target` over the
    /// theme's fast motion duration. The first call returns `target` at once.
    pub fn ease(&self, key: u64, target: f32) -> f32 {
        let t = self.theme();
        self.ease_with(key, target, t.motion.fast_ms as u64, t.motion.easing)
    }
    /// Like [`Ui::ease`] with an explicit duration and easing. A duration of
    /// zero jumps straight to the target.
    pub fn ease_with(&self, key: u64, target: f32, dur_ms: u64, easing: Easing) -> f32 {
        self.animate(key, target, Transition::eased(dur_ms as u32, easing))
    }
    /// Move any [`Animatable`] value (number, point, colour) toward `target`
    /// by tween or spring, and return where it is now. The first call for a
    /// `key` returns `target` at once; retargeting mid-flight continues from
    /// the current value (and, for springs, the current velocity). Honours
    /// [`Ui::reduced_motion`] by jumping.
    pub fn animate<T: Animatable>(&self, key: u64, target: T, tr: Transition) -> T {
        let mut s = self.0.borrow_mut();
        let now = s.now;
        let reduced = s.reduced_motion;
        let to = target.to_parts();
        let a = s.anims.entry(key).or_insert(Anim {
            from: to,
            to,
            cur: to,
            vel: [0.0; 4],
            start: Some(now),
            last: now,
            tr,
        });
        if a.to != to {
            let cur = a.value(now);
            let jump = reduced || matches!(tr, Transition::Tween { ms: 0, .. });
            *a = Anim {
                from: cur,
                to,
                cur: if jump { to } else { cur },
                vel: if matches!(tr, Transition::Spring(_)) && !jump {
                    a.vel
                } else {
                    [0.0; 4]
                },
                start: if jump { Some(now) } else { None },
                last: now,
                tr: if jump { Transition::ms(0) } else { tr },
            };
        }
        T::from_parts(a.value(now))
    }
    /// A spring-driven number: like [`Ui::ease`] but physical, with velocity
    /// carried across retargets so rapid hover in and out never snaps.
    pub fn spring(&self, key: u64, target: f32, spring: Spring) -> f32 {
        self.animate(key, target, Transition::Spring(spring))
    }
    /// Put the spring `key` at `value` moving at `velocity` (units/s), so the next
    /// [`Ui::spring`] call toward a new target carries on from there. This is how a
    /// drag hands over to a spring on release.
    pub fn spring_release(&self, key: u64, value: f32, velocity: f32, spring: Spring) {
        let mut s = self.0.borrow_mut();
        let now = s.now;
        s.anims.insert(
            key,
            Anim {
                from: [value, 0.0, 0.0, 0.0],
                to: [value, 0.0, 0.0, 0.0],
                cur: [value, 0.0, 0.0, 0.0],
                vel: [velocity, 0.0, 0.0, 0.0],
                start: Some(now),
                last: now,
                tr: Transition::Spring(spring),
            },
        );
    }
    /// Enter and exit progress for something that appears and disappears.
    /// `present` flips the target; `progress` eases 0 to 1 over `ms`, and
    /// `mounted` stays true until an exit has fully played, so the caller can
    /// keep drawing the outgoing content.
    pub fn presence(&self, key: u64, present: bool, ms: u32) -> Presence {
        let progress = self.animate(key, present as u8 as f32, Transition::ms(ms));
        Presence {
            progress,
            mounted: present || progress > 0.002,
        }
    }
    /// Frames drawn so far. A widget that notices it was not drawn last frame
    /// knows it has just appeared.
    pub fn frame(&self) -> u64 {
        self.0.borrow().frame
    }
    /// Start collecting shared elements for a page; returns the collector it replaced.
    pub(crate) fn hero_begin(&self) -> Option<Vec<HeroRec>> {
        self.0.borrow_mut().hero_collect.replace(Vec::new())
    }
    /// Stop collecting, restore `outer`, and return what the page held.
    pub(crate) fn hero_end(&self, outer: Option<Vec<HeroRec>>) -> Vec<HeroRec> {
        std::mem::replace(&mut self.0.borrow_mut().hero_collect, outer).unwrap_or_default()
    }
    /// Hand a shared element to the page being recorded. False if there is none, and
    /// the element should simply draw itself.
    pub(crate) fn hero_push(&self, rec: HeroRec) -> bool {
        match self.0.borrow_mut().hero_collect.as_mut() {
            Some(v) => {
                v.push(rec);
                true
            }
            None => false,
        }
    }
    /// Store `snap` as the latest snapshot for `key`, returning the previous one.
    pub(crate) fn swap_snapshot(&self, key: u64, snap: Snapshot) -> Option<Snapshot> {
        self.0.borrow_mut().snaps.insert(key, snap)
    }
    /// The page on its way out for `key`, if a transition is running.
    pub(crate) fn outgoing(&self, key: u64) -> Option<Snapshot> {
        self.0.borrow().outgoing.get(&key).cloned()
    }
    /// Set or clear (with `None`) the outgoing page for `key`.
    pub(crate) fn set_outgoing(&self, key: u64, snap: Option<Snapshot>) {
        let mut s = self.0.borrow_mut();
        match snap {
            Some(v) => s.outgoing.insert(key, v),
            None => s.outgoing.remove(&key),
        };
    }

    /// A choreographed sequence of `total_ms` that starts the first time it is
    /// seen and starts over whenever `trigger` changes (pass anything that
    /// identifies the state, e.g. the selected page).
    pub fn timeline(&self, key: u64, trigger: u64, total_ms: u32) -> crate::motion::Timeline {
        let packed = crate::motion::pack(trigger);
        let mk = key ^ 0x7131;
        if self.memo(mk) != packed {
            self.set_memo(mk, packed);
            self.restart_clock(key);
        }
        let (el, _) = self.clock(key, total_ms as u64);
        crate::motion::Timeline {
            elapsed_ms: el as f32,
            total_ms: total_ms as f32,
        }
    }
    /// A value travelling through `frames` over `ms`, replaying whenever
    /// `trigger` changes (and once when first seen).
    pub fn keyframes<T: Animatable>(
        &self,
        key: u64,
        trigger: u64,
        ms: u32,
        frames: &crate::motion::Keyframes<T>,
    ) -> T {
        let tl = self.timeline(key, trigger, ms);
        frames.sample(tl.progress())
    }

    /// A one-shot clock: milliseconds since `key` was first seen (or since
    /// [`Ui::restart_clock`]), capped at `dur_ms`, plus whether it is still
    /// running. Drives timelines, keyframes and staggered entrances. Under
    /// reduced motion it is already finished.
    pub fn clock(&self, key: u64, dur_ms: u64) -> (u64, bool) {
        let mut s = self.0.borrow_mut();
        if s.reduced_motion {
            return (dur_ms, false);
        }
        let now = s.now;
        let c = s.clocks.entry(key).or_insert(Clock {
            start: None,
            dur: dur_ms,
        });
        c.dur = c.dur.max(dur_ms);
        match c.start {
            // Not ticked yet: it starts at zero on the next tick.
            None => (0, true),
            Some(st) => (now.saturating_sub(st).min(dur_ms), now < st + dur_ms),
        }
    }
    /// Start the clock `key` again from zero.
    pub fn restart_clock(&self, key: u64) {
        let mut s = self.0.borrow_mut();
        s.clocks.insert(
            key,
            Clock {
                start: None,
                dur: 0,
            },
        );
    }
    /// Remembered four numbers for `key` (zeros if never set): per-widget
    /// scratch such as the point a ripple started from.
    pub fn memo(&self, key: u64) -> [f32; 4] {
        self.0.borrow().memo.get(&key).copied().unwrap_or([0.0; 4])
    }
    /// Remember four numbers for `key`.
    pub fn set_memo(&self, key: u64, v: [f32; 4]) {
        self.0.borrow_mut().memo.insert(key, v);
    }

    /// True when the user (or the app) asked for less motion: every
    /// transition then jumps to its end state, and nothing keeps ticking.
    pub fn reduced_motion(&self) -> bool {
        self.0.borrow().reduced_motion
    }
    /// The icon pack icons are drawn from unless they name one.
    pub fn icon_pack(&self) -> crate::IconPack {
        self.0.borrow().icon_pack
    }
    /// Choose the window's icon pack.
    pub fn set_icon_pack(&self, pack: crate::IconPack) {
        self.0.borrow_mut().icon_pack = pack;
    }
    /// Turn reduced motion on or off. It starts on when the environment
    /// variable `ALLOY_REDUCE_MOTION` is set to anything but `0`.
    pub fn set_reduced_motion(&self, on: bool) {
        self.0.borrow_mut().reduced_motion = on;
    }
    /// A key for a per-widget transition, derived from where the widget is.
    pub(crate) fn anim_key(bounds: &Rectangle, kind: u64) -> u64 {
        let (x, y) = util::key_of(bounds);
        ((x as u32 as u64) << 32 | y as u32 as u64) ^ kind.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }

    /// Draw `cmds` above everything else this frame and make `region`
    /// clickable for `owner` even where later siblings overlap it.
    pub(crate) fn overlay(
        &self,
        owner: Option<u64>,
        region: Option<Rectangle>,
        cmds: Vec<DrawCommand>,
    ) {
        let mut s = self.0.borrow_mut();
        s.overlay.extend(cmds);
        if let (Some(o), Some(r)) = (owner, region) {
            s.regions.push((o, r));
        }
    }
    fn take_overlay(&self) -> Vec<DrawCommand> {
        std::mem::take(&mut self.0.borrow_mut().overlay)
    }
    fn overlay_owner_at(&self, p: Point) -> Option<u64> {
        let s = self.0.borrow();
        s.regions
            .iter()
            .rev()
            .find(|(_, r)| r.contains(p))
            .map(|(o, _)| *o)
    }
    fn set_hit(&self, hit: Option<(u64, Point)>) {
        self.0.borrow_mut().hit = hit;
    }
    /// The overlay press addressed to `owner`, if any (consumes it).
    pub(crate) fn take_hit(&self, owner: u64) -> Option<Point> {
        let mut s = self.0.borrow_mut();
        match s.hit {
            Some((o, p)) if o == owner => {
                s.hit = None;
                Some(p)
            }
            _ => None,
        }
    }

    pub(crate) fn drag_state<R>(
        &self,
        f: impl FnOnce(&mut Option<crate::dnd::DragState>) -> R,
    ) -> R {
        f(&mut self.0.borrow_mut().drag)
    }
    /// True while a drag started by [`Draggable`](crate::Draggable) is in progress
    /// (the pointer has moved past the start threshold and the button is still down).
    pub fn dragging(&self) -> bool {
        self.0.borrow().drag.as_ref().is_some_and(|d| d.active)
    }

    /// The window area of the last drawn frame, so popups can stay on screen.
    /// Unbounded until the first frame.
    pub fn viewport(&self) -> Rectangle {
        self.0.borrow().viewport
    }
    pub(crate) fn set_viewport(&self, r: Rectangle) {
        self.0.borrow_mut().viewport = r;
    }

    /// Last known cursor position (used for hover styling).
    pub fn cursor(&self) -> Point {
        self.0.borrow().cursor
    }
    pub(crate) fn set_cursor(&self, p: Point) {
        self.0.borrow_mut().cursor = p;
    }
    pub(crate) fn pressed(&self) -> Option<(i32, i32)> {
        self.0.borrow().pressed
    }
    pub(crate) fn set_pressed(&self, key: Option<(i32, i32)>) {
        self.0.borrow_mut().pressed = key;
    }
    pub(crate) fn hovered(&self, bounds: &Rectangle) -> bool {
        bounds.contains(self.cursor())
    }
    pub(crate) fn is_pressed(&self, bounds: &Rectangle) -> bool {
        self.pressed() == Some(util::key_of(bounds))
    }

    /// Scroll offset of the scrollable `id`.
    pub fn scroll_offset(&self, id: u64) -> f32 {
        self.scroll_offset_f64(id) as f32
    }
    /// Set the scroll offset of `id` (clamped when next used).
    pub fn set_scroll_offset(&self, id: u64, offset: f32) {
        self.set_scroll_offset_f64(id, offset as f64);
    }
    /// Full-precision scroll offset. An `f32` stops resolving single pixels past 16.7 million,
    /// which a list of a million 28-pixel rows passes.
    pub fn scroll_offset_f64(&self, id: u64) -> f64 {
        self.0.borrow().scroll.get(&id).copied().unwrap_or(0.0)
    }
    /// Set the full-precision scroll offset of `id`.
    pub fn set_scroll_offset_f64(&self, id: u64, offset: f64) {
        self.0.borrow_mut().scroll.insert(id, offset.max(0.0));
    }

    fn scroll_key(id: u64) -> u64 {
        0x5C01_0000_0000 ^ id
    }
    /// Where the scrollable `id` is *drawn*: its target offset followed by a critically
    /// damped spring (about 150 ms to rest, no overshoot), so wheel notches, keys and
    /// jumps glide. A burst of wheel events retargets the spring without resetting its
    /// velocity, which is what makes quick successive notches feel like momentum.
    /// Jumps at once under reduced motion.
    pub fn scroll_shown(&self, id: u64, max: f32) -> f32 {
        let target = self.scroll_offset(id).clamp(0.0, max.max(0.0));
        self.spring(Self::scroll_key(id), target, SCROLL_SPRING)
    }
    /// Apply a wheel delta (positive scrolls toward the top) to the target offset of `id`.
    /// Returns true if the target moved, i.e. the event was used.
    pub fn scroll_wheel(&self, id: u64, dy: f32, max: f32) -> bool {
        let before = self.scroll_offset(id).clamp(0.0, max.max(0.0));
        let next = (before - dy).clamp(0.0, max.max(0.0));
        // A notched mouse wheel reports whole lines (multiples of the platform line height),
        // which glide on the spring. Trackpads and touch report arbitrary pixel deltas that the
        // OS has already smoothed and given momentum, so they track 1:1; gliding on top of that
        // would only add lag.
        let precise = dy.abs() > 0.0
            && (dy.abs() % WHEEL_LINE) > 0.01
            && (WHEEL_LINE - dy.abs() % WHEEL_LINE) > 0.01;
        if precise {
            self.scroll_jump(id, next, max);
        } else {
            self.set_scroll_offset(id, next);
        }
        next != before
    }
    /// Glide to `offset` (clamped to `0..=max`).
    pub fn scroll_to(&self, id: u64, offset: f32, max: f32) {
        self.set_scroll_offset(id, offset.clamp(0.0, max.max(0.0)));
    }
    /// Move to `offset` immediately, with no glide: for dragging a scrollbar thumb, where the
    /// content must stay under the pointer.
    pub fn scroll_jump(&self, id: u64, offset: f32, max: f32) {
        self.scroll_jump_f64(id, offset as f64, max as f64);
    }
    /// [`Ui::scroll_jump`] at full precision, for lists taller than an `f32` can resolve.
    pub fn scroll_jump_f64(&self, id: u64, offset: f64, max: f64) {
        self.set_scroll_offset_f64(id, offset.clamp(0.0, max.max(0.0)));
        self.0.borrow_mut().anims.remove(&Self::scroll_key(id));
    }

    /// Wall-clock milliseconds, for gesture timing. It follows the tick clock when
    /// [`Ui::use_tick_clock`] is on (tests), so velocities are deterministic.
    pub fn wall_ms(&self) -> u64 {
        let s = self.0.borrow();
        if s.tick_clock {
            s.now
        } else {
            s.epoch.elapsed().as_millis() as u64
        }
    }
    /// Make [`Ui::wall_ms`] follow `Event::Tick`, so tests control gesture time.
    pub fn use_tick_clock(&self) {
        self.0.borrow_mut().tick_clock = true;
    }
    /// Turn edge rubber-banding of wheel scrolling on or off (default on). Only
    /// small deltas, as trackpads send, stretch past an edge; mouse-wheel
    /// notches always stop at it.
    pub fn set_elastic_scroll(&self, on: bool) {
        self.0.borrow_mut().elastic = on;
    }
    /// Where the scrollable `id` is drawn, including any rubber-band stretch
    /// past its edges (negative at the top, beyond `max` at the bottom). `dim`
    /// is the viewport length, which sets how far the band can stretch.
    pub fn scroll_view(&self, id: u64, max: f32, dim: f32) -> f32 {
        self.phys_sync(id, max, dim);
        let flying = self.0.borrow().phys.get(&id).is_some_and(|b| b.flying);
        let base = if flying {
            self.scroll_offset(id).clamp(0.0, max.max(0.0))
        } else {
            self.scroll_shown(id, max)
        };
        base + self.scroll_overshoot(id)
    }
    /// The drawn stretch past an edge for `id`: zero when inside.
    pub fn scroll_overshoot(&self, id: u64) -> f32 {
        let s = self.0.borrow();
        s.phys
            .get(&id)
            .map_or(0.0, |b| physics::rubber_band(b.raw, b.dim.max(1.0)))
    }
    /// Remember the geometry for coasting, and hand a released stretch to the
    /// integrator once the wheel or pointer has been quiet for a moment.
    fn phys_sync(&self, id: u64, max: f32, dim: f32) {
        let wall = self.wall_ms();
        let mut s = self.0.borrow_mut();
        let now = s.now;
        let Some(b) = s.phys.get_mut(&id) else { return };
        b.max = max.max(0.0);
        b.dim = dim;
        if !b.flying && b.raw != 0.0 && wall.saturating_sub(b.last_input) > physics::GESTURE_GAP_MS
        {
            b.flying = true;
            b.v = 0.0;
            b.last_tick = now;
        }
    }
    /// The phase of the scroll gesture on `id`, inferred from input timing.
    pub fn scroll_phase(&self, id: u64) -> physics::Phase {
        let wall = self.wall_ms();
        let s = self.0.borrow();
        let Some(b) = s.phys.get(&id) else {
            return physics::Phase::Idle;
        };
        if b.flying {
            physics::Phase::Coasting
        } else if wall.saturating_sub(b.last_input) <= physics::GESTURE_GAP_MS {
            physics::Phase::Changed
        } else {
            physics::Phase::Idle
        }
    }
    /// Like [`Ui::scroll_wheel`] but the content can be pulled past an edge,
    /// resisting, and springs back when input stops. `force` stretches for any
    /// delta size (touch-style dragging); otherwise only trackpad-sized deltas
    /// (under 30 px) do. Returns true if anything moved.
    pub fn scroll_wheel_elastic(&self, id: u64, dy: f32, max: f32, dim: f32, force: bool) -> bool {
        let wall = self.wall_ms();
        let mut s = self.0.borrow_mut();
        let max = max.max(0.0);
        let elastic = (force || (s.elastic && dy.abs() < 30.0)) && !s.reduced_motion;
        let off = s
            .scroll
            .get(&id)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, max as f64) as f32;
        let b = s.phys.entry(id).or_default();
        let old_raw = b.raw;
        b.flying = false;
        b.v = 0.0;
        let virt = off + b.raw - dy;
        let (noff, raw) = if virt < 0.0 {
            (0.0, virt)
        } else if virt > max {
            (max, virt - max)
        } else {
            (virt, 0.0)
        };
        b.raw = if elastic { raw } else { 0.0 };
        b.last_input = wall;
        b.max = max;
        b.dim = dim;
        let raw_changed = b.raw != old_raw;
        s.scroll.insert(id, noff as f64);
        // A notched mouse wheel reports whole lines (multiples of the platform line height),
        // which glide on the spring. Trackpads and touch report arbitrary pixel deltas that the
        // OS has already smoothed and given momentum, so they track 1:1; gliding on top of that
        // would only add lag.
        let precise = dy.abs() > 0.0
            && (dy.abs() % WHEEL_LINE) > 0.01
            && (WHEEL_LINE - dy.abs() % WHEEL_LINE) > 0.01;
        if force || precise {
            // Dragging or a trackpad: the content stays under the pointer, with no glide.
            s.anims.remove(&Self::scroll_key(id));
        }
        noff != off || raw_changed
    }
    /// Stop `id` coasting, keeping where it is (a finger landing on a moving list).
    pub fn scroll_catch(&self, id: u64) {
        if let Some(b) = self.0.borrow_mut().phys.get_mut(&id) {
            b.flying = false;
            b.v = 0.0;
        }
    }
    /// Let go of `id` while moving at `velocity` px/s (positive scrolls toward
    /// the bottom): it coasts on friction and is caught at the edges.
    pub fn scroll_fling(&self, id: u64, velocity: f32) {
        let now = self.0.borrow().now;
        let mut s = self.0.borrow_mut();
        let b = s.phys.entry(id).or_default();
        b.flying = true;
        b.v = velocity;
        b.last_tick = now;
    }

    /// Id of the widget whose popup (e.g. an open pick list) is showing.
    pub fn popup(&self) -> Option<u64> {
        self.0.borrow().popup
    }
    /// Open the popup of `id`, or close any popup with `None`.
    pub fn set_popup(&self, id: Option<u64>) {
        self.0.borrow_mut().popup = id;
    }

    /// Run `f` on the undo history of text field `id`. `f` must not call back into `Ui`.
    pub(crate) fn with_history<R>(&self, id: u64, f: impl FnOnce(&mut History) -> R) -> R {
        f(self.0.borrow_mut().histories.entry(id).or_default())
    }

    /// A small integer remembered for widget `id` (e.g. the month a date picker shows).
    pub(crate) fn slot(&self, id: u64) -> Option<i64> {
        self.0.borrow().slots.get(&id).copied()
    }
    pub(crate) fn set_slot(&self, id: u64, v: Option<i64>) {
        let mut s = self.0.borrow_mut();
        match v {
            Some(v) => s.slots.insert(id, v),
            None => s.slots.remove(&id),
        };
    }

    /// The modifier keys as of the last key event. Mouse events carry none, so widgets that
    /// need Shift/Ctrl-click read it here. The platform shell sends a `KeyReleased` of the
    /// pseudo key `Named("Modifiers")` whenever the modifier state changes.
    pub fn modifiers(&self) -> alloy_ui_core::Modifiers {
        self.0.borrow().mods
    }
    pub(crate) fn set_modifiers(&self, m: alloy_ui_core::Modifiers) {
        self.0.borrow_mut().mods = m;
    }
    /// Run `f` on the typed state of widget `id` (created with `T::default()` on first use).
    /// `f` must not call back into `Ui`. For state too rich for a slot: a grid's selection,
    /// a find query, a rich-text caret.
    pub(crate) fn state<T: std::any::Any + Default, R>(
        &self,
        id: u64,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        let mut s = self.0.borrow_mut();
        let b = s.states.entry(id).or_insert_with(|| Box::new(T::default()));
        if !b.is::<T>() {
            *b = Box::new(T::default());
        }
        f(b.downcast_mut::<T>().expect("type checked above"))
    }

    pub(crate) fn edit(&self, id: u64) -> Edit {
        self.0.borrow().edits.get(&id).cloned().unwrap_or_default()
    }
    pub(crate) fn set_edit(&self, id: u64, e: Edit) {
        self.0.borrow_mut().edits.insert(id, e);
    }
}

/// Wraps the root element: tracks the cursor, resets focus bookkeeping each
/// frame, clears focus on clicks that hit nothing focusable, and handles
/// Tab / Shift+Tab navigation.
pub struct Root<M> {
    ui: Ui,
    child: Vec<Element<M>>,
}

impl<M: 'static> Root<M> {
    /// Wrap `child` as the root of a window's tree.
    pub fn new(ui: &Ui, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            child: vec![child],
        }
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Root<M> {
    fn style(&self) -> alloy_ui_core::style::Style {
        self.child[0].0.style()
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        util::arrange_widget(self, bounds)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        if let Event::Tick(ms) = event {
            self.ui.tick(*ms);
        }
        // A press on an overlay (open pick list) goes to its owner even if
        // later siblings cover the spot: deliver it with the pointer parked
        // off-screen so nothing else claims it.
        if let Event::MousePressed(_) = event {
            if let Some(owner) = self.ui.overlay_owner_at(cursor) {
                self.ui.set_cursor(cursor);
                self.ui.set_hit(Some((owner, cursor)));
                let off = Point::new(-1.0e6, -1.0e6);
                util::dispatch(&mut self.child, event, layout, off, out);
                self.ui.set_hit(None);
                return true;
            }
        }
        match event {
            Event::KeyPressed(_, m) | Event::KeyReleased(_, m) => self.ui.set_modifiers(*m),
            Event::CursorMoved(p) => self.ui.set_cursor(*p),
            Event::CursorLeft => self.ui.set_cursor(Point::new(-1.0e6, -1.0e6)),
            Event::MousePressed(_) => {
                self.ui.set_cursor(cursor);
                self.ui.set_focus(None);
            }
            _ => {}
        }
        let consumed = util::dispatch(&mut self.child, event, layout, cursor, out);
        match event {
            Event::MouseReleased(_) => {
                self.ui.set_pressed(None);
                // Every drop target has now seen the release: the drag is over.
                self.ui.drag_state(|d| *d = None);
            }
            Event::KeyPressed(Key::Tab, m) if !consumed => {
                if m.shift {
                    self.ui.focus_prev();
                } else {
                    self.ui.focus_next();
                }
                return true;
            }
            _ => {}
        }
        consumed
    }
    fn animating(&self) -> bool {
        self.ui.animating() || util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        self.ui.begin_frame();
        self.ui.set_viewport(layout.bounds);
        util::draw_children(&self.child, layout, out);
        for c in self.ui.take_overlay() {
            out.push(c);
        }
    }
}
