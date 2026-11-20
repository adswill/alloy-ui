//! Push button.

use crate::{
    ui::Ui,
    util::{self, focus_ring, key_of},
};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton,
    Point, Size, Widget,
};
use alloy_ui_layout::Length;

/// Visual weight of a [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// The one main action of a view: filled with the accent.
    Primary,
    /// Everyday action: outlined.
    #[default]
    Secondary,
    /// Low emphasis: no fill until hovered.
    Ghost,
}

/// A button that emits a message when clicked or activated with Enter/Space.
/// Without `on_press` it is disabled.
pub struct Button<M> {
    ui: Ui,
    label: String,
    on_press: Option<M>,
    variant: Variant,
    id: Option<u64>,
    width: Length,
}

impl<M: Clone + 'static> Button<M> {
    /// A button with `label`.
    pub fn new(ui: &Ui, label: impl Into<String>) -> Self {
        Self {
            ui: ui.clone(),
            label: label.into(),
            on_press: None,
            variant: Variant::default(),
            id: None,
            width: Length::Shrink,
        }
    }
    /// Message to emit on activation; enables the button.
    pub fn on_press(mut self, m: M) -> Self {
        self.on_press = Some(m);
        self
    }
    /// Visual weight.
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    /// Make the button keyboard-focusable under `id` (unique per window).
    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    /// Width sizing.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: Clone + 'static> Widget<M> for Button<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.label;
        let w = self.ui.metrics().width(&self.label, s.size);
        Size::new(w + 2.0 * t.spacing.md, s.line_height + 2.0 * t.spacing.sm)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let enabled = self.on_press.is_some();
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
                    if let Some(m) = &self.on_press {
                        out.push(m.clone());
                    }
                }
                was
            }
            Event::KeyPressed(Key::Enter, _) if enabled && self.ui.is_focused(self.id) => {
                out.extend(self.on_press.clone());
                true
            }
            Event::KeyPressed(Key::Character(c), _)
                if enabled && c == " " && self.ui.is_focused(self.id) =>
            {
                out.extend(self.on_press.clone());
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let enabled = self.on_press.is_some();
        let hover = enabled && self.ui.hovered(&b);
        let down = enabled && self.ui.is_pressed(&b) && hover;
        let (fill, border, text) = match (self.variant, enabled) {
            (Variant::Primary, true) => (
                if down {
                    p.accent_pressed
                } else if hover {
                    p.accent_hover
                } else {
                    p.accent
                },
                None,
                p.on_accent,
            ),
            (Variant::Primary, false) => (p.border, None, p.text_disabled),
            (Variant::Secondary, e) => (
                if down {
                    p.border
                } else if hover {
                    p.surface
                } else {
                    p.surface_raised
                },
                Some(Border {
                    color: if e { p.border_strong } else { p.border },
                    width: 1.0,
                }),
                if e { p.text } else { p.text_disabled },
            ),
            (Variant::Ghost, e) => (
                if down {
                    p.border
                } else if hover {
                    p.surface
                } else {
                    Color::TRANSPARENT
                },
                None,
                if e { p.text } else { p.text_disabled },
            ),
        };
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        if self.ui.is_focused(self.id) {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        out.push(DrawCommand::Quad {
            bounds: b,
            fill,
            radius: t.radii.medium,
            border,
            shadow: None,
        });
        let s = t.typography.label;
        let w = self.ui.metrics().width(&self.label, s.size);
        out.push(DrawCommand::Text {
            origin: Point::new(
                b.origin.x + (b.size.width - w) / 2.0,
                b.origin.y + (b.size.height - s.line_height) / 2.0,
            ),
            content: self.label.clone(),
            size: s.size,
            color: text,
            max_width: None,
        });
    }
}
