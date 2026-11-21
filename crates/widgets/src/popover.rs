//! Popover: a floating panel with any content, anchored to a widget.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use std::rc::Rc;

const GAP: f32 = 6.0;

/// Shows `content` in a floating panel below `anchor` (above it when there is no room), on the
/// overlay layer. Controlled: you keep `open` and learn about changes from `on_toggle(bool)`,
/// which fires with `true` when the anchor is clicked and `false` on Escape or a click outside.
/// Clicks inside the panel go to the content.
pub struct Popover<M> {
    ui: Ui,
    id: u64,
    open: bool,
    on_toggle: Option<Rc<dyn Fn(bool) -> M>>,
    anchor: Vec<Element<M>>,
    content: Element<M>,
}

impl<M: 'static> Popover<M> {
    /// `content` attached to `anchor`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, anchor: Element<M>, content: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            open: false,
            on_toggle: None,
            anchor: vec![anchor],
            content,
        }
    }
    /// Whether the panel is showing.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// Called with the wanted open state when the user clicks the anchor or dismisses the panel.
    pub fn on_toggle(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    /// The panel and the content area inside it, for an anchor at `b`.
    fn placed(&self, b: &Rectangle) -> (Rectangle, Rectangle) {
        let pad = self.ui.theme().spacing.md;
        let c = util::preferred(self.content.0.as_ref());
        let (w, h) = (c.width + 2.0 * pad, c.height + 2.0 * pad);
        let vp = self.ui.viewport();
        let x = b
            .origin
            .x
            .min(vp.origin.x + vp.size.width - w - 4.0)
            .max(vp.origin.x + 4.0);
        let below = b.origin.y + b.size.height + GAP;
        let y = if below + h > vp.origin.y + vp.size.height - 4.0
            && b.origin.y - GAP - h >= vp.origin.y + 4.0
        {
            b.origin.y - GAP - h
        } else {
            below
        };
        let panel = Rectangle::new(Point::new(x, y), Size::new(w, h));
        let inner = Rectangle::new(Point::new(x + pad, y + pad), Size::new(c.width, c.height));
        (panel, inner)
    }
    fn toggle(&self, to: bool, out: &mut Vec<M>) {
        if let Some(f) = &self.on_toggle {
            out.push(f(to));
        }
    }
}

impl<M: 'static> Widget<M> for Popover<M> {
    fn style(&self) -> Style {
        self.anchor[0].0.style()
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.anchor
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
        let b = layout.bounds;
        if self.open {
            let (panel, inner) = self.placed(&b);
            match event {
                Event::MousePressed(_) => {
                    if let Some(p) = self.ui.take_hit(self.id) {
                        let l = self.content.0.arrange(inner);
                        self.content.0.on_event(event, &l, p, out);
                        return true;
                    }
                    if panel.contains(cursor) {
                        return true;
                    }
                    self.toggle(false, out);
                    // A click on the anchor just closes; elsewhere it is consumed too.
                    return true;
                }
                Event::KeyPressed(Key::Escape, _) => {
                    self.toggle(false, out);
                    return true;
                }
                _ => {
                    let l = self.content.0.arrange(inner);
                    if self.content.0.on_event(event, &l, cursor, out) {
                        return true;
                    }
                }
            }
        } else if matches!(event, Event::MousePressed(MouseButton::Left)) && b.contains(cursor) {
            self.toggle(true, out);
            return true;
        }
        util::dispatch(&mut self.anchor, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.anchor) || (self.open && self.content.0.animating())
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        util::draw_children(&self.anchor, layout, out);
        if !self.open {
            return;
        }
        let t = self.ui.theme();
        let (panel, inner) = self.placed(&layout.bounds);
        let mut list = DrawList::default();
        list.0.push(DrawCommand::Quad {
            bounds: panel,
            fill: t.palette.surface_raised,
            radius: t.radii.medium,
            border: Some(Border {
                color: t.palette.border,
                width: 1.0,
            }),
            shadow: Some(t.elevation.medium),
        });
        let l = self.content.0.arrange(inner);
        self.content.0.draw(&l, &mut list);
        self.ui.overlay(Some(self.id), Some(panel), list.0);
    }
}
