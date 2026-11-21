//! Tap: make any element clickable, with a hover overlay and an optional selected tint.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Layout, MouseButton, Point, Rectangle,
    Size, Widget,
};

/// Wraps one element so a left click on it (that no inner control took) sends a message.
/// Rows, cards and tiles use it; it adds a quiet hover overlay and, when `selected`, the
/// theme's selection tint.
pub struct Tap<M> {
    ui: Ui,
    on_press: Option<M>,
    selected: bool,
    radius: f32,
    hover: bool,
    child: Vec<Element<M>>,
}

impl<M: Clone + 'static> Tap<M> {
    /// Make `child` clickable.
    pub fn new(ui: &Ui, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            on_press: None,
            selected: false,
            radius: ui.theme().radii.medium,
            hover: true,
            child: vec![child],
        }
    }
    /// Message sent on click.
    pub fn on_press(mut self, m: M) -> Self {
        self.on_press = Some(m);
        self
    }
    /// Draw the selection tint.
    pub fn selected(mut self, yes: bool) -> Self {
        self.selected = yes;
        self
    }
    /// Corner radius of the overlay.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    /// Turn the hover overlay off.
    pub fn no_hover(mut self) -> Self {
        self.hover = false;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: Clone + 'static> Widget<M> for Tap<M> {
    fn style(&self) -> Style {
        // Size and alignment come from the wrapped element, but its padding is applied once,
        // by the element itself, not again by this wrapper.
        let mut s = self.child[0].0.style();
        s.padding = alloy_ui_core::style::Padding::ZERO;
        s.spacing = 0.0;
        s
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
        if util::dispatch(&mut self.child, event, layout, cursor, out) {
            return true;
        }
        if let (Event::MousePressed(MouseButton::Left), Some(m)) = (event, &self.on_press) {
            if layout.bounds.contains(cursor) {
                out.push(m.clone());
                return true;
            }
        }
        false
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let p = self.ui.theme().palette.clone();
        let b = layout.bounds;
        let tint = if self.selected {
            Some(p.selection)
        } else if self.hover && self.on_press.is_some() && b.contains(self.ui.cursor()) {
            Some(p.hover)
        } else {
            None
        };
        if let Some(fill) = tint {
            out.push(DrawCommand::Quad {
                bounds: b,
                fill,
                radius: self.radius,
                border: None,
                shadow: None,
            });
        }
        util::draw_children(&self.child, layout, out);
    }
}
