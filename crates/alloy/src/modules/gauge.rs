//! Gauges and radial progress.

use super::*;
use crate::animated;
use alloy_ui_layout::Length;
use alloy_ui_widgets::Path;
use std::f32::consts::PI;

/// A dial under construction. See [`gauge`] and [`radial`].
pub struct Dial {
    value: f32,
    ring: bool,
    label: Option<String>,
    text: Option<String>,
    size: f32,
    bands: bool,
}

/// A 270-degree gauge for a value between 0 and 1: `gauge(0.68).label("CPU").bands()`.
/// The arc eases to each new value.
pub fn gauge(value: f32) -> Dial {
    Dial {
        value,
        ring: false,
        label: None,
        text: None,
        size: 132.0,
        bands: false,
    }
}

/// A full ring of progress, starting at the top: `radial(0.4).label("Uploaded")`.
pub fn radial(value: f32) -> Dial {
    Dial {
        ring: true,
        ..gauge(value)
    }
}

impl Dial {
    /// A word under the number.
    pub fn label(mut self, s: impl Into<String>) -> Self {
        self.label = Some(s.into());
        self
    }
    /// Replace the centre text (default is the percentage).
    pub fn text(mut self, s: impl Into<String>) -> Self {
        self.text = Some(s.into());
        self
    }
    /// Edge length in pixels (default 132).
    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
    /// Colour by level: accent, then amber from 70%, then red from 90%.
    pub fn bands(mut self) -> Self {
        self.bands = true;
        self
    }
}

impl<M: 'static> From<Dial> for Node<M> {
    fn from(d: Dial) -> Self {
        let v = animated(d.value.clamp(0.0, 1.0));
        let (ring, size, bands) = (d.ring, d.size, d.bands);
        let shown = d
            .text
            .unwrap_or_else(|| format!("{:.0}%", d.value.clamp(0.0, 1.0) * 100.0));
        let label = d.label;
        paint(Length::Fixed(size), size, move |f| {
            let p = f.palette();
            let tone = if !bands {
                p.accent
            } else if v >= 0.9 {
                p.danger
            } else if v >= 0.7 {
                p.warning
            } else {
                p.accent
            };
            let stroke = (size * 0.085).max(5.0);
            let c = pt(size / 2.0, size / 2.0);
            let r = size / 2.0 - stroke / 2.0 - 1.0;
            let (a0, sweep) = if ring {
                (-PI / 2.0, 2.0 * PI)
            } else {
                (PI * 0.75, PI * 1.5)
            };
            if ring {
                f.stroke_circle(c, r, stroke, p.border);
            } else {
                f.stroke(&Path::new().arc(c, r, a0, a0 + sweep), stroke, p.border);
            }
            if v > 0.004 {
                f.stroke(&Path::new().arc(c, r, a0, a0 + sweep * v), stroke, tone);
            }
            let big = size * 0.21;
            let tw = f.text_width(&shown, big) * 1.04;
            let has_label = label.is_some();
            let ty = c.y - big * 0.62 - if has_label { size * 0.04 } else { 0.0 };
            f.text_weight(pt(c.x - tw / 2.0, ty), shown.clone(), big, 600, p.text);
            if let Some(l) = &label {
                let small = (size * 0.1).max(11.0);
                let lw = f.text_width(l, small);
                f.text(
                    pt(c.x - lw / 2.0, ty + big * 1.25),
                    l.clone(),
                    small,
                    p.text_secondary,
                );
            }
        })
    }
}
