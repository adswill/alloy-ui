//! Line and bar charts with real axes, plus a sparkline.

use crate::util::{self, label_cmd};
use crate::zoom::Zoom;
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Layout, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// "Nice" axis bounds around `lo..=hi`: `(first_tick, last_tick, step)` with `step` one of
/// 1, 2, 2.5, 5 times a power of ten, giving about `target` intervals.
pub fn nice_ticks(lo: f32, hi: f32, target: usize) -> (f32, f32, f32) {
    let (lo, hi) = if hi > lo { (lo, hi) } else { (lo, lo + 1.0) };
    let raw = (hi - lo) / target.max(1) as f32;
    let mag = 10f32.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = mag
        * if norm <= 1.0 {
            1.0
        } else if norm <= 2.0 {
            2.0
        } else if norm <= 2.5 {
            2.5
        } else if norm <= 5.0 {
            5.0
        } else {
            10.0
        };
    ((lo / step).floor() * step, (hi / step).ceil() * step, step)
}

/// A value for an axis label or tooltip: thousands grouped, decimals only when the tick step
/// needs them (`step` 0.5 shows `2.5`, step 10 shows `40`).
pub fn format_value(v: f32, step: f32) -> String {
    let decimals = if step >= 1.0 || step <= 0.0 {
        0
    } else {
        (-step.log10()).ceil().clamp(0.0, 4.0) as usize
    };
    group(v, decimals)
}

/// A value with up to two decimals, trailing zeros dropped (`1,204`, `3.5`, `0.07`).
pub fn format_exact(v: f32) -> String {
    let s = group(v, 2);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

fn group(v: f32, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), ""), |(a, b)| (a, b));
    let mut grouped = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sign = if v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') {
        "-"
    } else {
        ""
    };
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac}")
    }
}

/// Categorical series colours: the theme accent first, then hues chosen to stay apart from it
/// and from the success/danger/warning roles (which carry meaning).
pub fn series_color(t: &alloy_ui_theme::Theme, i: usize) -> Color {
    if i == 0 {
        return t.palette.accent;
    }
    let dark = t.appearance == alloy_ui_theme::Appearance::Dark;
    let set: [u32; 4] = if dark {
        [0x7DA2F2, 0xE0A458, 0xC792D8, 0xA0AAAA]
    } else {
        [0x3558A8, 0x9A5A00, 0x7E4585, 0x5A6363]
    };
    Color::hex(set[(i - 1) % set.len()])
}

/// One named run of values.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// Shown in the legend and the hover tooltip.
    pub name: String,
    /// One value per x position.
    pub values: Vec<f32>,
    /// Override of the automatic colour.
    pub color: Option<Color>,
}

impl Series {
    /// A series called `name`.
    pub fn new(name: impl Into<String>, values: impl Into<Vec<f32>>) -> Self {
        Self {
            name: name.into(),
            values: values.into(),
            color: None,
        }
    }
    /// Use this colour instead of the automatic one.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Line,
    Bars,
}

/// A line or grouped-bar chart with a y axis (gridlines and tick labels), x labels, an optional
/// legend, and a hover readout.
pub struct Chart {
    ui: Ui,
    kind: Kind,
    series: Vec<Series>,
    x_labels: Vec<String>,
    height: f32,
    area: bool,
    legend: bool,
    unit: String,
    y_min: Option<f32>,
    y_max: Option<f32>,
    zoom: Option<u64>,
}

impl Chart {
    fn new(ui: &Ui, kind: Kind, series: Vec<Series>) -> Self {
        Self {
            ui: ui.clone(),
            kind,
            series,
            x_labels: Vec::new(),
            height: 220.0,
            area: false,
            legend: true,
            unit: String::new(),
            y_min: None,
            y_max: None,
            zoom: None,
        }
    }
    /// Let the user zoom (scroll) and pan (drag) along the x axis. `id` keeps the view across
    /// rebuilds. While zoomed a "Reset zoom" chip appears (or press `0`, Home or Esc). The y
    /// axis rescales to the visible samples.
    pub fn zoomable(mut self, id: u64) -> Self {
        self.zoom = Some(id);
        self
    }
    /// A line chart of `series`.
    pub fn line(ui: &Ui, series: impl Into<Vec<Series>>) -> Self {
        Self::new(ui, Kind::Line, series.into())
    }
    /// A grouped bar chart of `series`.
    pub fn bars(ui: &Ui, series: impl Into<Vec<Series>>) -> Self {
        Self::new(ui, Kind::Bars, series.into())
    }
    /// Names under the x positions (thinned automatically if they would collide).
    pub fn x_labels<S: Into<String>>(mut self, labels: impl IntoIterator<Item = S>) -> Self {
        self.x_labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Total height (default 220).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    /// Fill under each line.
    pub fn area(mut self) -> Self {
        self.area = true;
        self
    }
    /// Hide the legend a multi-series chart shows by default.
    pub fn no_legend(mut self) -> Self {
        self.legend = false;
        self
    }
    /// Suffix for tooltip values, e.g. `"ms"` or `"%"`.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }
    /// Fix the bottom of the y axis.
    pub fn y_min(mut self, v: f32) -> Self {
        self.y_min = Some(v);
        self
    }
    /// Fix the top of the y axis.
    pub fn y_max(mut self, v: f32) -> Self {
        self.y_max = Some(v);
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }

    fn n(&self) -> usize {
        self.series
            .iter()
            .map(|s| s.values.len())
            .max()
            .unwrap_or(0)
    }
    /// `(axis_lo, axis_hi, step)`.
    fn axis(&self) -> (f32, f32, f32) {
        let all = self.series.iter().flat_map(|s| s.values.iter().copied());
        let (mut lo, mut hi) = all.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
        if lo > hi {
            (lo, hi) = (0.0, 1.0);
        }
        let zero_based = self.kind == Kind::Bars || lo <= 0.0 || lo <= 0.6 * hi;
        if zero_based && lo > 0.0 {
            lo = 0.0;
        }
        let (mut a, mut b, step) =
            nice_ticks(self.y_min.unwrap_or(lo), self.y_max.unwrap_or(hi), 4);
        if let Some(v) = self.y_min {
            a = v;
        }
        if let Some(v) = self.y_max {
            b = v;
        }
        (a, b, step)
    }
    fn color(&self, t: &alloy_ui_theme::Theme, i: usize) -> Color {
        self.series[i].color.unwrap_or_else(|| series_color(t, i))
    }
}

impl Chart {
    /// This chart restricted to samples `start..start + len`, with the zoom turned off.
    fn sliced(&self, start: usize, len: usize) -> Chart {
        let cut = |v: &[f32]| {
            v.get(start..(start + len).min(v.len()))
                .unwrap_or(&[])
                .to_vec()
        };
        Chart {
            ui: self.ui.clone(),
            kind: self.kind,
            series: self
                .series
                .iter()
                .map(|s| Series {
                    values: cut(&s.values),
                    ..s.clone()
                })
                .collect(),
            x_labels: self
                .x_labels
                .get(start..(start + len).min(self.x_labels.len()))
                .unwrap_or(&[])
                .to_vec(),
            height: self.height,
            area: self.area,
            legend: self.legend,
            unit: self.unit.clone(),
            y_min: self.y_min,
            y_max: self.y_max,
            zoom: None,
        }
    }
}

impl<M> Widget<M> for Chart {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Fixed(self.height))
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(240.0, self.height)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        _out: &mut Vec<M>,
    ) -> bool {
        let Some(id) = self.zoom else {
            return false;
        };
        Zoom::new(&self.ui, id, self.n()).on_event(
            event,
            &layout.bounds,
            cursor,
            self.ui.is_focused(Some(id)),
        )
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let Some(id) = self.zoom else {
            return self.paint(layout, out, None);
        };
        self.ui.register(id);
        let z = Zoom::new(&self.ui, id, self.n());
        let (s, l) = z.window();
        if z.zoomed() {
            self.sliced(s, l).paint(layout, out, Some(&z));
        } else {
            self.paint(layout, out, Some(&z));
        }
        z.draw_chip(&layout.bounds, out);
    }
}

impl Chart {
    fn paint(&self, layout: &Layout, out: &mut DrawList, rec: Option<&Zoom>) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let m = self.ui.metrics();
        let width_of = |s: &str| m.width_styled(s, cap.size, 400, FontFamily::Sans);
        let b = layout.bounds;
        let summary = format!(
            "{} chart, {}",
            if self.kind == Kind::Line {
                "Line"
            } else {
                "Bar"
            },
            self.series
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.ui.describe(Role::Label, &summary, &b, |_| {});
        let n = self.n();
        if n == 0 {
            return;
        }
        let (lo, hi, step) = self.axis();
        let span = (hi - lo).max(f32::EPSILON);

        // Legend.
        let show_legend = self.legend && self.series.len() > 1;
        let legend_h = if show_legend {
            cap.line_height + 10.0
        } else {
            0.0
        };
        if show_legend {
            let mut x = b.origin.x;
            for (i, s) in self.series.iter().enumerate() {
                let c = self.color(&t, i);
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(x, b.origin.y + (cap.line_height - 8.0) / 2.0 + 2.0),
                        Size::new(8.0, 8.0),
                    ),
                    fill: c,
                    radius: 2.0,
                    border: None,
                    shadow: None,
                });
                out.push(label_cmd(
                    Point::new(x + 14.0, b.origin.y + 2.0),
                    s.name.clone(),
                    &cap,
                    400,
                    p.text_secondary,
                ));
                x += 14.0 + width_of(&s.name) + 16.0;
            }
        }

        // Plot rectangle: y labels on the left, x labels underneath.
        let ticks: Vec<f32> = (0..=((hi - lo) / step).round() as usize)
            .map(|i| lo + i as f32 * step)
            .collect();
        let tick_labels: Vec<String> = ticks.iter().map(|v| format_value(*v, step)).collect();
        let gutter = tick_labels.iter().map(|l| width_of(l)).fold(0.0, f32::max) + 10.0;
        let xl_h = if self.x_labels.is_empty() {
            0.0
        } else {
            cap.line_height + 8.0
        };
        let plot = Rectangle::new(
            Point::new(
                b.origin.x + gutter,
                b.origin.y + legend_h + cap.line_height / 2.0,
            ),
            Size::new(
                (b.size.width - gutter).max(1.0),
                (b.size.height - legend_h - xl_h - cap.line_height / 2.0).max(1.0),
            ),
        );
        if let Some(z) = rec {
            z.record_plot(plot.origin.x, plot.size.width);
        }
        let y_of = |v: f32| plot.origin.y + plot.size.height * (1.0 - (v - lo) / span);

        // Gridlines and y labels. The zero/base line is the only strong one.
        for (v, l) in ticks.iter().zip(&tick_labels) {
            let y = y_of(*v);
            let base = (*v - lo).abs() < step * 0.01;
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
                Point::new(plot.origin.x - 8.0 - width_of(l), y - cap.line_height / 2.0),
                l.clone(),
                &cap,
                400,
                p.text_secondary,
            ));
        }

        // Geometry of x positions.
        let slot = plot.size.width / n as f32;
        let x_of = |i: usize| match self.kind {
            Kind::Bars => plot.origin.x + slot * (i as f32 + 0.5),
            Kind::Line if n == 1 => plot.origin.x + plot.size.width / 2.0,
            Kind::Line => plot.origin.x + plot.size.width * i as f32 / (n - 1) as f32,
        };

        // x labels, thinned so neighbours never touch.
        if !self.x_labels.is_empty() {
            let widest = self
                .x_labels
                .iter()
                .map(|l| width_of(l))
                .fold(0.0, f32::max);
            let pitch = if n > 1 {
                (x_of(1) - x_of(0)).abs()
            } else {
                f32::MAX
            };
            let stride = ((widest + 12.0) / pitch).ceil().max(1.0) as usize;
            for (i, l) in self.x_labels.iter().enumerate().take(n) {
                if i % stride != 0 {
                    continue;
                }
                let w = width_of(l);
                let x = (x_of(i) - w / 2.0)
                    .clamp(plot.origin.x - gutter / 2.0, b.origin.x + b.size.width - w);
                out.push(label_cmd(
                    Point::new(x, plot.origin.y + plot.size.height + 8.0),
                    l.clone(),
                    &cap,
                    400,
                    p.text_secondary,
                ));
            }
        }

        // Hover target: nearest x position while the cursor is in the plot.
        let cur = self.ui.cursor();
        let hot = plot.contains(cur).then(|| {
            (0..n)
                .min_by(|a, b| {
                    (x_of(*a) - cur.x)
                        .abs()
                        .total_cmp(&(x_of(*b) - cur.x).abs())
                })
                .unwrap_or(0)
        });

        out.push(DrawCommand::PushClip(Rectangle::new(
            Point::new(plot.origin.x - 4.0, plot.origin.y - 4.0),
            Size::new(plot.size.width + 8.0, plot.size.height + 8.0),
        )));
        match self.kind {
            Kind::Bars => {
                let k = self.series.len() as f32;
                let group = (slot * 0.68).min(48.0 * k);
                let bw = (group / k - 2.0).max(2.0);
                let y0 = y_of(0f32.clamp(lo, hi));
                for i in 0..n {
                    if hot == Some(i) {
                        out.push(DrawCommand::Quad {
                            bounds: Rectangle::new(
                                Point::new(plot.origin.x + slot * i as f32, plot.origin.y),
                                Size::new(slot, plot.size.height),
                            ),
                            fill: p.hover,
                            radius: 0.0,
                            border: None,
                            shadow: None,
                        });
                    }
                    for (si, s) in self.series.iter().enumerate() {
                        let Some(v) = s.values.get(i) else { continue };
                        let x = x_of(i) - group / 2.0 + si as f32 * (bw + 2.0) + 1.0;
                        let y = y_of(*v);
                        let (top, h) = if y < y0 { (y, y0 - y) } else { (y0, y - y0) };
                        out.push(DrawCommand::Quad {
                            bounds: Rectangle::new(Point::new(x, top), Size::new(bw, h.max(1.0))),
                            fill: self.color(&t, si),
                            radius: t.radii.small.min(bw / 2.0),
                            border: None,
                            shadow: None,
                        });
                    }
                }
            }
            Kind::Line => {
                for (si, s) in self.series.iter().enumerate() {
                    let c = self.color(&t, si);
                    let pts: Vec<Point> = s
                        .values
                        .iter()
                        .enumerate()
                        .map(|(i, v)| Point::new(x_of(i), y_of(*v)))
                        .collect();
                    if self.area && pts.len() > 1 {
                        let base = y_of(lo.max(0.0).min(hi));
                        let base = base.min(plot.origin.y + plot.size.height);
                        let mut poly = pts.clone();
                        poly.push(Point::new(pts[pts.len() - 1].x, base));
                        poly.push(Point::new(pts[0].x, base));
                        out.push(DrawCommand::Polygon {
                            points: poly,
                            fill: c.with_alpha(0.10),
                        });
                    }
                    for w in pts.windows(2) {
                        out.push(DrawCommand::Line {
                            from: w[0],
                            to: w[1],
                            width: 2.0,
                            color: c,
                        });
                    }
                    let dot = |out: &mut DrawList, pt: Point, r: f32| {
                        out.push(DrawCommand::Quad {
                            bounds: Rectangle::new(
                                Point::new(pt.x - r, pt.y - r),
                                Size::new(2.0 * r, 2.0 * r),
                            ),
                            fill: p.surface_raised,
                            radius: r,
                            border: Some(Border {
                                color: c,
                                width: 2.0,
                            }),
                            shadow: None,
                        });
                    };
                    if n <= 12 {
                        for pt in &pts {
                            dot(out, *pt, 3.0);
                        }
                    }
                    if let Some(i) = hot {
                        if let Some(pt) = pts.get(i) {
                            dot(out, *pt, 4.5);
                        }
                    }
                }
                if let Some(i) = hot {
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(x_of(i) - 0.5, plot.origin.y),
                            Size::new(1.0, plot.size.height),
                        ),
                        fill: p.border_strong.with_alpha(0.5),
                        radius: 0.0,
                        border: None,
                        shadow: None,
                    });
                }
            }
        }
        out.push(DrawCommand::PopClip);

        // Hover readout on the overlay layer.
        if let Some(i) = hot {
            let head = self.x_labels.get(i).cloned().unwrap_or_default();
            let rows: Vec<(Color, String, String)> = self
                .series
                .iter()
                .enumerate()
                .filter_map(|(si, s)| {
                    s.values.get(i).map(|v| {
                        (
                            self.color(&t, si),
                            s.name.clone(),
                            format!("{}{}", format_exact(*v), self.unit),
                        )
                    })
                })
                .collect();
            let label_w = rows.iter().map(|r| width_of(&r.1)).fold(0.0, f32::max);
            let value_w = rows.iter().map(|r| width_of(&r.2)).fold(0.0, f32::max);
            let inner = (label_w + 18.0 + value_w).max(width_of(&head));
            let pad = 8.0;
            let rows_h = rows.len() as f32 * cap.line_height;
            let head_h = if head.is_empty() {
                0.0
            } else {
                cap.line_height + 2.0
            };
            let (bw, bh) = (inner + 2.0 * pad, rows_h + head_h + 2.0 * pad);
            let mut x = x_of(i) + 14.0;
            if x + bw > b.origin.x + b.size.width {
                x = x_of(i) - 14.0 - bw;
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
            let mut ty = y + pad;
            if !head.is_empty() {
                cmds.push(label_cmd(Point::new(x + pad, ty), head, &cap, 600, p.text));
                ty += head_h;
            }
            for (c, name, val) in rows {
                cmds.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(x + pad, ty + (cap.line_height - 8.0) / 2.0),
                        Size::new(8.0, 8.0),
                    ),
                    fill: c,
                    radius: 2.0,
                    border: None,
                    shadow: None,
                });
                cmds.push(label_cmd(
                    Point::new(x + pad + 14.0, ty),
                    name,
                    &cap,
                    400,
                    p.text_secondary,
                ));
                cmds.push(label_cmd(
                    Point::new(x + pad + inner - width_of(&val), ty),
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

/// A tiny trend line with no axes, sized to sit inside a table cell or a stat card. The last
/// value gets a dot.
pub struct Sparkline {
    ui: Ui,
    values: Vec<f32>,
    color: Option<Color>,
    width: Length,
    height: f32,
    area: bool,
}

impl Sparkline {
    /// A sparkline of `values`.
    pub fn new(ui: &Ui, values: impl Into<Vec<f32>>) -> Self {
        Self {
            ui: ui.clone(),
            values: values.into(),
            color: None,
            width: Length::Fixed(96.0),
            height: 28.0,
            area: true,
        }
    }
    /// Line colour (default: the accent).
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Width sizing (default 96 px).
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Height in pixels (default 28).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    /// Drop the soft fill under the line.
    pub fn no_area(mut self) -> Self {
        self.area = false;
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for Sparkline {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Fixed(self.height))
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(96.0, self.height)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        let c = self.color.unwrap_or(t.palette.accent);
        self.ui.describe(Role::Label, "Trend", &b, |n| {
            n.value = self.values.last().map(|v| v.to_string());
        });
        let n = self.values.len();
        if n < 2 {
            return;
        }
        let (lo, hi) = self
            .values
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, z), v| (a.min(*v), z.max(*v)));
        let span = (hi - lo).max(f32::EPSILON);
        let inset = 3.0;
        let (w, h) = (b.size.width - 2.0 * inset, b.size.height - 2.0 * inset);
        let pts: Vec<Point> = self
            .values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                Point::new(
                    b.origin.x + inset + w * i as f32 / (n - 1) as f32,
                    b.origin.y + inset + h * (1.0 - (v - lo) / span),
                )
            })
            .collect();
        if self.area {
            let strips = (w / 2.0).ceil().max(1.0) as usize;
            let sw = w / strips as f32;
            let base = b.origin.y + b.size.height - inset;
            for k in 0..strips {
                let xm = b.origin.x + inset + (k as f32 + 0.5) * sw;
                let seg = pts
                    .windows(2)
                    .find(|s| xm >= s[0].x && xm <= s[1].x)
                    .unwrap_or(&pts[n - 2..]);
                let f = ((xm - seg[0].x) / (seg[1].x - seg[0].x).max(0.001)).clamp(0.0, 1.0);
                let y = seg[0].y + (seg[1].y - seg[0].y) * f;
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(b.origin.x + inset + k as f32 * sw, y),
                        Size::new(sw, (base - y).max(0.0)),
                    ),
                    fill: c.with_alpha(0.12),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
        }
        for s in pts.windows(2) {
            out.push(DrawCommand::Line {
                from: s[0],
                to: s[1],
                width: 1.5,
                color: c,
            });
        }
        let last = pts[n - 1];
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(Point::new(last.x - 3.0, last.y - 3.0), Size::new(6.0, 6.0)),
            fill: c,
            radius: 3.0,
            border: None,
            shadow: None,
        });
        // Hover: ring the nearest point and say what it is.
        let cur = self.ui.cursor();
        if b.contains(cur) {
            let i = (0..n)
                .min_by(|a, z| {
                    (pts[*a].x - cur.x)
                        .abs()
                        .total_cmp(&(pts[*z].x - cur.x).abs())
                })
                .unwrap_or(0);
            let q = pts[i];
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(q.x - 4.0, q.y - 4.0), Size::new(8.0, 8.0)),
                fill: t.palette.surface,
                radius: 4.0,
                border: Some(Border {
                    color: c,
                    width: 2.0,
                }),
                shadow: None,
            });
            util::chart_tip(
                &self.ui,
                q,
                b,
                &format!("Point {} of {n}", i + 1),
                &format_exact(self.values[i]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_are_nice() {
        let (lo, hi, step) = nice_ticks(0.0, 97.0, 4);
        assert_eq!((lo, hi), (0.0, 100.0));
        assert_eq!(step, 25.0);
        let (lo, hi, step) = nice_ticks(3.2, 7.9, 4);
        assert!(lo <= 3.2 && hi >= 7.9 && step == 2.0, "{lo} {hi} {step}");
        let (_, hi, _) = nice_ticks(5.0, 5.0, 4);
        assert!(hi > 5.0);
    }

    #[test]
    fn values_group_and_trim() {
        assert_eq!(format_value(12400.0, 1000.0), "12,400");
        assert_eq!(format_value(2.5, 0.5), "2.5");
        assert_eq!(format_value(-1500.0, 500.0), "-1,500");
        assert_eq!(format_value(0.0, 0.5), "0.0");
        assert_eq!(format_exact(1204.50), "1,204.5");
        assert_eq!(format_exact(3.0), "3");
    }
}
