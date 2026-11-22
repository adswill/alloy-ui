//! A rolling number: each digit turns like an odometer wheel as the value
//! changes, with the carry only starting once the digits below have reached 9.

use crate::{a11y::Role, motion::Transition, ui::Ui};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, FontFamily, Layout, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_theme::Easing;

/// A number that rolls to its new value.
pub struct Counter {
    ui: Ui,
    key: u64,
    target: f64,
    decimals: u32,
    group: bool,
    prefix: String,
    suffix: String,
    size: f32,
    weight: u16,
    ms: u32,
}

/// One drawn glyph slot.
enum Slot {
    /// A rolling digit: place value `k`.
    Digit(u32),
    /// A fixed character.
    Fixed(char),
}

impl Counter {
    /// A counter showing `value`. `key` keeps its animation across rebuilds.
    pub fn new(ui: &Ui, key: u64, value: f64) -> Self {
        let t = ui.theme();
        Self {
            ui: ui.clone(),
            key,
            target: if value.is_finite() { value } else { 0.0 },
            decimals: 0,
            group: true,
            prefix: String::new(),
            suffix: String::new(),
            size: t.typography.body.size,
            weight: 600,
            ms: 900,
        }
    }
    /// Digits after the decimal point (default 0).
    pub fn decimals(mut self, d: u32) -> Self {
        self.decimals = d.min(6);
        self
    }
    /// Group thousands with commas (on by default).
    pub fn grouped(mut self, on: bool) -> Self {
        self.group = on;
        self
    }
    /// Text before the number, e.g. a currency sign.
    pub fn prefix(mut self, s: impl Into<String>) -> Self {
        self.prefix = s.into();
        self
    }
    /// Text after the number, e.g. a unit.
    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.suffix = s.into();
        self
    }
    /// Font size in logical pixels.
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }
    /// Font weight, 100 to 900.
    pub fn weight(mut self, w: u16) -> Self {
        self.weight = w;
        self
    }
    /// Roll time in milliseconds (default 900).
    pub fn duration(mut self, ms: u32) -> Self {
        self.ms = ms;
        self
    }
    /// As an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }

    fn scale(&self) -> f64 {
        10f64.powi(self.decimals as i32)
    }
    /// The rolling value in units of the last shown digit.
    fn rolling(&self) -> f32 {
        let n = (self.target.abs() * self.scale()).round() as f32;
        let sign = if self.target < 0.0 { -1.0 } else { 1.0 };
        self.ui.animate(
            self.key,
            n * sign,
            Transition::eased(self.ms, Easing::QUART_OUT),
        )
    }
    fn slots(&self, n: f32) -> (Vec<Slot>, bool) {
        let neg = n < -0.5;
        let n = n.abs();
        let whole_digits = (n as f64 / self.scale()).floor() as u64;
        let int_len = whole_digits.to_string().len() as u32;
        let mut slots = Vec::new();
        if neg {
            slots.push(Slot::Fixed('\u{2212}'));
        }
        for i in (0..int_len).rev() {
            let k = self.decimals + i;
            slots.push(Slot::Digit(k));
            if self.group && i > 0 && i % 3 == 0 {
                slots.push(Slot::Fixed(','));
            }
        }
        if self.decimals > 0 {
            slots.push(Slot::Fixed('.'));
            for k in (0..self.decimals).rev() {
                slots.push(Slot::Digit(k));
            }
        }
        (slots, neg)
    }
    fn adv(&self, c: char) -> f32 {
        self.ui
            .metrics()
            .width_styled(&c.to_string(), self.size, self.weight, FontFamily::Sans)
    }
    fn digit_adv(&self) -> f32 {
        "0123456789"
            .chars()
            .map(|c| self.adv(c))
            .fold(0.0, f32::max)
    }
    fn line(&self) -> f32 {
        (self.size * 1.35).round()
    }
    fn text_w(&self, s: &str) -> f32 {
        if s.is_empty() {
            0.0
        } else {
            self.ui
                .metrics()
                .width_styled(s, self.size, self.weight, FontFamily::Sans)
        }
    }
    fn width_for(&self, n: f32) -> f32 {
        let (slots, _) = self.slots(n);
        let d = self.digit_adv();
        let body: f32 = slots
            .iter()
            .map(|s| match s {
                Slot::Digit(_) => d,
                Slot::Fixed(c) => self.adv(*c),
            })
            .sum();
        self.text_w(&self.prefix) + body + self.text_w(&self.suffix)
    }
}

impl<M> Widget<M> for Counter {
    fn style(&self) -> Style {
        Style::default()
    }
    fn measure(&self, _a: Size) -> Size {
        Size::new(self.width_for(self.rolling()), self.line())
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        let n = self.rolling();
        let (slots, _) = self.slots(n);
        let lh = self.line();
        let dw = self.digit_adv();
        let text = |x: f32, y: f32, s: String, a: f32| DrawCommand::Text {
            origin: Point::new(x, y),
            content: s,
            size: self.size,
            color: t.palette.text.with_alpha(t.palette.text.a * a),
            max_width: None,
            weight: self.weight,
            family: FontFamily::Sans,
            line_height: Some(lh),
        };
        let mut x = b.origin.x;
        let y = b.origin.y;
        // At rest, draw the whole value as one string so it can be read (and found by the
        // test harness) as "3 of 8" rather than as separate glyphs. While rolling, the
        // per-digit wheels below are used.
        if (n.abs() - n.abs().round()).abs() < 1e-3 {
            let mag = n.abs().round() as f64;
            let mut full = self.prefix.clone();
            for s in &slots {
                match s {
                    Slot::Fixed(c) => full.push(*c),
                    Slot::Digit(k) => {
                        let d = (mag / 10f64.powi(*k as i32)).floor() as u64 % 10;
                        full.push(char::from_digit(d as u32, 10).unwrap_or('0'));
                    }
                }
            }
            full.push_str(&self.suffix);
            out.push(text(x, y, full, 1.0));
            let spoken = format!(
                "{}{:.*}{}",
                self.prefix, self.decimals as usize, self.target, self.suffix
            );
            self.ui.describe(Role::Label, &spoken, &b, |_| {});
            return;
        }
        if !self.prefix.is_empty() {
            out.push(text(x, y, self.prefix.clone(), 1.0));
            x += self.text_w(&self.prefix);
        }
        let mag = n.abs() as f64;
        for s in &slots {
            match s {
                Slot::Fixed(c) => {
                    out.push(text(x, y, c.to_string(), 1.0));
                    x += self.adv(*c);
                }
                Slot::Digit(k) => {
                    let unit = 10f64.powi(*k as i32);
                    let q = mag / unit;
                    let d = (q.floor() as u64 % 10) as u32;
                    // Odometer carry: the wheel for place k turns only while all
                    // lower digits sweep from 9.0 to 10.
                    let roll = if *k == 0 {
                        (mag - mag.floor()) as f32
                    } else {
                        (((q - q.floor()) as f32 - 0.9) * 10.0).clamp(0.0, 1.0)
                    };
                    let roll = Easing::SINE_IN_OUT.at(roll);
                    let shown = q.floor() >= 1.0 || *k == 0;
                    let digit_x = x + (dw - self.adv(char::from_digit(d, 10).unwrap_or('0'))) / 2.0;
                    out.push(DrawCommand::PushClip(Rectangle::new(
                        Point::new(x - 1.0, y),
                        Size::new(dw + 2.0, lh),
                    )));
                    if shown {
                        out.push(text(
                            digit_x,
                            y - roll * lh * 0.8,
                            d.to_string(),
                            1.0 - roll,
                        ));
                    }
                    if roll > 0.001 {
                        let nd = (d + 1) % 10;
                        let nx = x + (dw - self.adv(char::from_digit(nd, 10).unwrap_or('0'))) / 2.0;
                        out.push(text(nx, y + (1.0 - roll) * lh * 0.8, nd.to_string(), roll));
                    }
                    out.push(DrawCommand::PopClip);
                    x += dw;
                }
            }
        }
        if !self.suffix.is_empty() {
            out.push(text(x, y, self.suffix.clone(), 1.0));
        }
        let spoken = format!(
            "{}{:.*}{}",
            self.prefix, self.decimals as usize, self.target, self.suffix
        );
        self.ui.describe(Role::Label, &spoken, &b, |_| {});
    }
}
