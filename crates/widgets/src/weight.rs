//! Text whose weight animates.
//!
//! The bundled Alloy Sans ships as four static weights (400, 500, 600, 700). The text
//! shaper in use does not yet apply variable-font axes, so a continuous weight is faked
//! honestly: the two neighbouring static faces are drawn on top of each other and the
//! heavier one is faded in, which thickens the strokes smoothly. Once the shaper can set
//! `wght` on a variable face this widget can switch to it without any API change.

use crate::{motion::Transition, ui::Ui, util};
use alloy_ui_core::{
    style::Style, Color, DrawCommand, DrawList, Element, FontFamily, Layout, Point, Size, Widget,
};
use alloy_ui_layout::Length;

/// The weights that exist as faces, lightest first.
const LADDER: [u16; 4] = [400, 500, 600, 700];

/// A single line of text drawn at an animated weight.
pub struct WeightText {
    ui: Ui,
    key: u64,
    content: String,
    size: f32,
    weight: f32,
    color: Option<Color>,
    family: FontFamily,
}

impl WeightText {
    /// `content` at `weight` (400..=700); changing `weight` between rebuilds animates it.
    /// `key` identifies the text across rebuilds.
    pub fn new(ui: &Ui, key: u64, content: impl Into<String>, weight: u16) -> Self {
        Self {
            ui: ui.clone(),
            key,
            content: content.into(),
            size: ui.theme().typography.body.size,
            weight: weight.clamp(400, 700) as f32,
            color: None,
            family: FontFamily::Sans,
        }
    }
    /// Font size in logical pixels.
    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
    /// Text colour.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// As an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
    fn shown(&self) -> f32 {
        self.ui.animate(self.key, self.weight, Transition::ms(180))
    }
    /// The faces on either side of `w` and how far from the lighter one it is.
    fn bracket(w: f32) -> (u16, u16, f32) {
        for pair in LADDER.windows(2) {
            if w <= pair[1] as f32 {
                let t = ((w - pair[0] as f32) / (pair[1] - pair[0]) as f32).clamp(0.0, 1.0);
                return (pair[0], pair[1], t);
            }
        }
        (600, 700, 1.0)
    }
}

impl<M> Widget<M> for WeightText {
    fn style(&self) -> Style {
        let (w, h) = self.dims();
        util::sized(Length::Fixed(w), Length::Fixed(h))
    }
    fn measure(&self, _available: Size) -> Size {
        let (w, h) = self.dims();
        Size::new(w, h)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let (lo, hi, t) = Self::bracket(self.shown());
        let color = self.color.unwrap_or(self.ui.theme().palette.text);
        let cmd = |weight: u16, a: f32| DrawCommand::Text {
            origin: Point::new(b.origin.x, b.origin.y),
            content: self.content.clone(),
            size: self.size,
            color: color.with_alpha(color.a * a),
            max_width: None,
            weight,
            family: self.family,
            line_height: None,
        };
        out.push(cmd(lo, 1.0));
        if t > 0.004 {
            out.push(cmd(hi, t));
        }
    }
}

impl WeightText {
    fn dims(&self) -> (f32, f32) {
        let (lo, hi, t) = Self::bracket(self.shown());
        let m = self.ui.metrics();
        let a = m.width_styled(&self.content, self.size, lo, self.family);
        let b = m.width_styled(&self.content, self.size, hi, self.family);
        (a + (b - a) * t, self.size * 1.4)
    }
}
