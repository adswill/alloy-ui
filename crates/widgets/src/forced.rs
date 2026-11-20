//! Pin a widget into an interaction state, for galleries, docs and tests.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};

/// Draws its child as if the pointer were over it, pressed on it, or the
/// keyboard focus were on it. Real input is ignored, so a gallery can show
/// every state of every widget in one frame.
pub struct Forced<M> {
    ui: Ui,
    child: Vec<Element<M>>,
    hover: bool,
    press: bool,
    focus: Option<u64>,
}

impl<M: 'static> Forced<M> {
    /// Wrap `child` with no state forced yet.
    pub fn new(ui: &Ui, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            child: vec![child],
            hover: false,
            press: false,
            focus: None,
        }
    }
    /// Draw the child hovered.
    pub fn hover(mut self) -> Self {
        self.hover = true;
        self
    }
    /// Draw the child hovered and held down.
    pub fn press(mut self) -> Self {
        self.hover = true;
        self.press = true;
        self
    }
    /// Draw the child focused; `id` is the id the child was given.
    pub fn focus(mut self, id: u64) -> Self {
        self.focus = Some(id);
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Forced<M> {
    fn style(&self) -> Style {
        Style::default()
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
    fn on_event(&mut self, _: &Event, _: &Layout, _: Point, _: &mut Vec<M>) -> bool {
        false
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.children.first().map_or(layout.bounds, |c| c.bounds);
        let (cursor, pressed, focused) = (self.ui.cursor(), self.ui.pressed(), self.ui.focused());
        if self.hover {
            self.ui.set_cursor(Point::new(
                b.origin.x + b.size.width / 2.0,
                b.origin.y + b.size.height / 2.0,
            ));
        }
        if self.press {
            self.ui.set_pressed(Some(util::key_of(&b)));
        }
        if self.focus.is_some() {
            self.ui.set_focus(self.focus);
        }
        util::draw_children(&self.child, layout, out);
        self.ui.set_cursor(cursor);
        self.ui.set_pressed(pressed);
        self.ui.set_focus(focused);
    }
}
