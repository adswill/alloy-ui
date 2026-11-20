//! Switch for immediate on/off settings.

use crate::{
    ui::Ui,
    util::{focus_ring, key_of},
};
use alloy_ui_core::{
    Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle,
    Size, Widget,
};

const W: f32 = 34.0;
const H: f32 = 20.0;

/// A switch with an optional label. Use it for settings that apply at once;
/// use a checkbox for choices confirmed later. Controlled.
pub struct Toggle<M> {
    ui: Ui,
    label: String,
    on: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> M>>,
    id: Option<u64>,
}

impl<M: 'static> Toggle<M> {
    /// A switch showing `on`.
    pub fn new(ui: &Ui, label: impl Into<String>, on: bool) -> Self {
        Self {
            ui: ui.clone(),
            label: label.into(),
            on,
            on_toggle: None,
            id: None,
        }
    }
    /// Called with the new value; without it the switch is disabled.
    pub fn on_toggle(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
    /// Make keyboard-focusable under `id`.
    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
    fn flip(&self, out: &mut Vec<M>) {
        if let Some(f) = &self.on_toggle {
            out.push(f(!self.on));
        }
    }
}

impl<M: 'static> Widget<M> for Toggle<M> {
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.body;
        if self.label.is_empty() {
            return Size::new(W, H);
        }
        let w = self.ui.metrics().width(&self.label, s.size);
        Size::new(W + t.spacing.sm + w, s.line_height.max(H))
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let enabled = self.on_toggle.is_some();
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                if enabled {
                    self.ui.set_pressed(Some(key_of(&b)));
                    if self.id.is_some() {
                        self.ui.set_focus(self.id);
                    }
                }
                true
            }
            Event::MouseReleased(MouseButton::Left) => {
                let was = self.ui.pressed() == Some(key_of(&b));
                if was && b.contains(cursor) {
                    self.flip(out);
                }
                was
            }
            Event::KeyPressed(Key::Character(c), _)
                if enabled && c == " " && self.ui.is_focused(self.id) =>
            {
                self.flip(out);
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let enabled = self.on_toggle.is_some();
        let hover = enabled && self.ui.hovered(&b);
        let track = Rectangle::new(
            Point::new(b.origin.x, b.origin.y + (b.size.height - H) / 2.0),
            Size::new(W, H),
        );
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        if self.ui.is_focused(self.id) {
            out.push(focus_ring(track, H / 2.0, p.focus_ring));
        }
        let (fill, edge, knob) = match (self.on, enabled) {
            (true, true) => (
                if hover { p.accent_hover } else { p.accent },
                p.accent,
                p.on_accent,
            ),
            (true, false) => (p.border, p.border, p.surface),
            (false, true) => (p.surface_raised, p.border_strong, p.text_secondary),
            (false, false) => (p.surface, p.border, p.text_disabled),
        };
        out.push(DrawCommand::Quad {
            bounds: track,
            fill,
            radius: H / 2.0,
            border: Some(Border {
                color: edge,
                width: 1.0,
            }),
            shadow: None,
        });
        let k = H - 8.0;
        let kx = if self.on {
            track.origin.x + W - 4.0 - k
        } else {
            track.origin.x + 4.0
        };
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(Point::new(kx, track.origin.y + 4.0), Size::new(k, k)),
            fill: knob,
            radius: k / 2.0,
            border: None,
            shadow: None,
        });
        if !self.label.is_empty() {
            let s = t.typography.body;
            out.push(DrawCommand::Text {
                weight: 400,
                line_height: None,
                origin: Point::new(
                    b.origin.x + W + t.spacing.sm,
                    b.origin.y + (b.size.height - s.line_height) / 2.0,
                ),
                content: self.label.clone(),
                size: s.size,
                color: if enabled { p.text } else { p.text_disabled },
                max_width: None,
            });
        }
    }
}
