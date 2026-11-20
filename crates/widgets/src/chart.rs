//! Bar chart.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, FontFamily, Layout, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;

/// Vertical bars, one per value, with optional labels underneath. Hovering a
/// bar darkens it and shows its value. Values scale to `max` (default: the
/// largest value).
pub struct BarChart {
    ui: Ui,
    values: Vec<f32>,
    labels: Vec<String>,
    max: Option<f32>,
    height: f32,
}

impl BarChart {
    /// A chart of `values`.
    pub fn new(ui: &Ui, values: impl Into<Vec<f32>>) -> Self {
        Self {
            ui: ui.clone(),
            values: values.into(),
            labels: Vec::new(),
            max: None,
            height: 140.0,
        }
    }
    /// Names under the bars.
    pub fn labels<S: Into<String>>(mut self, labels: impl IntoIterator<Item = S>) -> Self {
        self.labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Value that fills the full height.
    pub fn max(mut self, max: f32) -> Self {
        self.max = Some(max);
        self
    }
    /// Total height including labels (default 140).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for BarChart {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Fixed(self.height))
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(0.0, self.height)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let cap = t.typography.caption;
        let label_h = if self.labels.is_empty() {
            0.0
        } else {
            cap.line_height + 6.0
        };
        let top = cap.line_height + 4.0; // room for the hover value
        let plot_h = (b.size.height - label_h - top).max(1.0);
        let n = self.values.len().max(1) as f32;
        let gap = t.spacing.sm;
        let w = ((b.size.width - gap * (n - 1.0)) / n).max(1.0);
        let max = self
            .max
            .unwrap_or_else(|| self.values.iter().copied().fold(0.0, f32::max))
            .max(f32::EPSILON);
        let base = b.origin.y + top + plot_h;
        let text = |x: f32, y: f32, content: String, color| DrawCommand::Text {
            family: FontFamily::Sans,
            weight: cap.weight.0,
            line_height: None,
            origin: Point::new(x, y),
            content,
            size: cap.size,
            color,
            max_width: None,
        };
        let centred = |s: &str, x: f32, bar_w: f32| {
            x + (bar_w
                - self
                    .ui
                    .metrics()
                    .width_styled(s, cap.size, cap.weight.0, FontFamily::Sans))
                / 2.0
        };
        for (i, v) in self.values.iter().enumerate() {
            let x = b.origin.x + i as f32 * (w + gap);
            let col = Rectangle::new(Point::new(x, b.origin.y + top), Size::new(w, plot_h));
            let hot = self.ui.hovered(&col);
            let h = plot_h * (v / max).clamp(0.0, 1.0);
            out.push(DrawCommand::Quad {
                bounds: col,
                fill: p.surface,
                radius: t.radii.small,
                border: None,
                shadow: None,
            });
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(x, base - h), Size::new(w, h)),
                fill: if hot { p.accent_hover } else { p.accent },
                radius: t.radii.small,
                border: None,
                shadow: None,
            });
            if hot {
                let s = format!("{v}");
                out.push(text(centred(&s, x, w), b.origin.y, s, p.text));
            }
            if let Some(l) = self.labels.get(i) {
                out.push(text(
                    centred(l, x, w),
                    base + 6.0,
                    l.clone(),
                    p.text_secondary,
                ));
            }
        }
    }
}
