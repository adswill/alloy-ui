//! Static text.

use crate::a11y::Role;
use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Color, DrawCommand, DrawList, Element, Layout, Point, Size, Widget,
};
use alloy_ui_layout::Length;

/// Which step of the theme's type scale a [`Text`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    /// Page title.
    Display,
    /// Section heading.
    Heading,
    /// Sub-section heading.
    Subheading,
    /// Body copy (default).
    Body,
    /// Control labels.
    Label,
    /// Secondary captions.
    Caption,
}

/// Greedy word wrap of `content` into lines no wider than `max_width`.
/// Explicit newlines always break. A single word wider than the limit gets its own line.
pub(crate) fn wrap_lines(ui: &Ui, content: &str, size: f32, max_width: f32) -> Vec<String> {
    let m = ui.metrics();
    let mut lines = Vec::new();
    for para in content.split('\n') {
        let mut line = String::new();
        for word in para.split_inclusive(' ') {
            let candidate = format!("{line}{word}");
            if !line.is_empty() && m.width(candidate.trim_end(), size) > max_width {
                lines.push(std::mem::take(&mut line).trim_end().to_owned());
            }
            line.push_str(word);
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}

/// A run of text, wrapped to the width it is given.
pub struct Text {
    ui: Ui,
    content: String,
    size: f32,
    line_height: f32,
    color: Color,
    weight: u16,
    italic: bool,
    width: Length,
    wrap: bool,
}

impl Text {
    /// Body text in the theme's primary text colour.
    pub fn new(ui: &Ui, content: impl Into<String>) -> Self {
        let t = ui.theme();
        let s = t.typography.body;
        Self {
            ui: ui.clone(),
            content: content.into(),
            size: s.size,
            line_height: s.line_height,
            color: t.palette.text,
            weight: s.weight.0,
            italic: false,
            width: Length::Shrink,
            wrap: true,
        }
    }
    /// Use a step of the type scale.
    pub fn role(mut self, role: TextRole) -> Self {
        let t = self.ui.theme();
        let ty = &t.typography;
        let s = match role {
            TextRole::Display => ty.display,
            TextRole::Heading => ty.heading,
            TextRole::Subheading => ty.subheading,
            TextRole::Body => ty.body,
            TextRole::Label => ty.label,
            TextRole::Caption => ty.caption,
        };
        self.size = s.size;
        self.line_height = s.line_height;
        self.weight = s.weight.0;
        if role == TextRole::Caption {
            self.color = t.palette.text_secondary;
        }
        self
    }
    /// Override the font weight (100..=900).
    pub fn weight(mut self, w: u16) -> Self {
        self.weight = w;
        self
    }
    /// Set in the true italic face.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    /// Override the colour.
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
    /// Secondary (de-emphasised) colour.
    pub fn secondary(mut self) -> Self {
        self.color = self.ui.theme().palette.text_secondary;
        self
    }
    /// Width sizing; `Fill` makes the text wrap to the available width.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Disable wrapping (the text stays on one line per `\n`).
    pub fn no_wrap(mut self) -> Self {
        self.wrap = false;
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }

    fn lines(&self, width: f32) -> Vec<String> {
        if self.wrap && width.is_finite() {
            wrap_lines(&self.ui, &self.content, self.size, width)
        } else {
            self.content.split('\n').map(str::to_owned).collect()
        }
    }
}

impl<M> Widget<M> for Text {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, available: Size) -> Size {
        let m = self.ui.metrics();
        let lines = self.lines(available.width);
        let w = lines
            .iter()
            .map(|l| m.width(l, self.size))
            .fold(0.0, f32::max);
        Size::new(w, lines.len() as f32 * self.line_height)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        self.ui.describe(Role::Label, &self.content, &b, |_| {});
        for (i, line) in self.lines(b.size.width).into_iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            out.push(DrawCommand::Text {
                family: if self.italic {
                    alloy_ui_core::FontFamily::SansItalic
                } else {
                    alloy_ui_core::FontFamily::Sans
                },
                weight: self.weight,
                line_height: None,
                origin: Point::new(b.origin.x, b.origin.y + i as f32 * self.line_height),
                content: line,
                size: self.size,
                color: self.color,
                max_width: None,
            });
        }
    }
}
