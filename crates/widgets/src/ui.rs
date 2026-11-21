//! Persistent UI state shared by widgets, text metrics and the clipboard seam.

use crate::{
    a11y::{A11yNode, Role},
    motion::{Animatable, Presence, Spring, Transition},
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
    scroll: HashMap<u64, f32>,
    edits: HashMap<u64, Edit>,
    popup: Option<u64>,
    now: u64,
    anims: HashMap<u64, Anim>,
    reduced_motion: bool,
    overlay: Vec<DrawCommand>,
    regions: Vec<(u64, Rectangle)>,
    hit: Option<(u64, Point)>,
    clip: Option<Rectangle>,
    histories: HashMap<u64, History>,
    slots: HashMap<u64, i64>,
    a11y: Vec<A11yNode>,
    viewport: Rectangle,
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
            reduced_motion: std::env::var("ALLOY_REDUCE_MOTION").is_ok_and(|v| v != "0"),
            overlay: Vec::new(),
            regions: Vec::new(),
            hit: None,
            clip: None,
            histories: HashMap::new(),
            slots: HashMap::new(),
            a11y: Vec::new(),
            viewport: Rectangle::new(
                Point::new(0.0, 0.0),
                Size::new(f32::INFINITY, f32::INFINITY),
            ),
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
    }
    /// True while any transition is still running; the runtime then keeps ticking.
    pub fn animating(&self) -> bool {
        let s = self.0.borrow();
        s.anims.values().any(|a| a.running(s.now))
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
    /// True when the user (or the app) asked for less motion: every
    /// transition then jumps to its end state, and nothing keeps ticking.
    pub fn reduced_motion(&self) -> bool {
        self.0.borrow().reduced_motion
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
        self.0.borrow().scroll.get(&id).copied().unwrap_or(0.0)
    }
    /// Set the scroll offset of `id` (clamped when next used).
    pub fn set_scroll_offset(&self, id: u64, offset: f32) {
        self.0.borrow_mut().scroll.insert(id, offset.max(0.0));
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
            Event::MouseReleased(_) => self.ui.set_pressed(None),
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
