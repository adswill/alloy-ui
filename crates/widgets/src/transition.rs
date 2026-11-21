//! Transitions between layouts and pages: [`Glide`] (layout and shared-element
//! motion), [`Enter`] (entrance with a delay, for staggered lists) and
//! [`Route`] (page changes).

use crate::{
    motion::Transition,
    ui::{Snapshot, Ui},
    util,
    xform::{transform, Xf},
};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use alloy_ui_theme::Easing;
use std::rc::Rc;

// ---------------------------------------------------------------- Glide

/// Animates its child's size and position when they change, so everything
/// around it reflows smoothly instead of jumping.
///
/// *Size*: the box reports an eased size, which neighbours lay out around.
/// *Position*: the child is drawn offset from where it now belongs and eases
/// into place (the FLIP technique), so reordering a list or moving a panel
/// glides. Give two widgets the same `key` in different places and the
/// element appears to travel between them: a shared-element transition.
///
/// Position tracking is in window coordinates, so avoid it inside a scrolling
/// area (use [`Glide::size_only`] there).
pub struct Glide<M> {
    ui: Ui,
    key: u64,
    tr: Transition,
    position: bool,
    child: Vec<Element<M>>,
}

impl<M: 'static> Glide<M> {
    /// Glide `child`; `key` identifies it across view rebuilds.
    pub fn new(ui: &Ui, key: u64, child: Element<M>) -> Self {
        let t = ui.theme();
        Self {
            ui: ui.clone(),
            key,
            tr: Transition::eased(t.motion.slow_ms + 60, Easing::QUART_OUT),
            position: true,
            child: vec![child],
        }
    }
    /// Animate size only.
    pub fn size_only(mut self) -> Self {
        self.position = false;
        self
    }
    /// Use this transition (default: slow, quart ease-out).
    pub fn transition(mut self, tr: Transition) -> Self {
        self.tr = tr;
        self
    }
    /// As an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
    fn natural(&self) -> Size {
        util::node_of(self.child[0].0.as_ref()).preferred_size()
    }
    fn shown_size(&self) -> (f32, f32) {
        let n = self.natural();
        self.ui
            .animate(self.key ^ 0x51, (n.width, n.height), self.tr)
    }
}

impl<M: 'static> Widget<M> for Glide<M> {
    fn style(&self) -> Style {
        let inner = self.child[0].0.style();
        let (w, h) = self.shown_size();
        Style {
            width: if matches!(inner.width, Length::Shrink | Length::Fixed(_)) {
                Length::Fixed(w)
            } else {
                inner.width
            },
            height: if matches!(inner.height, Length::Shrink | Length::Fixed(_)) {
                Length::Fixed(h)
            } else {
                inner.height
            },
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        // The child keeps its natural size while the box eases toward it.
        let n = self.natural();
        let inner = self.child[0].0.style();
        let size = Size::new(
            if matches!(inner.width, Length::Shrink | Length::Fixed(_)) {
                n.width
            } else {
                bounds.size.width
            },
            if matches!(inner.height, Length::Shrink | Length::Fixed(_)) {
                n.height
            } else {
                bounds.size.height
            },
        );
        Layout {
            bounds,
            children: vec![self.child[0].0.arrange(Rectangle::new(bounds.origin, size))],
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        util::dispatch(&mut self.child, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let (dx, dy) = if self.position {
            let p = self.ui.animate(self.key ^ 0x52, b.origin, self.tr);
            (p.x - b.origin.x, p.y - b.origin.y)
        } else {
            (0.0, 0.0)
        };
        let n = layout.children[0].bounds.size;
        let clip = n.width > b.size.width + 0.5 || n.height > b.size.height + 0.5;
        let mut tmp = DrawList::default();
        util::draw_children(&self.child, layout, &mut tmp);
        let xf = Xf::fade_move(1.0, dx, dy);
        if clip {
            out.push(DrawCommand::PushClip(Rectangle::new(
                Point::new(b.origin.x + dx, b.origin.y + dy),
                b.size,
            )));
        }
        for c in tmp.0 {
            out.push(if xf.is_identity() {
                c
            } else {
                transform(c, &xf)
            });
        }
        if clip {
            out.push(DrawCommand::PopClip);
        }
    }
}

// ---------------------------------------------------------------- Enter

/// Fades and rises its child into place when it first appears, after an
/// optional delay. Several with increasing delays make a staggered list.
/// It takes no space of its own and replays whenever it reappears after being
/// absent.
pub struct Enter<M> {
    ui: Ui,
    key: u64,
    delay_ms: u64,
    ms: u64,
    rise: f32,
    child: Vec<Element<M>>,
}

impl<M: 'static> Enter<M> {
    /// Enter `child` after `delay_ms`.
    pub fn new(ui: &Ui, key: u64, delay_ms: u64, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            key,
            delay_ms,
            ms: ui.theme().motion.slow_ms as u64 + 80,
            rise: 10.0,
            child: vec![child],
        }
    }
    /// How far it rises, in logical pixels (default 10).
    pub fn rise(mut self, px: f32) -> Self {
        self.rise = px;
        self
    }
    /// As an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Enter<M> {
    fn style(&self) -> Style {
        let inner = self.child[0].0.style();
        Style {
            width: inner.width,
            height: inner.height,
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        Layout {
            bounds,
            children: vec![self.child[0].0.arrange(bounds)],
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        util::dispatch(&mut self.child, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        // Absent last frame means it just appeared: start over.
        let seen = self.ui.memo(self.key);
        let frame = self.ui.frame();
        if (seen[0] as u64) + 1 < frame || seen[1] < 0.5 {
            self.ui.restart_clock(self.key);
        }
        self.ui.set_memo(self.key, [frame as f32, 1.0, 0.0, 0.0]);
        let total = self.delay_ms + self.ms;
        let (el, _running) = self.ui.clock(self.key, total);
        let p = (el.saturating_sub(self.delay_ms) as f32 / self.ms as f32).clamp(0.0, 1.0);
        let e = Easing::QUART_OUT.at(p);
        if p >= 0.999 {
            util::draw_children(&self.child, layout, out);
            return;
        }
        let mut tmp = DrawList::default();
        util::draw_children(&self.child, layout, &mut tmp);
        let xf = Xf::fade_move(e, 0.0, (1.0 - e) * self.rise);
        for c in tmp.0 {
            out.push(transform(c, &xf));
        }
    }
}

// ---------------------------------------------------------------- Route

/// How one page replaces another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RouteKind {
    /// Cross-fade only.
    Fade,
    /// Fade plus a short slide in the direction of travel. The default.
    #[default]
    SlideFade,
    /// The pages travel the full width together, like a navigation stack.
    Slide,
    /// The new page settles in from slightly smaller; the old one fades away.
    Scale,
}

/// Shows one page and animates changes of `page`: the page on its way out is
/// drawn from its last recorded frame (so it needs no extra view), the new
/// one arrives live. A larger `page` number than before moves forward.
pub struct Route<M> {
    ui: Ui,
    key: u64,
    page: u64,
    kind: RouteKind,
    ms: u64,
    child: Vec<Element<M>>,
}

impl<M: 'static> Route<M> {
    /// Show `child` as page number `page`.
    pub fn new(ui: &Ui, key: u64, page: u64, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            key,
            page,
            kind: RouteKind::default(),
            ms: ui.theme().motion.slow_ms as u64 + 90,
            child: vec![child],
        }
    }
    /// Pick the transition.
    pub fn kind(mut self, k: RouteKind) -> Self {
        self.kind = k;
        self
    }
    /// As an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Route<M> {
    fn style(&self) -> Style {
        let inner = self.child[0].0.style();
        Style {
            width: inner.width,
            height: inner.height,
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        Layout {
            bounds,
            children: vec![self.child[0].0.arrange(bounds)],
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        util::dispatch(&mut self.child, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let mut tmp = DrawList::default();
        util::draw_children(&self.child, layout, &mut tmp);
        let now_cmds = Rc::new(tmp.0);
        // Record this page; if the previous record was another page, it leaves.
        let prev = self.ui.swap_snapshot(
            self.key,
            Snapshot {
                page: self.page,
                cmds: now_cmds.clone(),
            },
        );
        if let Some(prev) = prev {
            if prev.page != self.page && !self.ui.reduced_motion() {
                self.ui.set_outgoing(self.key, Some(prev));
                self.ui.restart_clock(self.key ^ 0x77);
            }
        }
        let Some(old) = self.ui.outgoing(self.key) else {
            out.0.extend(now_cmds.iter().cloned());
            return;
        };
        let (el, running) = self.ui.clock(self.key ^ 0x77, self.ms);
        if !running {
            self.ui.set_outgoing(self.key, None);
            out.0.extend(now_cmds.iter().cloned());
            return;
        }
        let p = el as f32 / self.ms as f32;
        let dir = if self.page >= old.page { 1.0 } else { -1.0 };
        let e = Easing::QUART_OUT.at(p);
        let c = Point::new(
            b.origin.x + b.size.width * 0.5,
            b.origin.y + b.size.height * 0.5,
        );
        // (old, new) transforms.
        let (xo, xn) = match self.kind {
            RouteKind::Fade => (
                Xf::fade_move(1.0 - Easing::SINE_IN_OUT.at((p * 1.6).min(1.0)), 0.0, 0.0),
                Xf::fade_move(
                    Easing::SINE_IN_OUT.at(((p - 0.25) / 0.75).clamp(0.0, 1.0)),
                    0.0,
                    0.0,
                ),
            ),
            RouteKind::SlideFade => {
                let d = 28.0;
                (
                    Xf::fade_move(
                        1.0 - Easing::SINE_IN_OUT.at((p * 1.8).min(1.0)),
                        -dir * d * e,
                        0.0,
                    ),
                    Xf::fade_move(
                        Easing::SINE_IN_OUT.at(((p - 0.15) / 0.85).clamp(0.0, 1.0)),
                        dir * d * (1.0 - e),
                        0.0,
                    ),
                )
            }
            RouteKind::Slide => {
                let w = b.size.width;
                (
                    Xf::fade_move(1.0, -dir * w * e, 0.0),
                    Xf::fade_move(1.0, dir * w * (1.0 - e), 0.0),
                )
            }
            RouteKind::Scale => (
                Xf {
                    alpha: 1.0 - Easing::SINE_IN_OUT.at((p * 1.8).min(1.0)),
                    scale: 1.0 + 0.015 * e,
                    origin: c,
                    dx: 0.0,
                    dy: 0.0,
                },
                Xf {
                    alpha: Easing::SINE_IN_OUT.at(((p - 0.15) / 0.85).clamp(0.0, 1.0)),
                    scale: 0.975 + 0.025 * e,
                    origin: c,
                    dx: 0.0,
                    dy: 0.0,
                },
            ),
        };
        out.push(DrawCommand::PushClip(b));
        for cmd in old.cmds.iter() {
            out.push(transform(cmd.clone(), &xo));
        }
        for cmd in now_cmds.iter() {
            out.push(transform(cmd.clone(), &xn));
        }
        out.push(DrawCommand::PopClip);
    }
}
