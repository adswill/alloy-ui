//! Pie and donut charts.

use crate::canvas::Path;
use crate::plot::{format_exact, series_color};
use crate::util::{self, label_cmd};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::{f32::consts::TAU, rc::Rc};

/// One wedge: a name and a non-negative value.
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    /// Shown in the legend and the readout.
    pub label: String,
    /// Share of the whole; negative values count as zero.
    pub value: f32,
    /// Override of the automatic colour.
    pub color: Option<Color>,
}

impl Slice {
    /// A slice named `label` worth `value`.
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
            color: None,
        }
    }
    /// Use `c` instead of the automatic colour.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}

/// A pie, or with [`PieChart::donut`] a ring with the total (or the hovered
/// slice) in the hole. Slices start at twelve o'clock, clockwise, separated by
/// a thin gap; hovering lifts a slice a few pixels. A legend with values and
/// percentages sits to the right.
pub struct PieChart<M> {
    ui: Ui,
    slices: Vec<Slice>,
    hole: f32,
    height: f32,
    legend: bool,
    title: Option<String>,
    unit: String,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
    selected: Option<usize>,
}

const GAP_PX: f32 = 2.0;
const LIFT: f32 = 5.0;

impl<M: 'static> PieChart<M> {
    /// A pie of `slices`.
    pub fn new(ui: &Ui, slices: impl Into<Vec<Slice>>) -> Self {
        Self {
            ui: ui.clone(),
            slices: slices.into(),
            hole: 0.0,
            height: 180.0,
            legend: true,
            title: None,
            unit: String::new(),
            on_select: None,
            selected: None,
        }
    }
    /// Cut a hole so the chart reads as a ring (60% of the radius).
    pub fn donut(mut self) -> Self {
        self.hole = 0.62;
        self
    }
    /// Hole size as a fraction of the radius, `0.0` for a full pie.
    pub fn hole(mut self, fraction: f32) -> Self {
        self.hole = fraction.clamp(0.0, 0.9);
        self
    }
    /// Caption above the total in the middle of a donut (default "Total").
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
    /// Suffix for values, e.g. `"%"` or `" GB"`.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = u.into();
        self
    }
    /// Total height in pixels (default 180).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    /// Hide the legend.
    pub fn no_legend(mut self) -> Self {
        self.legend = false;
        self
    }
    /// Message to send when a slice or legend row is clicked.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
    /// Keep this slice lifted, e.g. the one your app state has selected.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn total(&self) -> f32 {
        self.slices.iter().map(|s| s.value.max(0.0)).sum()
    }
    fn color(&self, t: &alloy_ui_theme::Theme, i: usize) -> Color {
        self.slices[i].color.unwrap_or_else(|| {
            let c = series_color(t, i % 5);
            // Past the fifth slice, soften the repeat so neighbours stay apart.
            if i >= 5 {
                util::mix(c, t.palette.surface, 0.4)
            } else {
                c
            }
        })
    }
    /// `(center, outer radius)` of the pie and the legend's left edge.
    fn geometry(&self, b: &Rectangle, legend_w: f32) -> (Point, f32, f32) {
        let d = (b.size.height - 2.0 * LIFT).max(20.0);
        let r = d / 2.0;
        let show = self.legend && legend_w > 0.0 && b.size.width > d + 140.0;
        let cx = if show {
            b.origin.x + LIFT + r
        } else {
            b.origin.x + b.size.width / 2.0
        };
        let legend_x = cx + r + 28.0;
        (
            Point::new(cx, b.origin.y + b.size.height / 2.0),
            r,
            legend_x,
        )
    }
    /// Angle ranges `(start, end)` per slice, before gaps.
    fn spans(&self) -> Vec<(f32, f32)> {
        let total = self.total().max(f32::EPSILON);
        let mut a = -TAU / 4.0;
        self.slices
            .iter()
            .map(|s| {
                let sweep = s.value.max(0.0) / total * TAU;
                let span = (a, a + sweep);
                a += sweep;
                span
            })
            .collect()
    }
    fn legend_w(&self, b: &Rectangle) -> f32 {
        let (_, r, x) = self.geometry(b, 1.0);
        let _ = r;
        (b.origin.x + b.size.width - x).max(0.0)
    }
    fn hit(&self, b: &Rectangle, p: Point) -> Option<usize> {
        let (c, r, _) = self.geometry(b, self.legend_w(b));
        let (dx, dy) = (p.x - c.x, p.y - c.y);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > r + LIFT || dist < r * self.hole {
            return None;
        }
        let mut a = dy.atan2(dx);
        while a < -TAU / 4.0 {
            a += TAU;
        }
        self.spans()
            .iter()
            .position(|&(s, e)| e > s && a >= s && a < e)
    }
    fn row_rect(&self, b: &Rectangle, i: usize) -> Rectangle {
        let t = self.ui.theme();
        let rh = t.typography.caption.line_height + 10.0;
        let (_, _, x) = self.geometry(b, self.legend_w(b));
        let h = rh * self.slices.len() as f32;
        let y = b.origin.y + (b.size.height - h).max(0.0) / 2.0 + rh * i as f32;
        Rectangle::new(
            Point::new(x - 8.0, y),
            Size::new((b.origin.x + b.size.width - x + 8.0).max(0.0), rh),
        )
    }
    fn legend_hit(&self, b: &Rectangle, p: Point) -> Option<usize> {
        if !(self.legend && b.size.width > self.height + 140.0) {
            return None;
        }
        (0..self.slices.len()).find(|&i| self.row_rect(b, i).contains(p))
    }
}

impl<M: 'static> Widget<M> for PieChart<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Fixed(self.height))
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(self.height + 160.0, self.height)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let Some(f) = self.on_select.clone() else {
            return false;
        };
        if let Event::MousePressed(MouseButton::Left) = event {
            let b = layout.bounds;
            if let Some(i) = self.hit(&b, cursor).or_else(|| self.legend_hit(&b, cursor)) {
                out.push(f(i));
                return true;
            }
        }
        false
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let m = self.ui.metrics();
        let width_of = |s: &str, w: u16| m.width_styled(s, cap.size, w, FontFamily::Sans);
        let b = layout.bounds;
        let total = self.total();
        let summary = format!(
            "{} chart, {}",
            if self.hole > 0.0 { "Donut" } else { "Pie" },
            self.slices
                .iter()
                .map(|s| format!("{} {}{}", s.label, format_exact(s.value), self.unit))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.ui.describe(Role::Label, &summary, &b, |_| {});
        if self.slices.is_empty() || total <= 0.0 {
            return;
        }
        let (center, r, legend_x) = self.geometry(&b, self.legend_w(&b));
        let hole_r = r * self.hole;
        let cur = self.ui.cursor();
        let hot = self.hit(&b, cur).or_else(|| self.legend_hit(&b, cur));
        let spans = self.spans();
        let single = self.slices.iter().filter(|s| s.value > 0.0).count() == 1;

        for (i, (a0, a1)) in spans.iter().enumerate() {
            if a1 <= a0 {
                continue;
            }
            let on = hot == Some(i) || self.selected == Some(i);
            let lift = self.ui.ease(
                0x91E0_0000 ^ ((b.origin.x as u64) << 8) ^ i as u64,
                on as u8 as f32,
            ) * LIFT;
            let mid = (a0 + a1) / 2.0;
            let c = Point::new(center.x + mid.cos() * lift, center.y + mid.sin() * lift);
            let gap = if single { 0.0 } else { GAP_PX };
            // The gap is a constant pixel width, so the angular trim depends on radius.
            let trim_o = (gap / 2.0 / r).min((a1 - a0) / 3.0);
            let trim_i = if hole_r > 1.0 {
                (gap / 2.0 / hole_r).min((a1 - a0) / 3.0)
            } else {
                0.0
            };
            let col = self.color(&t, i);
            let fill = if hot.is_some() && !on {
                util::mix(col, p.surface, 0.28)
            } else {
                col
            };
            // Full circles are split in two: a ring is not a simple polygon.
            let parts: Vec<(f32, f32)> = if a1 - a0 > TAU - 0.01 {
                vec![(*a0, a0 + TAU / 2.0), (a0 + TAU / 2.0, *a1)]
            } else {
                vec![(*a0, *a1)]
            };
            for (s0, s1) in parts {
                let whole = a1 - a0 > TAU - 0.01;
                let (o0, o1) = if whole {
                    (s0, s1)
                } else {
                    (s0 + trim_o, s1 - trim_o)
                };
                let (i0, i1) = if whole {
                    (s0, s1)
                } else {
                    (s0 + trim_i, s1 - trim_i)
                };
                let path = if hole_r > 1.0 {
                    let mut pa = Path::new().arc(c, r, o0, o1);
                    pa = pa.arc(c, hole_r, i1, i0);
                    pa.close()
                } else {
                    Path::new().arc(c, r, o0, o1).line_to(c.x, c.y).close()
                };
                for (pts, _) in path.flatten() {
                    out.push(DrawCommand::Polygon { points: pts, fill });
                }
            }
        }

        // Middle of a donut: the hovered slice, or the total.
        if hole_r > 24.0 {
            let (head, value) = match hot {
                Some(i) => (
                    self.slices[i].label.clone(),
                    format!("{}{}", format_exact(self.slices[i].value), self.unit),
                ),
                None => (
                    self.title.clone().unwrap_or_else(|| "Total".into()),
                    format!("{}{}", format_exact(total), self.unit),
                ),
            };
            let hs = t.typography.heading;
            let vw = m.width_styled(&value, hs.size, 600, FontFamily::Sans);
            let hw = width_of(&head, 400);
            let block = cap.line_height + hs.line_height;
            let y = center.y - block / 2.0;
            out.push(label_cmd(
                Point::new(center.x - hw.min(hole_r * 1.7) / 2.0, y),
                head,
                &cap,
                400,
                p.text_secondary,
            ));
            out.push(label_cmd(
                Point::new(center.x - vw / 2.0, y + cap.line_height),
                value,
                &hs,
                600,
                p.text,
            ));
        }

        // Legend.
        if self.legend && b.size.width > self.height + 140.0 {
            for (i, s) in self.slices.iter().enumerate() {
                let row = self.row_rect(&b, i);
                if hot == Some(i) {
                    out.push(DrawCommand::Quad {
                        bounds: row,
                        fill: p.hover,
                        radius: t.radii.medium,
                        border: None,
                        shadow: None,
                    });
                }
                let ty = row.origin.y + (row.size.height - cap.line_height) / 2.0;
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(legend_x, ty + (cap.line_height - 8.0) / 2.0),
                        Size::new(8.0, 8.0),
                    ),
                    fill: self.color(&t, i),
                    radius: 2.0,
                    border: None,
                    shadow: None,
                });
                out.push(label_cmd(
                    Point::new(legend_x + 16.0, ty),
                    s.label.clone(),
                    &cap,
                    400,
                    p.text,
                ));
                let pct = format!("{:.0}%", s.value.max(0.0) / total * 100.0);
                let val = format!("{}{}", format_exact(s.value), self.unit);
                let right = row.origin.x + row.size.width - 8.0;
                out.push(label_cmd(
                    Point::new(right - width_of(&pct, 400), ty),
                    pct.clone(),
                    &cap,
                    400,
                    p.text_secondary,
                ));
                out.push(label_cmd(
                    Point::new(right - 44.0 - width_of(&val, 600), ty),
                    val,
                    &cap,
                    600,
                    p.text,
                ));
            }
        }

        // A plain pie has no hole to read from, so it gets a tooltip.
        if hole_r <= 24.0 {
            if let Some(i) = self.hit(&b, cur) {
                let s = &self.slices[i];
                let text = format!(
                    "{}  {}{}  {:.0}%",
                    s.label,
                    format_exact(s.value),
                    self.unit,
                    s.value.max(0.0) / total * 100.0
                );
                let w = width_of(&text, 600) + 16.0;
                let h = cap.line_height + 12.0;
                let x = (cur.x + 12.0).min(b.origin.x + b.size.width - w);
                let y = (cur.y - h - 8.0).max(b.origin.y);
                self.ui.overlay(
                    None,
                    None,
                    vec![
                        DrawCommand::Quad {
                            bounds: Rectangle::new(Point::new(x, y), Size::new(w, h)),
                            fill: p.surface_raised,
                            radius: t.radii.medium,
                            border: Some(Border {
                                color: p.border,
                                width: 1.0,
                            }),
                            shadow: Some(t.elevation.medium),
                        },
                        label_cmd(Point::new(x + 8.0, y + 6.0), text, &cap, 600, p.text),
                    ],
                );
            }
        }
    }
}
