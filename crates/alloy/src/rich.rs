//! Richer widgets: badges, swatches, charts with axes, sparklines, trees and a colour picker.

use crate::{
    bind::Val,
    cx::{self, auto_id},
    local,
    view::Node,
    Act, Cell,
};
use alloy_ui_core::Color;
use alloy_ui_widgets as w;
use std::sync::Arc;

pub use w::{Series, Tone, TreeItem};

// ---------------------------------------------------------------- badge

/// A status pill under construction. See [`badge`].
pub struct Badge {
    label: String,
    tone: Tone,
    solid: bool,
    outline: bool,
    dot: bool,
}

/// A small pill: `badge("Active").success().dot()`, `badge("12")`, `badge("beta").accent().outline()`.
pub fn badge(label: impl ToString) -> Badge {
    Badge {
        label: label.to_string(),
        tone: Tone::Neutral,
        solid: false,
        outline: false,
        dot: false,
    }
}

impl Badge {
    /// Healthy, done.
    pub fn success(mut self) -> Self {
        self.tone = Tone::Success;
        self
    }
    /// Needs attention.
    pub fn warning(mut self) -> Self {
        self.tone = Tone::Warning;
        self
    }
    /// Failed, blocked.
    pub fn danger(mut self) -> Self {
        self.tone = Tone::Danger;
        self
    }
    /// Highlighted (the theme accent).
    pub fn accent(mut self) -> Self {
        self.tone = Tone::Accent;
        self
    }
    /// Pick the tone from a status word: `Up`, `Degraded`, `Failed`, ... (see the status tones).
    pub fn auto(mut self) -> Self {
        let l = self.label.to_lowercase();
        let any = |ws: &[&str]| ws.iter().any(|w| l == *w);
        self.tone = if any(&[
            "up", "ok", "online", "active", "done", "healthy", "passed", "success", "paid",
        ]) {
            Tone::Success
        } else if any(&[
            "degraded",
            "warning",
            "slow",
            "pending",
            "paused",
            "review",
            "idle",
            "in progress",
            "draft",
        ]) {
            Tone::Warning
        } else if any(&[
            "down", "failed", "error", "offline", "critical", "blocked", "overdue",
        ]) {
            Tone::Danger
        } else {
            Tone::Neutral
        };
        self
    }
    /// Filled with the tone.
    pub fn solid(mut self) -> Self {
        self.solid = true;
        self
    }
    /// Hairline instead of a tinted fill.
    pub fn outline(mut self) -> Self {
        self.outline = true;
        self
    }
    /// A status dot before the label.
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }
}

impl<M: 'static> From<Badge> for Node<M> {
    fn from(b: Badge) -> Self {
        let mut e = w::Badge::new(&cx::ui(), b.label).tone(b.tone);
        if b.solid {
            e = e.solid();
        }
        if b.outline {
            e = e.outline();
        }
        if b.dot {
            e = e.dot();
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- swatch

/// A colour swatch under construction. See [`swatch`].
pub struct Swatch<M> {
    color: Color,
    size: f32,
    selected: bool,
    hex: bool,
    caption: Option<String>,
    on: Option<M>,
    id: u64,
}

/// A square of colour: `swatch(Color::hex(0x0B6B5F)).hex()`. Add `.selected(true)` and
/// `.act(|s| s.accent = c)` for a palette picker.
#[track_caller]
pub fn swatch<M>(color: Color) -> Swatch<M> {
    Swatch {
        color,
        size: 28.0,
        selected: false,
        hex: false,
        caption: None,
        on: None,
        id: auto_id(40),
    }
}

impl<M> Swatch<M> {
    /// Side length in pixels.
    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }
    /// Ring and tick on the chosen swatch.
    pub fn selected(mut self, yes: bool) -> Self {
        self.selected = yes;
        self
    }
    /// Caption it with its hex value.
    pub fn hex(mut self) -> Self {
        self.hex = true;
        self
    }
    /// Caption it with a name.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.caption = Some(name.into());
        self
    }
    /// Message sent when pressed.
    pub fn on(mut self, m: M) -> Self {
        self.on = Some(m);
        self
    }
}
impl<S: 'static> Swatch<Act<S>> {
    /// Closure style: `.act(move |s| s.accent = c)`.
    pub fn act(self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.on(Act::new(f))
    }
}
impl<M: Clone + 'static> From<Swatch<M>> for Node<M> {
    fn from(s: Swatch<M>) -> Self {
        let mut e = w::Swatch::new(&cx::ui(), s.color)
            .size(s.size)
            .selected(s.selected)
            .id(s.id);
        if s.hex {
            e = e.hex();
        }
        if let Some(c) = s.caption {
            e = e.caption(c);
        }
        if let Some(m) = s.on {
            e = e.on_press(m);
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- charts

/// A line or bar chart under construction. See [`line_chart`] and [`bar_chart`].
pub struct Chart {
    bars: bool,
    series: Vec<Series>,
    labels: Vec<String>,
    height: Option<f32>,
    area: bool,
    legend: bool,
    unit: String,
    zoom: Option<u64>,
}

fn chart(bars: bool, series: Vec<Series>) -> Chart {
    Chart {
        bars,
        series,
        labels: Vec::new(),
        height: None,
        area: false,
        legend: true,
        unit: String::new(),
        zoom: None,
    }
}

/// A line chart with a y axis, gridlines and a hover readout:
/// `line_chart([("Requests", reqs), ("Errors", errs)]).labels(days).area()`.
pub fn line_chart<N: Into<String>, V: Into<Vec<f32>>>(
    series: impl IntoIterator<Item = (N, V)>,
) -> Chart {
    chart(
        false,
        series.into_iter().map(|(n, v)| Series::new(n, v)).collect(),
    )
}

/// A grouped bar chart with a y axis, same shape as [`line_chart`].
pub fn bar_chart<N: Into<String>, V: Into<Vec<f32>>>(
    series: impl IntoIterator<Item = (N, V)>,
) -> Chart {
    chart(
        true,
        series.into_iter().map(|(n, v)| Series::new(n, v)).collect(),
    )
}

impl Chart {
    /// Names for the x positions.
    pub fn labels<S: Into<String>>(mut self, labels: impl IntoIterator<Item = S>) -> Self {
        self.labels = labels.into_iter().map(Into::into).collect();
        self
    }
    /// Total height in pixels (default 220).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
    /// Fill under lines.
    pub fn area(mut self) -> Self {
        self.area = true;
        self
    }
    /// Hide the legend.
    pub fn no_legend(mut self) -> Self {
        self.legend = false;
        self
    }
    /// Suffix for hover values, e.g. `"ms"`.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }
    /// Scroll to zoom and drag to pan along the x axis; a "Reset zoom" chip (or `0`) restores
    /// the full range and the y axis rescales to what is visible.
    #[track_caller]
    pub fn zoom(mut self) -> Self {
        self.zoom = Some(auto_id(70));
        self
    }
}

impl<M: 'static> From<Chart> for Node<M> {
    fn from(c: Chart) -> Self {
        let ui = cx::ui();
        let mut e = if c.bars {
            w::Chart::bars(&ui, c.series)
        } else {
            w::Chart::line(&ui, c.series)
        }
        .x_labels(c.labels)
        .unit(c.unit);
        if let Some(h) = c.height {
            e = e.height(h);
        }
        if c.area {
            e = e.area();
        }
        if !c.legend {
            e = e.no_legend();
        }
        if let Some(id) = c.zoom {
            e = e.zoomable(id);
        }
        Node(e.into_element())
    }
}

/// A sparkline under construction. See [`sparkline`].
pub struct Spark {
    values: Vec<f32>,
    color: Option<Color>,
    width: Option<f32>,
}

/// A tiny trend line for a table cell or stat card: `sparkline(series(24)).w(120.0)`.
pub fn sparkline(values: impl IntoIterator<Item = f32>) -> Spark {
    Spark {
        values: values.into_iter().collect(),
        color: None,
        width: None,
    }
}

impl Spark {
    /// Line colour (default the accent).
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Width in pixels (default 96).
    pub fn w(mut self, px: f32) -> Self {
        self.width = Some(px);
        self
    }
}

impl<M: 'static> From<Spark> for Node<M> {
    fn from(s: Spark) -> Self {
        let mut e = w::Sparkline::new(&cx::ui(), s.values);
        if let Some(c) = s.color {
            e = e.color(c);
        }
        if let Some(px) = s.width {
            e = e.width(alloy_ui_core::style::Length::Fixed(px));
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- tree

type Pick<S> = Arc<dyn Fn(&mut S, u64) + Send + Sync>;

/// A tree under construction. See [`tree`].
pub struct Tree<S> {
    items: Vec<TreeItem>,
    selected: Option<Option<u64>>,
    open: Vec<u64>,
    on: Option<Pick<S>>,
    id: u64,
}

/// An expandable outline for `sketch` apps. It remembers which branches are open and which row
/// is selected itself; react to picks with `.act(|s, id| ..)`, or pin the selection with
/// `.selected(..)`. (Elm-style apps use [`widgets::Tree`](alloy_ui_widgets::Tree) directly.)
#[track_caller]
pub fn tree<S>(items: impl IntoIterator<Item = TreeItem>) -> Tree<S> {
    Tree {
        items: items.into_iter().collect(),
        selected: None,
        open: Vec::new(),
        on: None,
        id: auto_id(41),
    }
}

impl<S: 'static> Tree<S> {
    /// Branches that start open.
    pub fn open(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.open = ids.into_iter().collect();
        self
    }
    /// Control the selection yourself.
    pub fn selected(mut self, id: Option<u64>) -> Self {
        self.selected = Some(id);
        self
    }
    /// Called with the id of a picked row.
    pub fn act(mut self, f: impl Fn(&mut S, u64) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}
impl<S: 'static> From<Tree<S>> for Node<Act<S>> {
    fn from(t: Tree<S>) -> Self {
        let expanded = local(t.open.clone());
        let picked = local(None::<u64>);
        let sel = t.selected.unwrap_or_else(|| picked.get());
        let on = t.on;
        let e = w::Tree::new(&cx::ui(), t.items)
            .expanded(expanded.get())
            .selected(sel)
            .id(t.id)
            .on_toggle(move |id| {
                Act::new(move |_: &mut S| {
                    expanded.with(|v| match v.iter().position(|x| *x == id) {
                        Some(i) => {
                            v.remove(i);
                        }
                        None => v.push(id),
                    })
                })
            })
            .on_select(move |id| {
                let on = on.clone();
                Act::new(move |s: &mut S| {
                    picked.set(Some(id));
                    if let Some(f) = &on {
                        f(s, id);
                    }
                })
            });
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- colour picker

/// A colour picker under construction. See [`color_picker`].
pub struct Picker<M> {
    color: Color,
    on: Option<Box<dyn Fn(Color) -> M>>,
    id: u64,
}

/// A saturation/value field, hue strip and hex readout: `color_picker(at!(s.accent))`.
#[track_caller]
pub fn color_picker<M>(color: impl Val<Color, M>) -> Picker<M> {
    let (color, on) = color.split();
    Picker {
        color,
        on,
        id: auto_id(42),
    }
}

impl<M: 'static> Picker<M> {
    /// Called with the new colour while dragging.
    pub fn on(mut self, f: impl Fn(Color) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
}
impl<S: 'static> Picker<Act<S>> {
    /// Closure style: `.act(|s, c| s.accent = c)`.
    pub fn act(self, f: impl Fn(&mut S, Color) + Send + Sync + 'static) -> Self {
        let f = Arc::new(f);
        self.on(move |c| {
            let f = f.clone();
            Act::new(move |s| f(s, c))
        })
    }
    /// Write the colour into a view-local cell.
    pub fn local(self, cell: &impl Cell<Color>) -> Self {
        let cell = *cell;
        self.on(move |c| Act::new(move |_| cell.write(c)))
    }
}
impl<M: 'static> From<Picker<M>> for Node<M> {
    fn from(p: Picker<M>) -> Self {
        let mut e = w::ColorPicker::new(&cx::ui(), p.id, p.color);
        if let Some(f) = p.on {
            e = e.on_change(f);
        }
        Node(e.into_element())
    }
}
