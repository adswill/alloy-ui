//! Carousel: one slide at a time with arrows, dots, a crossfade and optional auto-advance.

use crate::{
    a11y::Role,
    ui::Ui,
    util::{self, focus_ring},
};
use alloy_ui_core::{
    style::{Axis, Style},
    Color, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

const K_IDX: u64 = 0xCA50_0001 << 20;
const K_LAST: u64 = 0xCA50_0002 << 20;
const ARROW: f32 = 32.0;
const DOT_HIT: f32 = 16.0;

/// Shows one of several slides at a time and crossfades between them. Arrows, dots, the Left
/// and Right keys (when focused) and `.auto(ms)` move between slides; auto-advance pauses
/// while the pointer is over the carousel and restarts from zero after a manual change. The
/// current slide lives in the [`Ui`] unless you pin it with [`Carousel::index`].
pub struct Carousel<M> {
    ui: Ui,
    id: u64,
    index: Option<usize>,
    auto_ms: Option<u64>,
    fade_ms: u32,
    height: Option<f32>,
    arrows: bool,
    dots: bool,
    on_change: Option<Rc<dyn Fn(usize) -> M>>,
    slides: Vec<Element<M>>,
}

impl<M: 'static> Carousel<M> {
    /// Slides in order. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, slides: Vec<Element<M>>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            index: None,
            auto_ms: None,
            fade_ms: ui.theme().motion.normal_ms,
            height: None,
            arrows: true,
            dots: true,
            on_change: None,
            slides,
        }
    }
    /// Show slide `i` (you own the position; pair with `on_change`).
    pub fn index(mut self, i: usize) -> Self {
        self.index = Some(i);
        self
    }
    /// Advance every `ms` milliseconds (wrapping round), except while the pointer is over it.
    pub fn auto(mut self, ms: u64) -> Self {
        self.auto_ms = Some(ms.max(1));
        self
    }
    /// Crossfade duration in milliseconds.
    pub fn fade(mut self, ms: u32) -> Self {
        self.fade_ms = ms;
        self
    }
    /// A fixed height in pixels (by default the tallest slide, at least 96).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
    /// Show or hide the previous/next arrows (shown by default).
    pub fn arrows(mut self, on: bool) -> Self {
        self.arrows = on;
        self
    }
    /// Show or hide the dots (shown by default).
    pub fn dots(mut self, on: bool) -> Self {
        self.dots = on;
        self
    }
    /// Called with the new slide index whenever it changes by arrow, dot, key or timer.
    pub fn on_change(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn len(&self) -> usize {
        self.slides.len()
    }
    fn current(&self) -> usize {
        let i = self
            .index
            .unwrap_or_else(|| self.ui.slot(self.id ^ K_IDX).unwrap_or(0) as usize);
        i.min(self.len().saturating_sub(1))
    }
    fn go(&mut self, to: usize, now: u64, out: &mut Vec<M>) {
        let to = to.min(self.len().saturating_sub(1));
        if to == self.current() {
            return;
        }
        if self.index.is_none() {
            self.ui.set_slot(self.id ^ K_IDX, Some(to as i64));
        }
        self.ui.set_slot(self.id ^ K_LAST, Some(now as i64));
        if let Some(f) = &self.on_change {
            out.push(f(to));
        }
    }
    fn step(&mut self, by: isize, now: u64, out: &mut Vec<M>) {
        let n = self.len() as isize;
        if n > 1 {
            let to = (self.current() as isize + by).rem_euclid(n) as usize;
            self.go(to, now, out);
        }
    }
    fn controls(&self) -> bool {
        self.len() > 1
    }
    fn arrow_rect(&self, b: &Rectangle, next: bool) -> Rectangle {
        let x = if next {
            b.origin.x + b.size.width - ARROW - 8.0
        } else {
            b.origin.x + 8.0
        };
        Rectangle::new(
            Point::new(x, b.origin.y + (b.size.height - ARROW) / 2.0),
            Size::new(ARROW, ARROW),
        )
    }
    /// Centre of dot `i`, and the hit box around it.
    fn dot_rect(&self, b: &Rectangle, i: usize) -> Rectangle {
        let total = self.len() as f32 * DOT_HIT;
        Rectangle::new(
            Point::new(
                b.origin.x + (b.size.width - total) / 2.0 + i as f32 * DOT_HIT,
                b.origin.y + b.size.height - DOT_HIT - 4.0,
            ),
            Size::new(DOT_HIT, DOT_HIT),
        )
    }
    fn slide_key(&self, i: usize) -> u64 {
        self.id.wrapping_add((i as u64 + 1) << 32) ^ 0xCA50_0000_0000
    }
}

impl<M: 'static> Widget<M> for Carousel<M> {
    fn style(&self) -> Style {
        Style {
            axis: Axis::Vertical,
            width: Length::Fill(1),
            height: self.height.map_or(Length::Shrink, Length::Fixed),
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        self.slides
            .iter()
            .map(|s| util::node_of(s.0.as_ref()).preferred_size())
            .fold(Size::new(0.0, 0.0), |a, s| {
                Size::new(a.width.max(s.width), a.height.max(s.height).max(96.0))
            })
    }
    fn children(&self) -> &[Element<M>] {
        &self.slides
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        Layout {
            bounds,
            children: self.slides.iter().map(|s| s.0.arrange(bounds)).collect(),
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let inside = b.contains(cursor);
        let now = self.ui.now();
        match event {
            Event::Tick(ms) => {
                if let (Some(every), true) = (self.auto_ms, self.controls()) {
                    let last = self.ui.slot(self.id ^ K_LAST);
                    match last {
                        // Held while the pointer is over it, and started on the first tick.
                        _ if inside => self.ui.set_slot(self.id ^ K_LAST, Some(*ms as i64)),
                        None => self.ui.set_slot(self.id ^ K_LAST, Some(*ms as i64)),
                        Some(t) if *ms >= t as u64 + every => self.step(1, *ms, out),
                        Some(_) => {}
                    }
                }
            }
            Event::MousePressed(MouseButton::Left) if inside => {
                self.ui.set_focus(Some(self.id));
                if self.controls() {
                    if self.arrows {
                        for next in [false, true] {
                            if self.arrow_rect(&b, next).contains(cursor) {
                                self.step(if next { 1 } else { -1 }, now, out);
                                return true;
                            }
                        }
                    }
                    if self.dots {
                        for i in 0..self.len() {
                            if self.dot_rect(&b, i).contains(cursor) {
                                self.go(i, now, out);
                                return true;
                            }
                        }
                    }
                }
            }
            Event::KeyPressed(Key::Named(k), _)
                if self.ui.is_focused(Some(self.id)) && self.controls() =>
            {
                match k.as_str() {
                    "ArrowLeft" => {
                        self.step(-1, now, out);
                        return true;
                    }
                    "ArrowRight" => {
                        self.step(1, now, out);
                        return true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        // Only the slide on show is live; one fading out no longer takes input.
        let cur = self.current();
        match (self.slides.get_mut(cur), layout.children.get(cur)) {
            (Some(s), Some(l)) => s.0.on_event(event, l, cursor, out),
            _ => false,
        }
    }
    fn animating(&self) -> bool {
        // The timer needs ticks; the crossfade is tracked by the Ui itself.
        (self.auto_ms.is_some() && self.controls()) || util::any_animating(&self.slides)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let t = self.ui.theme();
        let p = &t.palette;
        self.ui.register(self.id);
        let cur = self.current();
        out.push(DrawCommand::PushClip(b));
        for (i, (s, l)) in self.slides.iter().zip(&layout.children).enumerate() {
            let pr = self.ui.presence(self.slide_key(i), i == cur, self.fade_ms);
            if !pr.mounted {
                continue;
            }
            if pr.progress >= 0.999 {
                s.0.draw(l, out);
            } else {
                let mut tmp = DrawList::default();
                s.0.draw(l, &mut tmp);
                for c in tmp.0 {
                    out.push(util::fade_translate(c, pr.progress, 0.0, 0.0));
                }
            }
        }
        out.push(DrawCommand::PopClip);
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(b, t.radii.small, p.focus_ring));
        }
        if !self.controls() {
            return;
        }
        let scrim = Color::hex(0x000000).with_alpha(0.45);
        let ink = Color::hex(0xFFFFFF).with_alpha(0.95);
        if self.arrows {
            for next in [false, true] {
                let r = self.arrow_rect(&b, next);
                let label = if next { "Next slide" } else { "Previous slide" };
                self.ui.describe(Role::Button, label, &r, |_| {});
                out.push(DrawCommand::Quad {
                    bounds: r,
                    fill: if self.ui.hovered(&r) {
                        Color::hex(0x000000).with_alpha(0.65)
                    } else {
                        scrim
                    },
                    radius: ARROW / 2.0,
                    border: None,
                    shadow: None,
                });
                let (cx, cy) = (r.origin.x + ARROW / 2.0, r.origin.y + ARROW / 2.0);
                let d = if next { 3.0 } else { -3.0 };
                for sy in [-1.0, 1.0] {
                    out.push(DrawCommand::Line {
                        from: Point::new(cx + d, cy),
                        to: Point::new(cx - d, cy + sy * 6.0),
                        width: 2.0,
                        color: ink,
                    });
                }
            }
        }
        if self.dots {
            for i in 0..self.len() {
                let r = self.dot_rect(&b, i);
                self.ui
                    .describe(Role::Button, &format!("Slide {}", i + 1), &r, |_| {});
                let on = i == cur;
                let d = if on { 8.0 } else { 6.0 };
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(
                            r.origin.x + (r.size.width - d) / 2.0,
                            r.origin.y + (r.size.height - d) / 2.0,
                        ),
                        Size::new(d, d),
                    ),
                    fill: if on {
                        ink
                    } else {
                        Color::hex(0xFFFFFF).with_alpha(0.5)
                    },
                    radius: d / 2.0,
                    border: Some(alloy_ui_core::Border {
                        color: Color::hex(0x000000).with_alpha(0.25),
                        width: 1.0,
                    }),
                    shadow: None,
                });
            }
        }
    }
}
