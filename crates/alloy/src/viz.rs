//! Visual building blocks: pie and donut, candlesticks, live charts, kanban, calendar,
//! node graph, canvas, 3D viewport and media player.
//!
//! ```ignore
//! pie([("Rent", 1200.0), ("Food", 540.0), ("Travel", 310.0)]).donut().unit(" €")
//! candles(prices).unit("$").height(260.0)
//! stream([("CPU", &s.cpu), ("GPU", &s.gpu)]).unit("%").window("last 60 s")
//! kanban(&s.lanes).act(|s: &mut App, mv| mv.apply(&mut s.lanes))
//! calendar(s.day, s.view, &s.entries).today(TODAY).act(|s: &mut App, a| s.calendar(a))
//! node_graph(&s.nodes, &s.wires).selected(s.sel).act(|s: &mut App, a| s.graph(a))
//! canvas(320.0, 160.0, |f| f.circle(Point::new(40.0, 40.0), 12.0, f.palette().accent))
//! viewport3d([object(Mesh::torus(1.6, 0.5, 36)).color(Color::hex(0x3558A8))]).spin(0.4)
//! media(TestCard::video(90), s.playback).act(|s: &mut App, a| s.play(a))
//! ```

use crate::{
    cx::{self, auto_id},
    view::Node,
    Act,
};
use alloy_ui_core::{Color, Point};
use alloy_ui_layout::Length;
use alloy_ui_widgets as w;
use std::{rc::Rc, sync::Arc};

pub use w::{
    CalAction, CalView, Candle, Card, Entry, Frame, GraphAction, GraphNode, Lane, MediaAction,
    MediaSource, Mesh, Move, Object, Orbit, Path, Playback, Pointer, Ring, Slice, TestCard, Wire,
};

/// A state-mutating callback taking the widget's event.
type Cb<S, T> = Arc<dyn Fn(&mut S, T) + Send + Sync>;

// ---------------------------------------------------------------- pie

/// A pie or donut under construction. See [`pie`].
pub struct Pie<S> {
    slices: Vec<Slice>,
    donut: bool,
    height: Option<f32>,
    unit: String,
    title: Option<String>,
    legend: bool,
    selected: Option<usize>,
    on: Option<Cb<S, usize>>,
}

/// A pie of `(name, value)` pairs with a legend, hover lift and tooltip:
/// `pie([("Rent", 1200.0), ("Food", 540.0)]).donut()`.
pub fn pie<S, N: Into<String>>(slices: impl IntoIterator<Item = (N, f32)>) -> Pie<S> {
    Pie {
        slices: slices.into_iter().map(|(n, v)| Slice::new(n, v)).collect(),
        donut: false,
        height: None,
        unit: String::new(),
        title: None,
        legend: true,
        selected: None,
        on: None,
    }
}

impl<S: 'static> Pie<S> {
    /// A ring with the total (or hovered slice) in the hole.
    pub fn donut(mut self) -> Self {
        self.donut = true;
        self
    }
    /// Caption above the total in a donut (default "Total").
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
    /// Suffix for values, e.g. `"%"`.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = u.into();
        self
    }
    /// Height in pixels (default 180).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
    /// Hide the legend.
    pub fn no_legend(mut self) -> Self {
        self.legend = false;
        self
    }
    /// Keep a slice lifted.
    pub fn selected(mut self, i: Option<usize>) -> Self {
        self.selected = i;
        self
    }
    /// Called with the index of a clicked slice or legend row.
    pub fn act(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<Pie<S>> for Node<Act<S>> {
    fn from(p: Pie<S>) -> Self {
        let mut e = w::PieChart::<Act<S>>::new(&cx::ui(), p.slices)
            .unit(p.unit)
            .selected(p.selected);
        if p.donut {
            e = e.donut();
        }
        if let Some(t) = p.title {
            e = e.title(t);
        }
        if let Some(h) = p.height {
            e = e.height(h);
        }
        if !p.legend {
            e = e.no_legend();
        }
        if let Some(f) = p.on {
            e = e.on_select(move |i| {
                let f = f.clone();
                Act::new(move |s: &mut S| f(s, i))
            });
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- candles

/// A candlestick chart under construction. See [`candles`].
pub struct Candles {
    data: Vec<Candle>,
    height: Option<f32>,
    unit: String,
}

/// Candlesticks from [`Candle`]s, oldest first, with a price axis, optional volume bars and a
/// crosshair readout: `candles([candle("Mon", 10.0, 12.0, 9.5, 11.5)])`.
pub fn candles(data: impl IntoIterator<Item = Candle>) -> Candles {
    Candles {
        data: data.into_iter().collect(),
        height: None,
        unit: String::new(),
    }
}

/// One period: label, open, high, low, close. Add `.volume(v)` for the volume bars.
pub fn candle(label: impl Into<String>, open: f32, high: f32, low: f32, close: f32) -> Candle {
    Candle::new(label, open, high, low, close)
}

impl Candles {
    /// Prefix for prices, e.g. `"$"`.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = u.into();
        self
    }
    /// Height in pixels (default 240).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
}

impl<M: 'static> From<Candles> for Node<M> {
    fn from(c: Candles) -> Self {
        let mut e = w::CandleChart::new(&cx::ui(), c.data).unit(c.unit);
        if let Some(h) = c.height {
            e = e.height(h);
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- stream

/// A live chart under construction. See [`stream`].
pub struct Stream {
    series: Vec<(String, Ring)>,
    id: u64,
    height: Option<f32>,
    unit: String,
    window: Option<String>,
    range: Option<(f32, f32)>,
    interval: u64,
    smooth: bool,
    area: bool,
}

/// A line chart that scrolls as samples arrive. Keep one [`Ring`] per series in your state and
/// push on a timer (`every(..)`): `stream([("CPU", &s.cpu)]).unit("%").interval(1000)`.
#[track_caller]
pub fn stream<'a, N: Into<String>>(series: impl IntoIterator<Item = (N, &'a Ring)>) -> Stream {
    Stream {
        series: series
            .into_iter()
            .map(|(n, r)| (n.into(), r.clone()))
            .collect(),
        id: auto_id(61),
        height: None,
        unit: String::new(),
        window: None,
        range: None,
        interval: 1000,
        smooth: true,
        area: true,
    }
}

impl Stream {
    /// Suffix for values, e.g. `"%"`.
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = u.into();
        self
    }
    /// A caption for the visible window, e.g. `"last 60 s"`.
    pub fn window(mut self, label: impl Into<String>) -> Self {
        self.window = Some(label.into());
        self
    }
    /// Fix the y axis.
    pub fn range(mut self, lo: f32, hi: f32) -> Self {
        self.range = Some((lo, hi));
        self
    }
    /// Milliseconds between samples (default 1000); new samples glide in over this time.
    pub fn interval(mut self, ms: u64) -> Self {
        self.interval = ms;
        self
    }
    /// Redraw once per sample instead of gliding, so the app idles in between.
    pub fn stepped(mut self) -> Self {
        self.smooth = false;
        self
    }
    /// No fill under the lines.
    pub fn no_area(mut self) -> Self {
        self.area = false;
        self
    }
    /// Height in pixels (default 160).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
}

impl<M: 'static> From<Stream> for Node<M> {
    fn from(s: Stream) -> Self {
        let mut e = w::StreamChart::new(
            &cx::ui(),
            s.id,
            s.series.iter().map(|(n, r)| (n.as_str(), r)),
        )
        .unit(s.unit)
        .interval_ms(s.interval)
        .smooth(s.smooth);
        if let Some(l) = s.window {
            e = e.window_label(l);
        }
        if let Some((lo, hi)) = s.range {
            e = e.range(lo, hi);
        }
        if let Some(h) = s.height {
            e = e.height(h);
        }
        if !s.area {
            e = e.no_area();
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- kanban

/// A kanban board under construction. See [`kanban`].
pub struct Kanban<S> {
    lanes: Vec<Lane>,
    id: u64,
    height: Option<Length>,
    on_move: Option<Cb<S, Move>>,
    on_open: Option<Cb<S, u64>>,
}

/// A card with a stable `id` and a title. Add `.detail(..)` and `.tag("Blocked", Tone::Danger)`.
pub fn card(id: u64, title: impl Into<String>) -> Card {
    Card::new(id, title)
}

/// A named column of cards.
pub fn lane(title: impl Into<String>, cards: impl IntoIterator<Item = Card>) -> Lane {
    Lane::new(title, cards.into_iter().collect::<Vec<_>>())
}

/// Columns of cards you drag between and within. Your state owns the lanes; the board reports
/// a [`Move`] and `mv.apply(&mut lanes)` applies it:
/// `kanban(&s.lanes).act(|s: &mut App, mv| mv.apply(&mut s.lanes))`.
#[track_caller]
pub fn kanban<S>(lanes: &[Lane]) -> Kanban<S> {
    Kanban {
        lanes: lanes.to_vec(),
        id: auto_id(62),
        height: None,
        on_move: None,
        on_open: None,
    }
}

impl<S: 'static> Kanban<S> {
    /// Called when a card is dropped somewhere new (drag, or Alt+arrows with the board focused).
    pub fn act(mut self, f: impl Fn(&mut S, Move) + Send + Sync + 'static) -> Self {
        self.on_move = Some(Arc::new(f));
        self
    }
    /// Called with a card's id when it is clicked or opened with Enter.
    pub fn open(mut self, f: impl Fn(&mut S, u64) + Send + Sync + 'static) -> Self {
        self.on_open = Some(Arc::new(f));
        self
    }
    /// Fixed height in pixels (default: fill).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(Length::Fixed(px));
        self
    }
}

impl<S: 'static> From<Kanban<S>> for Node<Act<S>> {
    fn from(k: Kanban<S>) -> Self {
        let mut e = w::Kanban::<Act<S>>::new(&cx::ui(), k.id, k.lanes);
        if let Some(h) = k.height {
            e = e.height(h);
        }
        if let Some(f) = k.on_move {
            e = e.on_move(move |mv| {
                let f = f.clone();
                Act::new(move |s: &mut S| f(s, mv))
            });
        }
        if let Some(f) = k.on_open {
            e = e.on_open(move |id| {
                let f = f.clone();
                Act::new(move |s: &mut S| f(s, id))
            });
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- calendar

/// A calendar under construction. See [`calendar`].
pub struct Calendar<S> {
    anchor: w::Date,
    view: CalView,
    entries: Vec<Entry>,
    id: u64,
    today: Option<w::Date>,
    now: Option<u32>,
    hours: Option<(u32, u32)>,
    on: Option<Cb<S, CalAction>>,
}

/// An entry on `date` from `start` to `end` minutes after midnight: `entry(1, "Standup", d, 9 * 60 + 30, 10 * 60)`.
pub fn entry(id: u64, title: impl Into<String>, date: w::Date, start: u32, end: u32) -> Entry {
    Entry::new(id, title, date, start, end)
}

/// A month, week or day calendar around `anchor`. Drag entries to move them, drag their bottom
/// edge to resize, click an empty slot to create. Every interaction arrives as a
/// [`CalAction`] in `.act`.
#[track_caller]
pub fn calendar<S>(anchor: w::Date, view: CalView, entries: &[Entry]) -> Calendar<S> {
    Calendar {
        anchor,
        view,
        entries: entries.to_vec(),
        id: auto_id(63),
        today: None,
        now: None,
        hours: None,
        on: None,
    }
}

impl<S: 'static> Calendar<S> {
    /// Today's date: its number gets the accent.
    pub fn today(mut self, d: w::Date) -> Self {
        self.today = Some(d);
        self
    }
    /// The current time in minutes after midnight, for the "now" line.
    pub fn now(mut self, minute: u32) -> Self {
        self.now = Some(minute);
        self
    }
    /// First and last hour of the time grid (default 7 to 20).
    pub fn hours(mut self, from: u32, to: u32) -> Self {
        self.hours = Some((from, to));
        self
    }
    /// Called for every interaction: navigation, view switch, open, create, move, resize.
    pub fn act(mut self, f: impl Fn(&mut S, CalAction) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<Calendar<S>> for Node<Act<S>> {
    fn from(c: Calendar<S>) -> Self {
        let mut e = w::Calendar::<Act<S>>::new(&cx::ui(), c.id, c.anchor, c.view, c.entries);
        if let Some(d) = c.today {
            e = e.today(d);
        }
        if let Some(m) = c.now {
            e = e.now(m);
        }
        if let Some((a, z)) = c.hours {
            e = e.hours(a, z);
        }
        if let Some(f) = c.on {
            e = e.on_action(move |a| {
                let f = f.clone();
                Act::new(move |s: &mut S| f(s, a))
            });
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- node graph

/// A node graph under construction. See [`node_graph`].
pub struct NodeGraph<S> {
    nodes: Vec<GraphNode>,
    wires: Vec<Wire>,
    id: u64,
    selected: Option<u64>,
    height: Option<Length>,
    on: Option<Cb<S, GraphAction>>,
}

/// A node with ports: `node(1, "Blur", 120.0, 40.0).inputs(["Image"]).outputs(["Result"])`.
pub fn node(id: u64, title: impl Into<String>, x: f32, y: f32) -> GraphNode {
    GraphNode::new(id, title, x, y)
}

/// A pannable, zoomable graph of nodes and wires. Drag headers to move, drag from an output to
/// an input to connect, click a wire and press Delete to remove it. Edits arrive as
/// [`GraphAction`]s in `.act`; the graph lives in your state.
#[track_caller]
pub fn node_graph<S>(nodes: &[GraphNode], wires: &[Wire]) -> NodeGraph<S> {
    NodeGraph {
        nodes: nodes.to_vec(),
        wires: wires.to_vec(),
        id: auto_id(64),
        selected: None,
        height: None,
        on: None,
    }
}

impl<S: 'static> NodeGraph<S> {
    /// The selected node.
    pub fn selected(mut self, id: Option<u64>) -> Self {
        self.selected = id;
        self
    }
    /// Fixed height in pixels (default: fill).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(Length::Fixed(px));
        self
    }
    /// Called for every edit: move, select, connect, disconnect, delete.
    pub fn act(mut self, f: impl Fn(&mut S, GraphAction) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<NodeGraph<S>> for Node<Act<S>> {
    fn from(g: NodeGraph<S>) -> Self {
        let mut e =
            w::NodeGraph::<Act<S>>::new(&cx::ui(), g.id, g.nodes, g.wires).selected(g.selected);
        if let Some(h) = g.height {
            e = e.height(h);
        }
        if let Some(f) = g.on {
            e = e.on_action(move |a| {
                let f = f.clone();
                Act::new(move |s: &mut S| f(s, a))
            });
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- canvas

/// A canvas under construction. See [`canvas`].
pub struct Canvas<S> {
    w: Length,
    h: Length,
    id: u64,
    paint: Rc<dyn Fn(&mut Frame)>,
    on: Option<Cb<S, Pointer>>,
    live: bool,
}

/// A `width` by `height` pixel drawing surface painted by `paint` every frame. Draw with
/// [`Frame`]: `rect`, `circle`, `line`, `text`, and `fill`/`stroke` of a [`Path`].
#[track_caller]
pub fn canvas<S>(width: f32, height: f32, paint: impl Fn(&mut Frame) + 'static) -> Canvas<S> {
    Canvas {
        w: Length::Fixed(width),
        h: Length::Fixed(height),
        id: auto_id(65),
        paint: Rc::new(paint),
        on: None,
        live: false,
    }
}

impl<S: 'static> Canvas<S> {
    /// Fill the available width instead of a fixed one.
    pub fn fill_width(mut self) -> Self {
        self.w = Length::Fill(1);
        self
    }
    /// Repaint every frame (live content). Without it the canvas repaints only when something
    /// else asks for a frame.
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }
    /// Called for pointer down, move (while over or dragging) and up, in canvas coordinates.
    pub fn act(mut self, f: impl Fn(&mut S, Pointer) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<Canvas<S>> for Node<Act<S>> {
    fn from(c: Canvas<S>) -> Self {
        let paint = c.paint;
        let mut e = w::Canvas::<Act<S>>::new(&cx::ui(), c.id, c.w, c.h, move |f| paint(f));
        if c.live {
            e = e.live();
        }
        if let Some(f) = c.on {
            e = e.on_pointer(move |p| {
                let f = f.clone();
                Some(Act::new(move |s: &mut S| f(s, p)))
            });
        }
        Node(e.into_element())
    }
}

/// A coloured point, for canvas code: `pt(10.0, 20.0)`.
pub fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

// ---------------------------------------------------------------- 3D

/// A viewport under construction. See [`viewport3d`].
pub struct Viewport3d {
    objects: Vec<Object>,
    id: u64,
    spin: f32,
    grid: bool,
    height: Option<f32>,
    orbit: Option<Orbit>,
}

/// A mesh to place in a [`viewport3d`]: `object(Mesh::cube(1.0)).at(0.0, 0.5, 0.0).color(c)`.
pub fn object(mesh: Mesh) -> Object {
    Object::new(mesh)
}

/// Lit 3D meshes on an orbit camera (drag to orbit, scroll to zoom), drawn with the 2D renderer
/// so it looks identical on every back end.
#[track_caller]
pub fn viewport3d(objects: impl IntoIterator<Item = Object>) -> Viewport3d {
    Viewport3d {
        objects: objects.into_iter().collect(),
        id: auto_id(66),
        spin: 0.0,
        grid: true,
        height: None,
        orbit: None,
    }
}

impl Viewport3d {
    /// Turntable speed in radians per second. This keeps the app redrawing; leave it off for a
    /// static preview.
    pub fn spin(mut self, rad_per_sec: f32) -> Self {
        self.spin = rad_per_sec;
        self
    }
    /// Hide the floor grid.
    pub fn no_grid(mut self) -> Self {
        self.grid = false;
        self
    }
    /// Starting camera.
    pub fn orbit(mut self, o: Orbit) -> Self {
        self.orbit = Some(o);
        self
    }
    /// Height in pixels (default 280).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
}

impl<M: 'static> From<Viewport3d> for Node<M> {
    fn from(v: Viewport3d) -> Self {
        let mut e = w::Viewport3d::new(&cx::ui(), v.id, v.objects)
            .spin(v.spin)
            .grid(v.grid);
        if let Some(h) = v.height {
            e = e.height(Length::Fixed(h));
        }
        if let Some(o) = v.orbit {
            e = e.orbit(o);
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- media

/// A media player under construction. See [`media`].
pub struct Media<S> {
    source: Rc<dyn MediaSource>,
    state: Playback,
    id: u64,
    title: Option<String>,
    height: Option<f32>,
    on: Option<Cb<S, MediaAction>>,
}

/// A player: picture area, play/pause, seek bar, volume. `source` is anything implementing
/// [`MediaSource`] (Alloy ships no decoder; [`TestCard`] is a demo). Keep a [`Playback`] in your
/// state and feed it the actions: `s.playback.apply(a, ui().now(), duration)`.
#[track_caller]
pub fn media<S>(source: impl MediaSource + 'static, state: Playback) -> Media<S> {
    Media {
        source: Rc::new(source),
        state,
        id: auto_id(67),
        title: None,
        height: None,
        on: None,
    }
}

impl<S: 'static> Media<S> {
    /// A name shown over the picture.
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
    /// Height in pixels (default 320).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
    /// Called for play/pause, seek, volume and mute.
    pub fn act(mut self, f: impl Fn(&mut S, MediaAction) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<Media<S>> for Node<Act<S>> {
    fn from(m: Media<S>) -> Self {
        let mut e = w::MediaPlayer::<Act<S>>::new(&cx::ui(), m.id, m.source, m.state);
        if let Some(t) = m.title {
            e = e.title(t);
        }
        if let Some(h) = m.height {
            e = e.height(Length::Fixed(h));
        }
        if let Some(f) = m.on {
            e = e.on_action(move |a| {
                let f = f.clone();
                Act::new(move |s: &mut S| f(s, a))
            });
        }
        Node(e.into_element())
    }
}

/// A [`Color`] from `0xRRGGBB`, handy inside canvas and 3D code.
pub fn rgb(hex: u32) -> Color {
    Color::hex(hex)
}
