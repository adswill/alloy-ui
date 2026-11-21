//! Spotlight: dim the window except one widget and explain it in a small card (product tours).

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, Key, Layout, Point,
    Rectangle, Size, Widget,
};
use std::rc::Rc;

const GAP: f32 = 14.0;
const RING_PAD: f32 = 6.0;

/// Wraps one widget (the target). While `active`, the rest of the window is dimmed on the overlay
/// layer, the target is outlined, and `card` floats beside it (below, else above, else beside).
/// The tour is modal: clicks on the dim area are swallowed, Escape calls `on_dismiss`.
pub struct Spotlight<M> {
    ui: Ui,
    id: u64,
    active: bool,
    on_dismiss: Option<Rc<dyn Fn() -> M>>,
    target: Vec<Element<M>>,
    card: Element<M>,
}

impl<M: 'static> Spotlight<M> {
    /// `card` explains `target`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, target: Element<M>, card: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            active: false,
            on_dismiss: None,
            target: vec![target],
            card,
        }
    }
    /// Whether the spotlight is showing.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
    /// Message sent on Escape.
    pub fn on_dismiss(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    /// The outlined target area and the card panel (with the content area inside it).
    fn placed(&self, b: &Rectangle) -> (Rectangle, Rectangle, Rectangle) {
        let pad = self.ui.theme().spacing.lg;
        let hole = Rectangle::new(
            Point::new(b.origin.x - RING_PAD, b.origin.y - RING_PAD),
            Size::new(
                b.size.width + 2.0 * RING_PAD,
                b.size.height + 2.0 * RING_PAD,
            ),
        );
        let c = util::preferred(self.card.0.as_ref());
        let (w, h) = (c.width + 2.0 * pad, c.height + 2.0 * pad);
        let vp = self.ui.viewport();
        let (vl, vt) = (vp.origin.x + 8.0, vp.origin.y + 8.0);
        let (vr, vb) = (
            vp.origin.x + vp.size.width - 8.0,
            vp.origin.y + vp.size.height - 8.0,
        );
        let clamp_x = |x: f32| x.min(vr - w).max(vl);
        let clamp_y = |y: f32| y.min(vb - h).max(vt);
        let below = hole.origin.y + hole.size.height + GAP;
        let above = hole.origin.y - GAP - h;
        let (x, y) = if below + h <= vb {
            (clamp_x(hole.origin.x), below)
        } else if above >= vt {
            (clamp_x(hole.origin.x), above)
        } else if hole.origin.x + hole.size.width + GAP + w <= vr {
            (
                hole.origin.x + hole.size.width + GAP,
                clamp_y(hole.origin.y),
            )
        } else {
            (vl.max(hole.origin.x - GAP - w), clamp_y(hole.origin.y))
        };
        let panel = Rectangle::new(Point::new(x, y), Size::new(w, h));
        let inner = Rectangle::new(Point::new(x + pad, y + pad), Size::new(c.width, c.height));
        (hole, panel, inner)
    }
}

impl<M: 'static> Widget<M> for Spotlight<M> {
    fn style(&self) -> Style {
        // Size and alignment come from the wrapped element, but its padding is applied once,
        // by the element itself, not again by this wrapper.
        let mut s = self.target[0].0.style();
        s.padding = alloy_ui_core::style::Padding::ZERO;
        s.spacing = 0.0;
        s
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.target
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
        if !self.active {
            return util::dispatch(&mut self.target, event, layout, cursor, out);
        }
        let (_, panel, inner) = self.placed(&layout.bounds);
        match event {
            Event::MousePressed(_) => {
                if let Some(p) = self.ui.take_hit(self.id) {
                    let l = self.card.0.arrange(inner);
                    self.card.0.on_event(event, &l, p, out);
                }
                // Modal: nothing behind the scrim receives the press.
                true
            }
            Event::KeyPressed(Key::Escape, _) => {
                if let Some(f) = &self.on_dismiss {
                    out.push(f());
                }
                true
            }
            _ => {
                let l = self.card.0.arrange(inner);
                if panel.contains(cursor) || matches!(event, Event::MouseReleased(_)) {
                    self.card.0.on_event(event, &l, cursor, out);
                }
                false
            }
        }
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.target) || (self.active && self.card.0.animating())
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        util::draw_children(&self.target, layout, out);
        let t = self.ui.theme();
        let k = self.ui.ease_with(
            self.id ^ 0x5107,
            if self.active { 1.0 } else { 0.0 },
            t.motion.normal_ms as u64,
            t.motion.easing,
        );
        if k <= 0.001 {
            return;
        }
        let (hole, panel, inner) = self.placed(&layout.bounds);
        let vp = self.ui.viewport();
        let scrim = Color::hex(0x0A0D14).with_alpha(0.58 * k);
        let quad = |r: Rectangle| DrawCommand::Quad {
            bounds: r,
            fill: scrim,
            radius: 0.0,
            border: None,
            shadow: None,
        };
        let (l, tp) = (vp.origin.x, vp.origin.y);
        let (r, bt) = (l + vp.size.width, tp + vp.size.height);
        let (hl, ht) = (hole.origin.x.max(l), hole.origin.y.max(tp));
        let (hr, hb) = (
            (hole.origin.x + hole.size.width).min(r),
            (hole.origin.y + hole.size.height).min(bt),
        );
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
            Rectangle::new(
                Point::new(x0, y0),
                Size::new((x1 - x0).max(0.0), (y1 - y0).max(0.0)),
            )
        };
        let mut dim = vec![
            quad(rect(l, tp, r, ht)),
            quad(rect(l, hb, r, bt)),
            quad(rect(l, ht, hl, hb)),
            quad(rect(hr, ht, r, hb)),
        ];
        dim.push(DrawCommand::Quad {
            bounds: hole,
            fill: Color::TRANSPARENT,
            radius: t.radii.large,
            border: Some(Border {
                color: t.palette.accent.with_alpha(k),
                width: 2.0,
            }),
            shadow: None,
        });
        self.ui.overlay(Some(self.id), Some(vp), dim);
        let mut list = DrawList::default();
        let fade = |c: Color| c.with_alpha(c.a * k);
        let mut sh = t.elevation.high;
        sh.color = fade(sh.color);
        list.0.push(DrawCommand::Quad {
            bounds: panel,
            fill: fade(t.palette.surface_raised),
            radius: t.radii.large,
            border: Some(Border {
                color: fade(t.palette.border),
                width: 1.0,
            }),
            shadow: Some(sh),
        });
        let l = self.card.0.arrange(inner);
        self.card.0.draw(&l, &mut list);
        self.ui.overlay(Some(self.id), Some(panel), list.0);
    }
}
