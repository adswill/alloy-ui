//! Colour swatch.

use crate::util::{self, focus_ring, key_of, label_cmd};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle,
    Size, Widget,
};
use alloy_ui_layout::Length;

/// `#RRGGBB` for an opaque colour.
pub fn hex_of(c: Color) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", b(c.r), b(c.g), b(c.b))
}

/// A square of colour, optionally captioned with its hex value and optionally
/// pressable (palettes, theme pickers). The selected swatch gets a ring plus a
/// tick, so selection is not colour-only either.
pub struct Swatch<M> {
    ui: Ui,
    color: Color,
    size: f32,
    selected: bool,
    caption: Option<String>,
    on_press: Option<M>,
    id: Option<u64>,
}

impl<M: Clone + 'static> Swatch<M> {
    /// A 28 px swatch of `color`.
    pub fn new(ui: &Ui, color: Color) -> Self {
        Self {
            ui: ui.clone(),
            color,
            size: 28.0,
            selected: false,
            caption: None,
            on_press: None,
            id: None,
        }
    }
    /// Side length in pixels.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size.max(8.0);
        self
    }
    /// Mark as the chosen one.
    pub fn selected(mut self, yes: bool) -> Self {
        self.selected = yes;
        self
    }
    /// A caption under the swatch; `None` shows nothing, [`Swatch::hex`] shows the value.
    pub fn caption(mut self, text: impl Into<String>) -> Self {
        self.caption = Some(text.into());
        self
    }
    /// Caption it with its hex value.
    pub fn hex(self) -> Self {
        let h = hex_of(self.color);
        self.caption(h)
    }
    /// Message sent when clicked (or Space while focused).
    pub fn on_press(mut self, m: M) -> Self {
        self.on_press = Some(m);
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
    fn square(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(b.origin, Size::new(self.size, self.size))
    }
}

impl<M: Clone + 'static> Widget<M> for Swatch<M> {
    fn style(&self) -> Style {
        util::sized(Length::Shrink, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let cap = self.ui.theme().typography.caption;
        match &self.caption {
            Some(c) => Size::new(
                self.size.max(self.ui.metrics().width(c, cap.size)),
                self.size + 4.0 + cap.line_height,
            ),
            None => Size::new(self.size, self.size),
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let Some(m) = self.on_press.clone() else {
            return false;
        };
        let sq = self.square(&layout.bounds);
        let (consumed, activated) = util::activation(&self.ui, self.id, true, event, &sq, cursor);
        if activated {
            out.push(m);
        }
        consumed
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let sq = self.square(&layout.bounds);
        let name = hex_of(self.color);
        self.ui.describe(Role::Button, &name, &sq, |n| {
            n.id = self.id;
            n.enabled = self.on_press.is_some();
        });
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        let r = t.radii.medium.min(self.size / 4.0);
        if self.ui.is_focused(self.id) {
            out.push(focus_ring(sq, r, p.focus_ring));
        }
        if self.selected {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(sq.origin.x - 3.0, sq.origin.y - 3.0),
                    Size::new(sq.size.width + 6.0, sq.size.height + 6.0),
                ),
                fill: Color::TRANSPARENT,
                radius: r + 3.0,
                border: Some(Border {
                    color: p.text,
                    width: 2.0,
                }),
                shadow: None,
            });
        }
        out.push(DrawCommand::Quad {
            bounds: sq,
            fill: self.color,
            radius: r,
            // A hairline keeps near-white and near-black swatches visible on any surface.
            border: Some(Border {
                color: p.text.with_alpha(0.18),
                width: 1.0,
            }),
            shadow: None,
        });
        if self.selected {
            let ink = if alloy_ui_theme::contrast_ratio(Color::hex(0xFFFFFF), self.color)
                >= alloy_ui_theme::contrast_ratio(Color::hex(0x0B0F0F), self.color)
            {
                Color::hex(0xFFFFFF)
            } else {
                Color::hex(0x0B0F0F)
            };
            let (cx, cy, s) = (
                sq.origin.x + self.size / 2.0,
                sq.origin.y + self.size / 2.0,
                self.size / 6.0,
            );
            for (a, b) in [
                (
                    Point::new(cx - s, cy),
                    Point::new(cx - s * 0.3, cy + s * 0.7),
                ),
                (
                    Point::new(cx - s * 0.3, cy + s * 0.7),
                    Point::new(cx + s, cy - s * 0.7),
                ),
            ] {
                out.push(DrawCommand::Line {
                    from: a,
                    to: b,
                    width: 2.0,
                    color: ink,
                });
            }
        }
        if self.on_press.is_some() && (self.ui.is_pressed(&sq) || self.ui.hovered(&sq)) {
            out.push(DrawCommand::Quad {
                bounds: sq,
                fill: if self.ui.is_pressed(&sq) {
                    p.pressed
                } else {
                    p.hover
                },
                radius: r,
                border: None,
                shadow: None,
            });
        }
        let _ = key_of;
        if let Some(c) = &self.caption {
            let cap = t.typography.caption;
            out.push(label_cmd(
                Point::new(sq.origin.x, sq.origin.y + self.size + 4.0),
                c.clone(),
                &cap,
                400,
                p.text_secondary,
            ));
        }
    }
}
