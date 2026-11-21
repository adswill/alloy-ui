//! Live-streaming line chart over a fixed-size window of recent samples.

use crate::plot::{format_exact, format_value, nice_ticks, series_color};
use crate::util::{self, label_cmd};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, FontFamily, Layout, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::collections::VecDeque;

/// A fixed-capacity window of the most recent samples. Keep one per series in
/// your app state and [`push`](Ring::push) on every tick; the oldest sample
/// falls off the far end.
#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    cap: usize,
    data: VecDeque<f32>,
    pushed: u64,
}

impl Ring {
    /// A window holding up to `capacity` samples.
    pub fn new(capacity: usize) -> Self {
        Self {
            cap: capacity.max(2),
            data: VecDeque::with_capacity(capacity.max(2)),
            pushed: 0,
        }
    }
    /// A window pre-filled with `value`, so the line spans the full width at once.
    pub fn filled(capacity: usize, value: f32) -> Self {
        let mut r = Self::new(capacity);
        for _ in 0..r.cap {
            r.data.push_back(value);
        }
        r
    }
    /// Append a sample, dropping the oldest when full.
    pub fn push(&mut self, v: f32) {
        if self.data.len() == self.cap {
            self.data.pop_front();
        }
        self.data.push_back(v);
        self.pushed += 1;
    }
    /// Number of samples held.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// True before the first sample.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    /// The newest sample.
    pub fn last(&self) -> Option<f32> {
        self.data.back().copied()
    }
    /// Oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = f32> + '_ {
        self.data.iter().copied()
    }
    /// Window capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

struct Line {
    name: String,
    values: Vec<f32>,
    pushed: u64,
    color: Option<Color>,
}

/// A line chart that scrolls as samples arrive. New samples glide in over the
/// sample interval, so a one-second feed reads as continuous motion rather
/// than a step. The chart only keeps the app awake between a sample and the
/// end of its glide; with [`StreamChart::smooth`] off it redraws once per
/// sample and is idle in between. Reduced motion turns the glide off.
pub struct StreamChart {
    ui: Ui,
    id: u64,
    lines: Vec<Line>,
    height: f32,
    interval_ms: u64,
    smooth: bool,
    area: bool,
    range: Option<(f32, f32)>,
    unit: String,
    span: Option<String>,
    capacity: usize,
}

const K_SEEN: u64 = 0x57E4_0001;
const K_AT: u64 = 0x57E4_0002;

impl StreamChart {
    /// A chart with one line per `(name, ring)`. `id` keeps the glide state
    /// across view rebuilds.
    pub fn new<'a, N: Into<String>>(
        ui: &Ui,
        id: u64,
        series: impl IntoIterator<Item = (N, &'a Ring)>,
    ) -> Self {
        let mut capacity = 2;
        let lines: Vec<Line> = series
            .into_iter()
            .map(|(n, r)| {
                capacity = capacity.max(r.cap);
                Line {
                    name: n.into(),
                    values: r.iter().collect(),
                    pushed: r.pushed,
                    color: None,
                }
            })
            .collect();
        Self {
            ui: ui.clone(),
            id,
            lines,
            height: 160.0,
            interval_ms: 1000,
            smooth: true,
            area: true,
            range: None,
            unit: String::new(),
            span: None,
            capacity,
        }
    }
    /// Time between samples in milliseconds (default 1000), used for the glide.
    pub fn interval_ms(mut self, ms: u64) -> Self {
        self.interval_ms = ms.max(16);
        self
    }
    /// Number of samples across the full width (defaults to the largest ring
    /// capacity).
    pub fn capacity(mut self, n: usize) -> Self {
        self.capacity = n.max(2);
        self
    }
    /// Turn the glide on or off (default on).
    pub fn smooth(mut self, on: bool) -> Self {
        self.smooth = on;
        self
    }
    /// Skip the soft fill under the lines.
    pub fn no_area(mut self) -> Self {
        self.area = false;
        self
    }
    /// Pin the y axis instead of fitting it to the data.
    pub fn range(mut self, lo: f32, hi: f32) -> Self {
        self.range = Some((lo, hi));
        self
    }
    /// Suffix for values, e.g. `"%"`.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = u.into();
        self
    }
    /// A caption for the window, shown bottom-left (e.g. `"last 60 s"`).
    pub fn window_label(mut self, s: impl Into<String>) -> Self {
        self.span = Some(s.into());
        self
    }
    /// Use `c` for line `i` instead of the automatic colour.
    pub fn color(mut self, i: usize, c: Color) -> Self {
        if let Some(l) = self.lines.get_mut(i) {
            l.color = Some(c);
        }
        self
    }
    /// Total height in pixels (default 160).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }

    fn pushed(&self) -> u64 {
        self.lines.first().map_or(0, |l| l.pushed)
    }
    /// Progress through the current glide, `1.0` when settled.
    fn glide(&self) -> f32 {
        if !self.smooth || self.ui.reduced_motion() {
            return 1.0;
        }
        let seen = self.ui.slot(self.id ^ K_SEEN);
        if seen != Some(self.pushed() as i64) {
            return 0.0;
        }
        let at = self.ui.slot(self.id ^ K_AT).unwrap_or(0) as u64;
        let f = self.ui.now().saturating_sub(at) as f32 / self.interval_ms as f32;
        f.clamp(0.0, 1.0)
    }
}

impl<M> Widget<M> for StreamChart {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Fixed(self.height))
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(240.0, self.height)
    }
    fn animating(&self) -> bool {
        self.smooth
            && !self.ui.reduced_motion()
            && self.lines.iter().any(|l| l.values.len() > 1)
            && self.glide() < 1.0
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let m = self.ui.metrics();
        let width_of = |s: &str, w: u16| m.width_styled(s, cap.size, w, FontFamily::Sans);
        let b = layout.bounds;
        self.ui.describe(
            Role::Label,
            &format!(
                "Live chart, {}",
                self.lines
                    .iter()
                    .map(|l| l.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            &b,
            |_| {},
        );
        // Start (or continue) the glide for this sample count.
        let pushed = self.pushed() as i64;
        if self.ui.slot(self.id ^ K_SEEN) != Some(pushed) {
            self.ui.set_slot(self.id ^ K_SEEN, Some(pushed));
            self.ui.set_slot(self.id ^ K_AT, Some(self.ui.now() as i64));
        }
        let frac = self.glide();
        let n_cap = self.capacity;
        if self.lines.iter().all(|l| l.values.is_empty()) {
            return;
        }

        // y axis.
        let (lo, hi) = self.range.unwrap_or_else(|| {
            let all = self.lines.iter().flat_map(|l| l.values.iter().copied());
            let (a, z) = all.fold((f32::MAX, f32::MIN), |(a, z), v| (a.min(v), z.max(v)));
            let span = (z - a).max(f32::EPSILON);
            (
                (a - span * 0.1).max(if a >= 0.0 { 0.0 } else { f32::MIN }),
                z + span * 0.1,
            )
        });
        let (a, z, step) = nice_ticks(lo, hi, 3);
        let (a, z) = match self.range {
            Some((l, h)) => (l, h),
            None => (a, z),
        };
        let span = (z - a).max(f32::EPSILON);
        let ticks: Vec<f32> = (0..=((z - a) / step).round() as usize)
            .map(|i| a + i as f32 * step)
            .filter(|v| *v <= z + step * 0.01)
            .collect();
        let labels: Vec<String> = ticks.iter().map(|v| format_value(*v, step)).collect();
        let gutter = labels.iter().map(|l| width_of(l, 400)).fold(0.0, f32::max) + 10.0;

        let legend_h = cap.line_height + 10.0;
        let foot_h = if self.span.is_some() {
            cap.line_height + 8.0
        } else {
            0.0
        };
        let plot = Rectangle::new(
            Point::new(
                b.origin.x + gutter,
                b.origin.y + legend_h + cap.line_height / 2.0,
            ),
            Size::new(
                (b.size.width - gutter - 2.0).max(1.0),
                (b.size.height - legend_h - foot_h - cap.line_height / 2.0).max(1.0),
            ),
        );
        let y_of = |v: f32| plot.origin.y + plot.size.height * (1.0 - (v - a) / span);

        for (v, l) in ticks.iter().zip(&labels) {
            let y = y_of(*v);
            let base = (*v - a).abs() < step * 0.01;
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(plot.origin.x, y - 0.5),
                    Size::new(plot.size.width, 1.0),
                ),
                fill: if base {
                    p.border_control
                } else {
                    p.border.with_alpha(0.6)
                },
                radius: 0.0,
                border: None,
                shadow: None,
            });
            out.push(label_cmd(
                Point::new(
                    plot.origin.x - 8.0 - width_of(l, 400),
                    y - cap.line_height / 2.0,
                ),
                l.clone(),
                &cap,
                400,
                p.text_secondary,
            ));
        }

        // Newest sample sits at the right edge once the glide is done; mid-glide
        // everything is shifted right by the unfinished part of one step.
        let step_px = plot.size.width / (n_cap.max(2) - 1) as f32;
        let shift = (1.0 - frac) * step_px;
        let color = |i: usize| self.lines[i].color.unwrap_or_else(|| series_color(&t, i));

        // Legend with live values.
        let mut lx = b.origin.x;
        for (i, l) in self.lines.iter().enumerate() {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(lx, b.origin.y + (cap.line_height - 8.0) / 2.0 + 2.0),
                    Size::new(8.0, 8.0),
                ),
                fill: color(i),
                radius: 2.0,
                border: None,
                shadow: None,
            });
            out.push(label_cmd(
                Point::new(lx + 14.0, b.origin.y + 2.0),
                l.name.clone(),
                &cap,
                400,
                p.text_secondary,
            ));
            lx += 14.0 + width_of(&l.name, 400) + 8.0;
            if let Some(v) = l.values.last() {
                let s = format!("{}{}", format_exact(*v), self.unit);
                out.push(label_cmd(
                    Point::new(lx, b.origin.y + 2.0),
                    s.clone(),
                    &cap,
                    600,
                    p.text,
                ));
                lx += width_of(&s, 600);
            }
            lx += 18.0;
        }

        out.push(DrawCommand::PushClip(Rectangle::new(
            Point::new(plot.origin.x, plot.origin.y - 4.0),
            Size::new(plot.size.width + 1.0, plot.size.height + 8.0),
        )));
        let cur = self.ui.cursor();
        let mut hot: Option<(usize, usize)> = None; // (line, sample)
        for (li, l) in self.lines.iter().enumerate() {
            let n = l.values.len();
            let c = color(li);
            let pts: Vec<Point> = l
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let age = (n - 1 - i) as f32;
                    Point::new(
                        plot.origin.x + plot.size.width - age * step_px + shift,
                        y_of(*v),
                    )
                })
                .collect();
            // Far more samples than pixels: draw each pixel column's extremes, not every sample,
            // so the draw list (and the GPU instance buffer) stays bounded by the width.
            let dense = pts.len() as f32 > plot.size.width * 1.5;
            let shown = if dense { decimate(&pts) } else { pts.clone() };
            if self.area && shown.len() > 1 {
                let base = plot.origin.y + plot.size.height;
                let mut poly = shown.clone();
                poly.push(Point::new(shown[shown.len() - 1].x, base));
                poly.push(Point::new(shown[0].x, base));
                out.push(DrawCommand::Polygon {
                    points: poly,
                    fill: c.with_alpha(if li == 0 { 0.12 } else { 0.07 }),
                });
            }
            for w in shown.windows(2) {
                out.push(DrawCommand::Line {
                    from: w[0],
                    to: w[1],
                    width: 2.0,
                    color: c,
                });
            }
            if plot.contains(cur) && !pts.is_empty() {
                let i = (0..n)
                    .min_by(|x, y| {
                        (pts[*x].x - cur.x)
                            .abs()
                            .total_cmp(&(pts[*y].x - cur.x).abs())
                    })
                    .unwrap_or(0);
                if hot.is_none() {
                    hot = Some((li, i));
                }
            }
            if let Some(last) = pts.last() {
                if frac >= 0.999 {
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(last.x - 3.5, last.y - 3.5),
                            Size::new(7.0, 7.0),
                        ),
                        fill: p.surface_raised,
                        radius: 3.5,
                        border: Some(Border {
                            color: c,
                            width: 2.0,
                        }),
                        shadow: None,
                    });
                }
            }
        }
        if let Some((_, i)) = hot {
            // All lines share the x position of the hovered sample.
            let l0 = &self.lines[0];
            let n0 = l0.values.len();
            let x =
                plot.origin.x + plot.size.width - (n0 - 1 - i.min(n0 - 1)) as f32 * step_px + shift;
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(x - 0.5, plot.origin.y),
                    Size::new(1.0, plot.size.height),
                ),
                fill: p.border_strong.with_alpha(0.5),
                radius: 0.0,
                border: None,
                shadow: None,
            });
        }
        out.push(DrawCommand::PopClip);

        if let Some(s) = &self.span {
            let y = b.origin.y + b.size.height - cap.line_height - 1.0;
            out.push(label_cmd(
                Point::new(plot.origin.x, y),
                s.clone(),
                &cap,
                400,
                p.text_secondary,
            ));
            let now = "now";
            out.push(label_cmd(
                Point::new(plot.origin.x + plot.size.width - width_of(now, 400), y),
                now,
                &cap,
                400,
                p.text_secondary,
            ));
        }

        if let Some((_, i)) = hot {
            let rows: Vec<(Color, String, String)> = self
                .lines
                .iter()
                .enumerate()
                .filter_map(|(li, l)| {
                    let back = self.lines[0].values.len().saturating_sub(1 + i);
                    let idx = l.values.len().checked_sub(1 + back)?;
                    Some((
                        color(li),
                        l.name.clone(),
                        format!("{}{}", format_exact(l.values[idx]), self.unit),
                    ))
                })
                .collect();
            let label_w = rows.iter().map(|r| width_of(&r.1, 400)).fold(0.0, f32::max);
            let value_w = rows.iter().map(|r| width_of(&r.2, 600)).fold(0.0, f32::max);
            let inner = label_w + 18.0 + value_w;
            let padp = 8.0;
            let (bw, bh) = (
                inner + 2.0 * padp,
                rows.len() as f32 * cap.line_height + 2.0 * padp,
            );
            let mut x = cur.x + 14.0;
            if x + bw > b.origin.x + b.size.width {
                x = cur.x - 14.0 - bw;
            }
            let y = (cur.y - bh / 2.0).clamp(b.origin.y, b.origin.y + b.size.height - bh);
            let mut cmds = vec![DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(x, y), Size::new(bw, bh)),
                fill: p.surface_raised,
                radius: t.radii.medium,
                border: Some(Border {
                    color: p.border,
                    width: 1.0,
                }),
                shadow: Some(t.elevation.medium),
            }];
            let mut ty = y + padp;
            for (c, name, val) in rows {
                cmds.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(x + padp, ty + (cap.line_height - 8.0) / 2.0),
                        Size::new(8.0, 8.0),
                    ),
                    fill: c,
                    radius: 2.0,
                    border: None,
                    shadow: None,
                });
                cmds.push(label_cmd(
                    Point::new(x + padp + 14.0, ty),
                    name,
                    &cap,
                    400,
                    p.text_secondary,
                ));
                cmds.push(label_cmd(
                    Point::new(x + padp + inner - width_of(&val, 600), ty),
                    val,
                    &cap,
                    600,
                    p.text,
                ));
                ty += cap.line_height;
            }
            self.ui.overlay(None, None, cmds);
        }
    }
}

/// Reduce a dense polyline (x ascending) to at most about three points per pixel column: the
/// first, lowest and highest points of the column, in x order. The envelope looks identical at
/// that scale and costs a fixed number of draw commands however many samples there are.
pub(crate) fn decimate(pts: &[Point]) -> Vec<Point> {
    let mut out = Vec::with_capacity(pts.len().min(4096));
    let mut i = 0;
    while i < pts.len() {
        let col = pts[i].x.floor();
        let mut j = i;
        let (mut lo, mut hi) = (i, i);
        while j < pts.len() && pts[j].x.floor() == col {
            if pts[j].y < pts[lo].y {
                lo = j;
            }
            if pts[j].y > pts[hi].y {
                hi = j;
            }
            j += 1;
        }
        let mut keep = [i, lo, hi, j - 1];
        keep.sort_unstable();
        let mut prev = usize::MAX;
        for k in keep {
            if k != prev {
                out.push(pts[k]);
                prev = k;
            }
        }
        i = j;
    }
    out
}

#[cfg(test)]
mod decimate_tests {
    use super::*;

    #[test]
    fn keeps_the_extremes_of_every_column_and_bounds_the_count() {
        // 10,000 samples over 100 px with a spike at sample 5,000.
        let pts: Vec<Point> = (0..10_000)
            .map(|i| {
                Point::new(
                    i as f32 * 0.01,
                    if i == 5000 { -50.0 } else { (i % 7) as f32 },
                )
            })
            .collect();
        let d = decimate(&pts);
        assert!(d.len() <= 100 * 4, "{} points", d.len());
        assert!(d.iter().any(|p| p.y == -50.0), "the spike survives");
        assert!(d.windows(2).all(|w| w[0].x <= w[1].x), "x stays ascending");
        assert_eq!(d[0], pts[0]);
        assert_eq!(*d.last().unwrap(), pts[9_999]);
    }
}
