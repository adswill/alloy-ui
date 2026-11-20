//! Checkbox with a label.

use crate::{
    ui::Ui,
    util::{focus_ring, key_of},
};
use alloy_ui_core::{
    Border, Color, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};

const BOX: f32 = 16.0;

/// A labelled on/off control. Controlled: you pass `checked` and receive the
/// requested new value through `on_toggle`.
pub struct Checkbox<M> {
    ui: Ui,
    label: String,
    checked: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> M>>,
    id: Option<u64>,
}

impl<M: 'static> Checkbox<M> {
    /// A checkbox showing `checked`.
    pub fn new(ui: &Ui, label: impl Into<String>, checked: bool) -> Self {
        Self {
            ui: ui.clone(),
            label: label.into(),
            checked,
            on_toggle: None,
            id: None,
        }
    }
    /// Called with the new value when the user toggles; without it the checkbox is disabled.
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
    fn toggle(&self, out: &mut Vec<M>) {
        if let Some(f) = &self.on_toggle {
            out.push(f(!self.checked));
        }
    }
}

impl<M: 'static> Widget<M> for Checkbox<M> {
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.body;
        let w = self.ui.metrics().width(&self.label, s.size);
        Size::new(BOX + t.spacing.sm + w, s.line_height.max(BOX))
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
                    self.toggle(out);
                }
                was
            }
            Event::KeyPressed(Key::Character(c), _)
                if enabled && c == " " && self.ui.is_focused(self.id) =>
            {
                self.toggle(out);
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
        let bx = Rectangle::new(
            Point::new(b.origin.x, b.origin.y + (b.size.height - BOX) / 2.0),
            Size::new(BOX, BOX),
        );
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        if self.ui.is_focused(self.id) {
            out.push(focus_ring(bx, t.radii.small, p.focus_ring));
        }
        let (fill, edge) = if !enabled {
            (p.surface, p.border)
        } else if self.checked {
            (if hover { p.accent_hover } else { p.accent }, p.accent)
        } else {
            (
                p.surface_raised,
                if hover {
                    p.text_secondary
                } else {
                    p.border_strong
                },
            )
        };
        out.push(DrawCommand::Quad {
            bounds: bx,
            fill,
            radius: t.radii.small,
            border: Some(Border {
                color: edge,
                width: 1.0,
            }),
            shadow: None,
        });
        if self.checked {
            // A check glyph; the renderer's font fallback supplies it.
            out.push(DrawCommand::Text {
                origin: Point::new(bx.origin.x + 2.5, bx.origin.y - 1.0),
                content: "\u{2713}".into(),
                size: 13.0,
                color: if enabled {
                    p.on_accent
                } else {
                    p.text_disabled
                },
                max_width: None,
            });
        }
        let s = t.typography.body;
        out.push(DrawCommand::Text {
            origin: Point::new(
                b.origin.x + BOX + t.spacing.sm,
                b.origin.y + (b.size.height - s.line_height) / 2.0,
            ),
            content: self.label.clone(),
            size: s.size,
            color: if enabled { p.text } else { p.text_disabled },
            max_width: None,
        });
        let _ = Color::TRANSPARENT;
    }
}
