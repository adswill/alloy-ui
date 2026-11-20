//! Radio button.

use crate::{
    ui::Ui,
    util::{focus_ring, key_of},
};
use alloy_ui_core::{
    Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle,
    Size, Widget,
};

const D: f32 = 16.0;

/// One choice of an exclusive group. Build a column of them and select by
/// comparing against your state; `on_select` fires when this one is picked.
pub struct Radio<M> {
    ui: Ui,
    label: String,
    selected: bool,
    on_select: Option<M>,
    id: Option<u64>,
}

impl<M: Clone + 'static> Radio<M> {
    /// A radio showing `selected`.
    pub fn new(ui: &Ui, label: impl Into<String>, selected: bool) -> Self {
        Self {
            ui: ui.clone(),
            label: label.into(),
            selected,
            on_select: None,
            id: None,
        }
    }
    /// Message sent when picked; without it the radio is disabled.
    pub fn on_select(mut self, m: M) -> Self {
        self.on_select = Some(m);
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
}

impl<M: Clone + 'static> Widget<M> for Radio<M> {
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.body;
        let w = self.ui.metrics().width(&self.label, s.size);
        Size::new(D + t.spacing.sm + w, s.line_height.max(D))
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let enabled = self.on_select.is_some();
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
                if was && b.contains(cursor) && !self.selected {
                    out.extend(self.on_select.clone());
                }
                was
            }
            Event::KeyPressed(Key::Character(c), _)
                if enabled && c == " " && self.ui.is_focused(self.id) =>
            {
                if !self.selected {
                    out.extend(self.on_select.clone());
                }
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let enabled = self.on_select.is_some();
        let hover = enabled && self.ui.hovered(&b);
        let ring = Rectangle::new(
            Point::new(b.origin.x, b.origin.y + (b.size.height - D) / 2.0),
            Size::new(D, D),
        );
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        if self.ui.is_focused(self.id) {
            out.push(focus_ring(ring, D / 2.0, p.focus_ring));
        }
        let edge = if !enabled {
            p.border
        } else if self.selected {
            p.accent
        } else if hover {
            p.text_secondary
        } else {
            p.border_strong
        };
        out.push(DrawCommand::Quad {
            bounds: ring,
            fill: if enabled { p.surface_raised } else { p.surface },
            radius: D / 2.0,
            border: Some(Border {
                color: edge,
                width: if self.selected { 2.0 } else { 1.0 },
            }),
            shadow: None,
        });
        if self.selected {
            let d = 6.0;
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(ring.origin.x + (D - d) / 2.0, ring.origin.y + (D - d) / 2.0),
                    Size::new(d, d),
                ),
                fill: if enabled { p.accent } else { p.text_disabled },
                radius: d / 2.0,
                border: None,
                shadow: None,
            });
        }
        let s = t.typography.body;
        out.push(DrawCommand::Text {
            weight: 400,
            line_height: None,
            origin: Point::new(
                b.origin.x + D + t.spacing.sm,
                b.origin.y + (b.size.height - s.line_height) / 2.0,
            ),
            content: self.label.clone(),
            size: s.size,
            color: if enabled { p.text } else { p.text_disabled },
            max_width: None,
        });
    }
}
