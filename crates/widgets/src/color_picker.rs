//! Colour picker: saturation/value field, hue strip and a hex readout.

use crate::swatch::hex_of;
use crate::util::{self, focus_ring, key_of, label_cmd};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// `(hue 0..360, saturation 0..1, value 0..1)` of an opaque colour.
pub fn to_hsv(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let h = if d <= f32::EPSILON {
        0.0
    } else if max == c.r {
        60.0 * (((c.g - c.b) / d).rem_euclid(6.0))
    } else if max == c.g {
        60.0 * ((c.b - c.r) / d + 2.0)
    } else {
        60.0 * ((c.r - c.g) / d + 4.0)
    };
    (h, if max <= 0.0 { 0.0 } else { d / max }, max)
}

/// The opaque colour of an HSV triple (hue in degrees).
pub fn from_hsv(h: f32, s: f32, v: f32) -> Color {
    let h = h.rem_euclid(360.0) / 60.0;
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    Color {
        r: r + m,
        g: g + m,
        b: b + m,
        a: 1.0,
    }
}

/// Parse `#RGB`, `#RRGGBB` or the same without `#`.
pub fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    let v = match s.len() {
        3 => {
            let d: Vec<u32> = s.chars().map(|c| c.to_digit(16)).collect::<Option<_>>()?;
            d.iter().fold(0, |a, x| (a << 8) | (x * 17))
        }
        6 => u32::from_str_radix(s, 16).ok()?,
        _ => return None,
    };
    Some(Color::hex(v))
}

const SV_H: f32 = 150.0;
const HUE_H: f32 = 14.0;
const KNOB: f32 = 14.0;

/// Picks an opaque colour. Controlled: pass the current `color` and handle `on_change`. The hue
/// is remembered under the widget `id` so dragging through grey, black or white does not lose it.
pub struct ColorPicker<M> {
    ui: Ui,
    color: Color,
    on_change: Option<Box<dyn Fn(Color) -> M>>,
    id: u64,
}

impl<M: 'static> ColorPicker<M> {
    /// A picker showing `color`, stateful under `id`.
    pub fn new(ui: &Ui, id: u64, color: Color) -> Self {
        Self {
            ui: ui.clone(),
            color,
            on_change: None,
            id,
        }
    }
    /// Called with the new colour; without it the picker is read-only.
    pub fn on_change(mut self, f: impl Fn(Color) -> M + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn hsv(&self) -> (f32, f32, f32) {
        let (h, s, v) = to_hsv(self.color);
        // Grey has no hue of its own: fall back to the remembered one.
        if s > 0.005 && v > 0.005 {
            (h, s, v)
        } else {
            (self.ui.scroll_offset(self.id), s, v)
        }
    }
    fn areas(b: &Rectangle) -> (Rectangle, Rectangle) {
        let sv = Rectangle::new(b.origin, Size::new(b.size.width, SV_H));
        let hue = Rectangle::new(
            Point::new(b.origin.x, b.origin.y + SV_H + 12.0),
            Size::new(b.size.width, HUE_H),
        );
        (sv, hue)
    }
    fn emit(&self, h: f32, s: f32, v: f32, out: &mut Vec<M>) {
        self.ui.set_scroll_offset(self.id, h);
        if let Some(f) = &self.on_change {
            out.push(f(from_hsv(h, s, v)));
        }
    }
}

impl<M: 'static> Widget<M> for ColorPicker<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(220.0, SV_H + 12.0 + HUE_H + 14.0 + 28.0)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        if self.on_change.is_none() {
            return false;
        }
        let (sv, hue) = Self::areas(&layout.bounds);
        let (h, s, v) = self.hsv();
        let at_sv = |p: Point| {
            (
                ((p.x - sv.origin.x) / sv.size.width).clamp(0.0, 1.0),
                1.0 - ((p.y - sv.origin.y) / sv.size.height).clamp(0.0, 1.0),
            )
        };
        let at_hue = |p: Point| ((p.x - hue.origin.x) / hue.size.width).clamp(0.0, 1.0) * 359.99;
        match event {
            Event::MousePressed(MouseButton::Left) if sv.contains(cursor) => {
                self.ui.set_pressed(Some(key_of(&sv)));
                self.ui.set_focus(Some(self.id));
                let (ns, nv) = at_sv(cursor);
                self.emit(h, ns, nv, out);
                true
            }
            Event::MousePressed(MouseButton::Left) if hue.contains(cursor) => {
                self.ui.set_pressed(Some(key_of(&hue)));
                self.emit(at_hue(cursor), s, v.max(0.001), out);
                true
            }
            Event::CursorMoved(p) if self.ui.pressed() == Some(key_of(&sv)) => {
                let (ns, nv) = at_sv(*p);
                self.emit(h, ns, nv, out);
                true
            }
            Event::CursorMoved(p) if self.ui.pressed() == Some(key_of(&hue)) => {
                self.emit(at_hue(*p), s, v.max(0.001), out);
                true
            }
            Event::KeyPressed(Key::Named(k), _) if self.ui.is_focused(Some(self.id)) => {
                let d = 0.02;
                let (ns, nv) = match k.as_str() {
                    "ArrowLeft" => (s - d, v),
                    "ArrowRight" => (s + d, v),
                    "ArrowUp" => (s, v + d),
                    "ArrowDown" => (s, v - d),
                    _ => return false,
                };
                self.emit(h, ns.clamp(0.0, 1.0), nv.clamp(0.0, 1.0), out);
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let (sv, hue) = Self::areas(&b);
        let (h, s, v) = self.hsv();
        self.ui.describe(Role::Label, "Colour picker", &b, |n| {
            n.id = Some(self.id);
            n.value = Some(hex_of(self.color));
            n.enabled = self.on_change.is_some();
        });
        self.ui.register(self.id);

        // Saturation (x) by value (y) field, painted as small cells.
        let (cols, rows) = (((sv.size.width / 5.0) as usize).clamp(8, 64), 30usize);
        let (cw, ch) = (sv.size.width / cols as f32, sv.size.height / rows as f32);
        for r in 0..rows {
            for c in 0..cols {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(sv.origin.x + c as f32 * cw, sv.origin.y + r as f32 * ch),
                        Size::new(cw + 0.5, ch + 0.5),
                    ),
                    fill: from_hsv(
                        h,
                        (c as f32 + 0.5) / cols as f32,
                        1.0 - (r as f32 + 0.5) / rows as f32,
                    ),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
        }
        let frame = |r: Rectangle| DrawCommand::Quad {
            bounds: r,
            fill: Color::TRANSPARENT,
            radius: t.radii.medium,
            border: Some(Border {
                color: p.border_control,
                width: 1.0,
            }),
            shadow: None,
        };
        out.push(frame(sv));
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(sv, t.radii.medium, p.focus_ring));
        }
        let knob = |c: Point, fill: Color| {
            [DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(c.x - KNOB / 2.0, c.y - KNOB / 2.0),
                    Size::new(KNOB, KNOB),
                ),
                fill,
                radius: KNOB / 2.0,
                border: Some(Border {
                    color: Color::hex(0xFFFFFF),
                    width: 2.5,
                }),
                shadow: Some(t.elevation.low),
            }]
        };
        out.0.extend(knob(
            Point::new(
                sv.origin.x + s * sv.size.width,
                sv.origin.y + (1.0 - v) * sv.size.height,
            ),
            self.color,
        ));

        // Hue strip.
        let strips = ((hue.size.width / 3.0) as usize).clamp(12, 120);
        let sw = hue.size.width / strips as f32;
        for i in 0..strips {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(hue.origin.x + i as f32 * sw, hue.origin.y),
                    Size::new(sw + 0.5, hue.size.height),
                ),
                fill: from_hsv((i as f32 + 0.5) / strips as f32 * 360.0, 1.0, 1.0),
                radius: 0.0,
                border: None,
                shadow: None,
            });
        }
        out.push(frame(hue));
        out.0.extend(knob(
            Point::new(
                hue.origin.x + h / 360.0 * hue.size.width,
                hue.origin.y + hue.size.height / 2.0,
            ),
            from_hsv(h, 1.0, 1.0),
        ));

        // Preview chip and hex readout.
        let y = hue.origin.y + hue.size.height + 14.0;
        let chip = Rectangle::new(Point::new(b.origin.x, y), Size::new(28.0, 28.0));
        out.push(DrawCommand::Quad {
            bounds: chip,
            fill: self.color,
            radius: t.radii.medium,
            border: Some(Border {
                color: p.text.with_alpha(0.18),
                width: 1.0,
            }),
            shadow: None,
        });
        let code = t.typography.code;
        out.push(DrawCommand::Text {
            family: FontFamily::Mono,
            weight: 400,
            line_height: Some(code.line_height),
            origin: Point::new(chip.origin.x + 38.0, y + (28.0 - code.line_height) / 2.0),
            content: hex_of(self.color),
            size: code.size,
            color: p.text,
            max_width: None,
        });
        let cap = t.typography.caption;
        let rgb = format!(
            "{}, {}, {}",
            (self.color.r * 255.0).round(),
            (self.color.g * 255.0).round(),
            (self.color.b * 255.0).round()
        );
        let w = self.ui.metrics().width(&rgb, cap.size);
        out.push(label_cmd(
            Point::new(
                b.origin.x + b.size.width - w,
                y + (28.0 - cap.line_height) / 2.0,
            ),
            rgb,
            &cap,
            400,
            p.text_secondary,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_round_trips() {
        for c in [0x0B6B5F, 0xFFE600, 0x3366FF, 0x808080, 0x000000, 0xFFFFFF] {
            let c = Color::hex(c);
            let (h, s, v) = to_hsv(c);
            let d = from_hsv(h, s, v);
            assert!(
                (c.r - d.r).abs() < 0.01 && (c.g - d.g).abs() < 0.01 && (c.b - d.b).abs() < 0.01
            );
        }
    }

    #[test]
    fn hex_parses() {
        assert_eq!(parse_hex("#0B6B5F"), Some(Color::hex(0x0B6B5F)));
        assert_eq!(parse_hex("fff"), Some(Color::hex(0xFFFFFF)));
        assert_eq!(parse_hex("#12"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
    }
}
