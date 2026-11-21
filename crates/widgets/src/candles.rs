//! Candlestick (OHLC) chart.

use crate::plot::{format_exact, format_value, nice_ticks};
use crate::util::{self, label_cmd};
use crate::zoom::Zoom;
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, FontFamily, Layout, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// One period's open, high, low and close.
#[derive(Debug, Clone, PartialEq)]
pub struct Candle {
    /// Period name for the x axis and tooltip, e.g. `"Mon"` or `"09:30"`.
    pub label: String,
    /// First price of the period.
    pub open: f32,
    /// Highest price.
    pub high: f32,
    /// Lowest price.
    pub low: f32,
    /// Last price.
    pub close: f32,
    /// Traded volume, drawn as bars underneath when any candle has one.
    pub volume: Option<f32>,
}

impl Candle {
    /// A candle from open, high, low, close.
    pub fn new(label: impl Into<String>, open: f32, high: f32, low: f32, close: f32) -> Self {
        Self {
            label: label.into(),
            open,
            high,
            low,
            close,
            volume: None,
        }
    }
    /// Attach a traded volume.
    pub fn volume(mut self, v: f32) -> Self {
        self.volume = Some(v);
        self
    }
    /// True when the period closed at or above its open.
    pub fn rising(&self) -> bool {
        self.close >= self.open
    }
}

/// Candlesticks with a price axis, optional volume bars, and a crosshair with
/// an OHLC readout on hover. Rising periods use the theme's success colour and
/// falling ones the danger colour; falling candles are filled and rising ones
/// are drawn with a lighter body, so direction never relies on colour alone.
pub struct CandleChart {
    ui: Ui,
    candles: Vec<Candle>,
    height: f32,
    unit: String,
    zoom: Option<u64>,
}

impl CandleChart {
    /// A chart of `candles`, oldest first.
    pub fn new(ui: &Ui, candles: impl Into<Vec<Candle>>) -> Self {
        Self {
            ui: ui.clone(),
            candles: candles.into(),
            height: 240.0,
            unit: String::new(),
            zoom: None,
        }
    }
    /// Let the user zoom (scroll) and pan (drag) along the time axis. `id` keeps the view
    /// across rebuilds. While zoomed a "Reset zoom" chip appears (or press `0`, Home or Esc);
    /// the price axis rescales to the visible candles.
    pub fn zoomable(mut self, id: u64) -> Self {
        self.zoom = Some(id);
        self
    }
    /// Total height in pixels (default 240).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    /// Prefix for prices, e.g. `"$"`.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = u.into();
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
    fn has_volume(&self) -> bool {
        self.candles.iter().any(|c| c.volume.is_some())
    }
}

impl<M> Widget<M> for CandleChart {
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
        Zoom::new(&self.ui, id, self.candles.len()).on_event(
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
        let z = Zoom::new(&self.ui, id, self.candles.len());
        let (s, l) = z.window();
        if z.zoomed() {
            let cut = CandleChart {
                ui: self.ui.clone(),
                candles: self.candles[s..s + l].to_vec(),
                height: self.height,
                unit: self.unit.clone(),
                zoom: None,
            };
            cut.paint(layout, out, Some(&z));
        } else {
            self.paint(layout, out, Some(&z));
        }
        z.draw_chip(&layout.bounds, out);
    }
}

impl CandleChart {
    fn paint(&self, layout: &Layout, out: &mut DrawList, rec: Option<&Zoom>) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let m = self.ui.metrics();
        let width_of = |s: &str, w: u16| m.width_styled(s, cap.size, w, FontFamily::Sans);
        let b = layout.bounds;
        self.ui.describe(
            Role::Label,
            &format!("Candlestick chart, {} periods", self.candles.len()),
            &b,
            |_| {},
        );
        let n = self.candles.len();
        if n == 0 {
            return;
        }
        let lo = self.candles.iter().map(|c| c.low).fold(f32::MAX, f32::min);
        let hi = self.candles.iter().map(|c| c.high).fold(f32::MIN, f32::max);
        let pad = (hi - lo).max(f32::EPSILON) * 0.06;
        let (a, z, step) = nice_ticks(lo - pad, hi + pad, 4);
        let span = (z - a).max(f32::EPSILON);
        let ticks: Vec<f32> = (0..=((z - a) / step).round() as usize)
            .map(|i| a + i as f32 * step)
            .collect();
        let labels: Vec<String> = ticks
            .iter()
            .map(|v| format!("{}{}", self.unit, format_value(*v, step)))
            .collect();
        let gutter = labels.iter().map(|l| width_of(l, 400)).fold(0.0, f32::max) + 10.0;
        let xl_h = cap.line_height + 8.0;
        let vol_h = if self.has_volume() {
            (b.size.height * 0.18).clamp(28.0, 60.0)
        } else {
            0.0
        };
        let gap = if vol_h > 0.0 { 10.0 } else { 0.0 };
        let plot = Rectangle::new(
            Point::new(b.origin.x + gutter, b.origin.y + cap.line_height / 2.0),
            Size::new(
                (b.size.width - gutter).max(1.0),
                (b.size.height - xl_h - vol_h - gap - cap.line_height / 2.0).max(1.0),
            ),
        );
        if let Some(z) = rec {
            z.record_plot(plot.origin.x, plot.size.width);
        }
        let y_of = |v: f32| plot.origin.y + plot.size.height * (1.0 - (v - a) / span);
        let slot = plot.size.width / n as f32;
        let x_of = |i: usize| plot.origin.x + slot * (i as f32 + 0.5);

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

        // x labels, thinned so neighbours never touch.
        let widest = self
            .candles
            .iter()
            .map(|c| width_of(&c.label, 400))
            .fold(0.0, f32::max);
        let stride = ((widest + 12.0) / slot).ceil().max(1.0) as usize;
        let label_y = b.origin.y + b.size.height - cap.line_height - 1.0;
        for (i, c) in self.candles.iter().enumerate().step_by(stride) {
            let w = width_of(&c.label, 400);
            let x = (x_of(i) - w / 2.0).clamp(b.origin.x, b.origin.x + b.size.width - w);
            out.push(label_cmd(
                Point::new(x, label_y),
                c.label.clone(),
                &cap,
                400,
                p.text_secondary,
            ));
        }

        let cur = self.ui.cursor();
        let hot = Rectangle::new(
            plot.origin,
            Size::new(plot.size.width, plot.size.height + gap + vol_h),
        )
        .contains(cur)
        .then(|| (((cur.x - plot.origin.x) / slot).floor().max(0.0) as usize).min(n - 1));

        let body_w = (slot * 0.62).clamp(2.0, 16.0);
        out.push(DrawCommand::PushClip(Rectangle::new(
            Point::new(plot.origin.x, plot.origin.y - 2.0),
            Size::new(plot.size.width, plot.size.height + 4.0),
        )));
        if let Some(i) = hot {
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
        for (i, c) in self.candles.iter().enumerate() {
            let color = if c.rising() { p.success } else { p.danger };
            let x = x_of(i);
            // Wick: one crisp pixel.
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(x - 0.5, y_of(c.high)),
                    Size::new(1.0, (y_of(c.low) - y_of(c.high)).max(1.0)),
                ),
                fill: color,
                radius: 0.0,
                border: None,
                shadow: None,
            });
            let (top, bot) = (y_of(c.open.max(c.close)), y_of(c.open.min(c.close)));
            let body = Rectangle::new(
                Point::new(x - body_w / 2.0, top),
                Size::new(body_w, (bot - top).max(1.0)),
            );
            out.push(DrawCommand::Quad {
                bounds: body,
                // Rising: tinted body with a solid outline. Falling: solid.
                fill: if c.rising() {
                    util::mix(p.surface, color, 0.22)
                } else {
                    color
                },
                radius: t.radii.small.min(body_w / 2.0),
                border: c.rising().then_some(Border { color, width: 1.0 }),
                shadow: None,
            });
        }
        out.push(DrawCommand::PopClip);

        if vol_h > 0.0 {
            let vmax = self
                .candles
                .iter()
                .filter_map(|c| c.volume)
                .fold(0.0, f32::max)
                .max(f32::EPSILON);
            let top = plot.origin.y + plot.size.height + gap;
            for (i, c) in self.candles.iter().enumerate() {
                let Some(v) = c.volume else { continue };
                let h = (vol_h * (v / vmax)).max(1.0);
                let color = if c.rising() { p.success } else { p.danger };
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(x_of(i) - body_w / 2.0, top + vol_h - h),
                        Size::new(body_w, h),
                    ),
                    fill: color.with_alpha(if hot == Some(i) { 0.7 } else { 0.38 }),
                    radius: t.radii.small.min(body_w / 2.0),
                    border: None,
                    shadow: None,
                });
            }
        }

        if let Some(i) = hot {
            let c = &self.candles[i];
            let color = if c.rising() { p.success } else { p.danger };
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(x_of(i) - 0.5, plot.origin.y),
                    Size::new(1.0, plot.size.height),
                ),
                fill: p.border_strong.with_alpha(0.45),
                radius: 0.0,
                border: None,
                shadow: None,
            });
            let money = |v: f32| format!("{}{}", self.unit, format_exact(v));
            let mut rows = vec![
                ("Open", money(c.open)),
                ("High", money(c.high)),
                ("Low", money(c.low)),
                ("Close", money(c.close)),
            ];
            if let Some(v) = c.volume {
                rows.push(("Volume", format_exact(v)));
            }
            let pct = (c.close - c.open) / c.open.abs().max(f32::EPSILON) * 100.0;
            let change = format!("{}{:.2}%", if pct >= 0.0 { "+" } else { "" }, pct);
            let label_w = rows.iter().map(|r| width_of(r.0, 400)).fold(0.0, f32::max);
            let value_w = rows.iter().map(|r| width_of(&r.1, 600)).fold(0.0, f32::max);
            let head_w = width_of(&c.label, 600) + 14.0 + width_of(&change, 600);
            let inner = (label_w + 16.0 + value_w).max(head_w);
            let padp = 8.0;
            let head_h = cap.line_height + 4.0;
            let (bw, bh) = (
                inner + 2.0 * padp,
                head_h + rows.len() as f32 * cap.line_height + 2.0 * padp,
            );
            let mut x = x_of(i) + 16.0;
            if x + bw > b.origin.x + b.size.width {
                x = x_of(i) - 16.0 - bw;
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
            cmds.push(label_cmd(
                Point::new(x + padp, y + padp),
                c.label.clone(),
                &cap,
                600,
                p.text,
            ));
            cmds.push(label_cmd(
                Point::new(x + padp + inner - width_of(&change, 600), y + padp),
                change,
                &cap,
                600,
                color,
            ));
            let mut ty = y + padp + head_h;
            for (k, v) in rows {
                cmds.push(label_cmd(
                    Point::new(x + padp, ty),
                    k,
                    &cap,
                    400,
                    p.text_secondary,
                ));
                cmds.push(label_cmd(
                    Point::new(x + padp + inner - width_of(&v, 600), ty),
                    v,
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
