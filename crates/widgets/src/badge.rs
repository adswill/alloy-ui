//! Badge: a small status pill or count.

use crate::util::{self, label_cmd, over};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Layout, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;

/// What a badge says about its subject. Colour is never the only signal: the
/// label always carries the meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tone {
    /// No judgement: counts, versions, categories.
    #[default]
    Neutral,
    /// Highlighted or selected (the theme accent).
    Accent,
    /// Healthy, done, passed.
    Success,
    /// Needs attention, degraded.
    Warning,
    /// Failed, blocked, destructive.
    Danger,
}

impl Tone {
    /// The tone's ink colour in `t`'s palette, as readable text on its surfaces.
    pub fn color(self, t: &alloy_ui_theme::Theme) -> Color {
        let p = &t.palette;
        match self {
            Self::Neutral => p.text_secondary,
            Self::Accent => p.accent,
            Self::Success => p.success,
            Self::Warning => p.warning,
            Self::Danger => p.danger,
        }
    }
}

/// A pill with a short label. `subtle` (the default) is a tinted fill with
/// tone-coloured text; `solid` is a filled pill for the one thing that must
/// stand out; `outline` is a hairline pill. `dot` puts a status dot before the label.
pub struct Badge {
    ui: Ui,
    label: String,
    tone: Tone,
    solid: bool,
    outline: bool,
    dot: bool,
}

impl Badge {
    /// A neutral badge saying `label`.
    pub fn new(ui: &Ui, label: impl Into<String>) -> Self {
        Self {
            ui: ui.clone(),
            label: label.into(),
            tone: Tone::Neutral,
            solid: false,
            outline: false,
            dot: false,
        }
    }
    /// Set the tone.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
    /// Filled with the tone colour.
    pub fn solid(mut self) -> Self {
        self.solid = true;
        self
    }
    /// Hairline outline instead of a tinted fill.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }
    /// A status dot before the label.
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
    fn size(&self) -> Size {
        let t = self.ui.theme();
        let s = t.typography.caption;
        let w = self.ui.metrics().width_styled(
            &self.label,
            s.size,
            600,
            alloy_ui_core::FontFamily::Sans,
        );
        let dot = if self.dot { 6.0 + 5.0 } else { 0.0 };
        Size::new(w + dot + 2.0 * 8.0, s.line_height + 4.0)
    }
}

impl<M> Widget<M> for Badge {
    fn style(&self) -> Style {
        util::sized(Length::Shrink, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        self.size()
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let s = t.typography.caption;
        let b = layout.bounds;
        self.ui.describe(Role::Label, &self.label, &b, |_| {});
        let ink = self.tone.color(&t);
        let (fill, fg, border) = if self.solid {
            let on = if self.tone == Tone::Neutral {
                p.background
            } else {
                // Light ink on dark fills and vice versa, whichever is readable.
                let light = Color::hex(0xFFFFFF);
                let dark = Color::hex(0x0B0F0F);
                if alloy_ui_theme::contrast_ratio(light, ink)
                    >= alloy_ui_theme::contrast_ratio(dark, ink)
                {
                    light
                } else {
                    dark
                }
            };
            (ink, on, None)
        } else if self.outline {
            (
                Color::TRANSPARENT,
                ink,
                Some(Border {
                    color: ink.with_alpha(0.55),
                    width: 1.0,
                }),
            )
        } else {
            // Tint over the page so it works on any surface; text stays AA because
            // the tone is already AA on the surface and the tint only darkens/lightens 14%.
            (ink.with_alpha(0.14), ink, None)
        };
        let _ = over;
        out.push(DrawCommand::Quad {
            bounds: b,
            fill,
            radius: b.size.height / 2.0,
            border,
            shadow: None,
        });
        let mut x = b.origin.x + 8.0;
        if self.dot {
            let d = 6.0;
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(x, b.origin.y + (b.size.height - d) / 2.0),
                    Size::new(d, d),
                ),
                fill: if self.solid { fg } else { ink },
                radius: d / 2.0,
                border: None,
                shadow: None,
            });
            x += d + 5.0;
        }
        out.push(label_cmd(
            Point::new(x, b.origin.y + (b.size.height - s.line_height) / 2.0),
            self.label.clone(),
            &s,
            600,
            fg,
        ));
    }
}
