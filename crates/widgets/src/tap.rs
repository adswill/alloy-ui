//! Tap: make any element clickable, with a hover overlay and an optional selected tint.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
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
    id: Option<u64>,
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
            id: None,
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
    /// Make the tap reachable with Tab / Shift+Tab under `id` (unique per window): it draws a
    /// focus ring while focused and Enter or Space press it. A mouse press focuses it too.
    pub fn focusable(mut self, id: u64) -> Self {
        self.id = Some(id);
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
                if self.id.is_some() {
                    self.ui.set_focus(self.id);
                }
                out.push(m.clone());
                return true;
            }
        }
        let pressed_by_key = match event {
            Event::KeyPressed(Key::Enter, m) => !(m.ctrl || m.alt || m.logo),
            Event::KeyPressed(Key::Character(c), m) => c == " " && !(m.ctrl || m.alt || m.logo),
            _ => false,
        };
        if let (true, Some(m)) = (pressed_by_key, &self.on_press) {
            if self.ui.is_focused(self.id) {
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
        if let Some(id) = self.id {
            self.ui.register_at(id, b);
            let ring = self.ui.ease(
                Ui::anim_key(&b, 9),
                self.ui.is_focused(Some(id)) as u8 as f32,
            );
            if ring > 0.01 {
                out.push(util::focus_ring(
                    b,
                    self.radius,
                    p.focus_ring.with_alpha(p.focus_ring.a * ring),
                ));
            }
        }
        util::draw_children(&self.child, layout, out);
    }
}

/// Lets the arrow keys move keyboard focus between the focusable widgets inside it, in two
/// dimensions: a grid of [`Tap`]s (or buttons) becomes navigable cell to cell. Keys a focused
/// child handles itself, and every key while a text field has focus, are left alone.
pub struct ArrowNav<M> {
    ui: Ui,
    child: Vec<Element<M>>,
}

impl<M: 'static> ArrowNav<M> {
    /// Wrap `child`.
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

impl<M: 'static> Widget<M> for ArrowNav<M> {
    fn style(&self) -> Style {
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
        if util::dispatch(&mut self.child, event, layout, cursor, out) {
            return true;
        }
        if let Event::KeyPressed(Key::Named(n), m) = event {
            if m.ctrl || m.alt || m.logo || m.shift || self.ui.text_focused() {
                return false;
            }
            let (dx, dy) = match n.as_str() {
                "ArrowLeft" => (-1, 0),
                "ArrowRight" => (1, 0),
                "ArrowUp" => (0, -1),
                "ArrowDown" => (0, 1),
                _ => return false,
            };
            return self.ui.focus_toward(dx, dy, layout.bounds);
        }
        false
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        util::draw_children(&self.child, layout, out);
    }
}
