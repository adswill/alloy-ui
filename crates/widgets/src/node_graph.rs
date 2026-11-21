//! Node graph editor: boxes with ports, wires between them, pan and zoom.

use crate::badge::Tone;
use crate::util::{self, mix};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

/// One box in the graph. Positions are in graph units (zoom 1.0 = pixels).
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    /// Stable identity.
    pub id: u64,
    /// Header text.
    pub title: String,
    /// Top-left in graph space.
    pub pos: Point,
    /// Input port names, drawn down the left edge.
    pub inputs: Vec<String>,
    /// Output port names, drawn down the right edge.
    pub outputs: Vec<String>,
    /// Colour role of the header dot and its outgoing wires.
    pub tone: Tone,
    /// Width in graph units.
    pub width: f32,
}

impl GraphNode {
    /// A node at `(x, y)` with no ports yet.
    pub fn new(id: u64, title: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            id,
            title: title.into(),
            pos: Point::new(x, y),
            inputs: Vec::new(),
            outputs: Vec::new(),
            tone: Tone::Neutral,
            width: 180.0,
        }
    }
    /// Input ports, top to bottom.
    pub fn inputs<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.inputs = names.into_iter().map(Into::into).collect();
        self
    }
    /// Output ports, top to bottom.
    pub fn outputs<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.outputs = names.into_iter().map(Into::into).collect();
        self
    }
    /// Colour role.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
    /// Width in graph units (default 180).
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
}

/// A connection from output port `from.1` of node `from.0` to input port
/// `to.1` of node `to.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wire {
    /// `(node id, output index)`.
    pub from: (u64, usize),
    /// `(node id, input index)`.
    pub to: (u64, usize),
}

impl Wire {
    /// A wire from `(node, output)` to `(node, input)`.
    pub fn new(from: (u64, usize), to: (u64, usize)) -> Self {
        Self { from, to }
    }
}

/// What the user did. Your `update` applies it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphAction {
    /// A node is being dragged: set its `pos` (sent on every pointer move).
    Move {
        /// The node.
        id: u64,
        /// New top-left in graph space.
        pos: Point,
    },
    /// A node was clicked, or `None` when empty space was.
    Select(Option<u64>),
    /// An output was dragged onto an input: add this wire (replace any wire
    /// already ending at that input if your graph allows one per input).
    Connect(Wire),
    /// A wire was selected and Delete pressed.
    Disconnect(Wire),
    /// A node was selected and Delete pressed.
    Delete(u64),
}

type Handler<M> = Rc<dyn Fn(GraphAction) -> M>;

/// A pannable, zoomable canvas of nodes joined by wires. Controlled: nodes,
/// wires and selection live in your state; the widget reports
/// [`GraphAction`]s. Drag a header to move a node, drag from an output port
/// (right edge) to an input port (left edge) to connect, click a wire and press
/// Delete to remove it, drag the background to pan and scroll to zoom. Zoom and
/// pan are view state kept by the widget (press `0` to reset them, `F` to fit every node,
/// `+` and `-` to zoom). An optional minimap shows the whole graph.
pub struct NodeGraph<M> {
    ui: Ui,
    id: u64,
    nodes: Vec<GraphNode>,
    wires: Vec<Wire>,
    selected: Option<u64>,
    height: Length,
    minimap: bool,
    on_action: Option<Handler<M>>,
}

const HEADER: f32 = 30.0;
const ROW: f32 = 24.0;
const PAD: f32 = 10.0;
const PORT_R: f32 = 5.0;
const K_PAN: u64 = 0x4E47_0001; // pan x, y in 1/16 px (two slots)
const K_PAN_Y: u64 = 0x4E47_0002;
const K_ZOOM: u64 = 0x4E47_0003; // thousandths
const K_DRAG: u64 = 0x4E47_0004; // node being dragged
const K_GX: u64 = 0x4E47_0005; // grab offset in graph space, 1/16
const K_GY: u64 = 0x4E47_0006;
const K_CONN: u64 = 0x4E47_0007; // (node << 8 | output) while connecting
const K_PANNING: u64 = 0x4E47_0008;
const K_LX: u64 = 0x4E47_0009; // last pointer for panning
const K_LY: u64 = 0x4E47_000A;
const K_WIRE: u64 = 0x4E47_000B; // selected wire index
const K_DOWN: u64 = 0x4E47_000C; // pressed on empty space
const K_MAP: u64 = 0x4E47_000D; // dragging inside the minimap
const Z_MIN: f32 = 0.25;
const Z_MAX: f32 = 2.5;
const MAP_W: f32 = 168.0;

impl<M: 'static> NodeGraph<M> {
    /// A graph of `nodes` and `wires`. `id` keeps pan, zoom and drag state.
    pub fn new(
        ui: &Ui,
        id: u64,
        nodes: impl Into<Vec<GraphNode>>,
        wires: impl Into<Vec<Wire>>,
    ) -> Self {
        Self {
            ui: ui.clone(),
            id,
            nodes: nodes.into(),
            wires: wires.into(),
            selected: None,
            height: Length::Fill(1),
            minimap: false,
            on_action: None,
        }
    }
    /// Show a minimap in the bottom-right corner: every node as a small tile, the visible
    /// area as an outline. Click or drag inside it to move the view there.
    pub fn minimap(mut self, on: bool) -> Self {
        self.minimap = on;
        self
    }
    /// The selected node (drawn with an accent outline).
    pub fn selected(mut self, id: Option<u64>) -> Self {
        self.selected = id;
        self
    }
    /// Receive edits as messages.
    pub fn on_action(mut self, f: impl Fn(GraphAction) -> M + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
    /// Height (default: fill).
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn slot(&self, k: u64) -> Option<i64> {
        self.ui.slot(self.id ^ k)
    }
    fn set(&self, k: u64, v: Option<i64>) {
        self.ui.set_slot(self.id ^ k, v);
    }
    fn zoom(&self) -> f32 {
        self.slot(K_ZOOM).map_or(1.0, |z| z as f32 / 1000.0)
    }
    fn pan(&self) -> Point {
        Point::new(
            self.slot(K_PAN).map_or(32.0, |v| v as f32 / 16.0),
            self.slot(K_PAN_Y).map_or(32.0, |v| v as f32 / 16.0),
        )
    }
    fn set_view(&self, pan: Point, zoom: f32) {
        self.set(K_PAN, Some((pan.x * 16.0) as i64));
        self.set(K_PAN_Y, Some((pan.y * 16.0) as i64));
        self.set(K_ZOOM, Some((zoom * 1000.0) as i64));
    }
    fn to_screen(&self, b: &Rectangle, g: Point) -> Point {
        let (pan, z) = (self.pan(), self.zoom());
        Point::new(b.origin.x + pan.x + g.x * z, b.origin.y + pan.y + g.y * z)
    }
    fn to_graph(&self, b: &Rectangle, s: Point) -> Point {
        let (pan, z) = (self.pan(), self.zoom());
        Point::new(
            (s.x - b.origin.x - pan.x) / z,
            (s.y - b.origin.y - pan.y) / z,
        )
    }
    /// Graph-space bounds of every node, or `None` with no nodes.
    fn content(&self) -> Option<Rectangle> {
        let mut it = self.nodes.iter();
        let first = it.next()?;
        let mut lo = first.pos;
        let mut hi = Point::new(first.pos.x + first.width, first.pos.y + Self::node_h(first));
        for n in it {
            lo = Point::new(lo.x.min(n.pos.x), lo.y.min(n.pos.y));
            hi = Point::new(
                hi.x.max(n.pos.x + n.width),
                hi.y.max(n.pos.y + Self::node_h(n)),
            );
        }
        Some(Rectangle::new(lo, Size::new(hi.x - lo.x, hi.y - lo.y)))
    }
    /// Where the minimap panel sits inside `b`.
    fn minimap_rect(&self, b: &Rectangle) -> Option<Rectangle> {
        if !self.minimap || self.nodes.is_empty() || b.size.width < 240.0 || b.size.height < 160.0 {
            return None;
        }
        let w = MAP_W.min(b.size.width * 0.4);
        let h = w * 0.66;
        Some(Rectangle::new(
            Point::new(
                b.origin.x + b.size.width - w - 12.0,
                b.origin.y + b.size.height - h - 12.0,
            ),
            Size::new(w, h),
        ))
    }
    /// Scale and the minimap-space position of graph point (0, 0): the minimap shows the
    /// nodes and the visible area together, so the outline never leaves the panel.
    fn minimap_xf(&self, b: &Rectangle, m: &Rectangle) -> (f32, Point) {
        let (lo, hi) = {
            let a = self.to_graph(b, b.origin);
            let z = self.to_graph(
                b,
                Point::new(b.origin.x + b.size.width, b.origin.y + b.size.height),
            );
            let c = self
                .content()
                .unwrap_or(Rectangle::new(a, Size::new(1.0, 1.0)));
            (
                Point::new(a.x.min(c.origin.x), a.y.min(c.origin.y)),
                Point::new(
                    z.x.max(c.origin.x + c.size.width),
                    z.y.max(c.origin.y + c.size.height),
                ),
            )
        };
        let inner = Rectangle::new(
            Point::new(m.origin.x + 7.0, m.origin.y + 7.0),
            Size::new(m.size.width - 14.0, m.size.height - 14.0),
        );
        let (ww, wh) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        let k = (inner.size.width / ww).min(inner.size.height / wh);
        let origin = Point::new(
            inner.origin.x + (inner.size.width - ww * k) / 2.0 - lo.x * k,
            inner.origin.y + (inner.size.height - wh * k) / 2.0 - lo.y * k,
        );
        (k, origin)
    }
    /// Move the view so graph point under minimap point `p` is centred.
    fn recenter_from_minimap(&self, b: &Rectangle, m: &Rectangle, p: Point) {
        let (k, o) = self.minimap_xf(b, m);
        let g = Point::new((p.x - o.x) / k, (p.y - o.y) / k);
        let z = self.zoom();
        self.set_view(
            Point::new(b.size.width / 2.0 - g.x * z, b.size.height / 2.0 - g.y * z),
            z,
        );
    }
    /// Zoom by `factor` keeping the graph point under screen point `at` fixed.
    fn zoom_about(&self, b: &Rectangle, at: Point, factor: f32) -> bool {
        let z0 = self.zoom();
        let z1 = (z0 * factor).clamp(Z_MIN, Z_MAX);
        if (z1 - z0).abs() < 1e-4 {
            return false;
        }
        let g = self.to_graph(b, at);
        self.set_view(
            Point::new(at.x - b.origin.x - g.x * z1, at.y - b.origin.y - g.y * z1),
            z1,
        );
        true
    }
    /// Zoom and pan so every node is visible.
    fn fit(&self, b: &Rectangle) -> bool {
        let Some(c) = self.content() else {
            return false;
        };
        let margin = 48.0;
        let z = ((b.size.width - 2.0 * margin) / c.size.width.max(1.0))
            .min((b.size.height - 2.0 * margin) / c.size.height.max(1.0))
            .clamp(Z_MIN, 1.5);
        self.set_view(
            Point::new(
                (b.size.width - c.size.width * z) / 2.0 - c.origin.x * z,
                (b.size.height - c.size.height * z) / 2.0 - c.origin.y * z,
            ),
            z,
        );
        true
    }
    fn node(&self, id: u64) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
    fn node_h(n: &GraphNode) -> f32 {
        HEADER + n.inputs.len().max(n.outputs.len()).max(1) as f32 * ROW + PAD * 0.6
    }
    fn node_rect(&self, b: &Rectangle, n: &GraphNode) -> Rectangle {
        let z = self.zoom();
        Rectangle::new(
            self.to_screen(b, n.pos),
            Size::new(n.width * z, Self::node_h(n) * z),
        )
    }
    fn port_pos(&self, b: &Rectangle, n: &GraphNode, output: bool, i: usize) -> Point {
        let r = self.node_rect(b, n);
        let z = self.zoom();
        Point::new(
            if output {
                r.origin.x + r.size.width
            } else {
                r.origin.x
            },
            r.origin.y + (HEADER + i as f32 * ROW + ROW / 2.0 + 2.0) * z,
        )
    }
    /// Port under `p`: `(node, index, is_output)`.
    fn port_at(&self, b: &Rectangle, p: Point) -> Option<(u64, usize, bool)> {
        let reach = (PORT_R + 5.0) * self.zoom().max(0.8);
        for n in self.nodes.iter().rev() {
            for (output, count) in [(false, n.inputs.len()), (true, n.outputs.len())] {
                for i in 0..count {
                    let q = self.port_pos(b, n, output, i);
                    if (p.x - q.x).hypot(p.y - q.y) <= reach {
                        return Some((n.id, i, output));
                    }
                }
            }
        }
        None
    }
    fn node_at(&self, b: &Rectangle, p: Point) -> Option<u64> {
        self.nodes
            .iter()
            .rev()
            .find(|n| self.node_rect(b, n).contains(p))
            .map(|n| n.id)
    }
    /// Control points of the wire between two screen points.
    fn curve(&self, a: Point, z: Point) -> [Point; 4] {
        let k = ((z.x - a.x).abs() * 0.5).max(48.0 * self.zoom());
        [a, Point::new(a.x + k, a.y), Point::new(z.x - k, z.y), z]
    }
    fn sample(c: &[Point; 4], n: usize) -> Vec<Point> {
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                let u = 1.0 - t;
                let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                Point::new(
                    b0 * c[0].x + b1 * c[1].x + b2 * c[2].x + b3 * c[3].x,
                    b0 * c[0].y + b1 * c[1].y + b2 * c[2].y + b3 * c[3].y,
                )
            })
            .collect()
    }
    fn wire_ends(&self, b: &Rectangle, w: &Wire) -> Option<(Point, Point)> {
        let (a, z) = (self.node(w.from.0)?, self.node(w.to.0)?);
        Some((
            self.port_pos(b, a, true, w.from.1),
            self.port_pos(b, z, false, w.to.1),
        ))
    }
    fn wire_at(&self, b: &Rectangle, p: Point) -> Option<usize> {
        self.wires.iter().enumerate().rev().find_map(|(i, w)| {
            let (a, z) = self.wire_ends(b, w)?;
            let pts = Self::sample(&self.curve(a, z), 24);
            pts.windows(2)
                .any(|s| dist_to_segment(p, s[0], s[1]) <= 6.0)
                .then_some(i)
        })
    }
    fn emit(&self, out: &mut Vec<M>, a: GraphAction) {
        if let Some(f) = &self.on_action {
            out.push(f(a));
        }
    }
}

fn dist_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

impl<M: 'static> Widget<M> for NodeGraph<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(320.0, 240.0)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        match event {
            Event::MousePressed(MouseButton::Left)
                if self.minimap_rect(&b).is_some_and(|m| m.contains(cursor)) =>
            {
                let m = self.minimap_rect(&b).expect("checked");
                self.ui.set_focus(Some(self.id));
                self.set(K_MAP, Some(1));
                self.recenter_from_minimap(&b, &m, cursor);
                true
            }
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
                self.set(K_WIRE, None);
                if let Some((node, port, true)) = self.port_at(&b, cursor) {
                    self.set(K_CONN, Some((node << 8 | port as u64) as i64));
                } else if let Some(id) = self.node_at(&b, cursor) {
                    let n = self.node(id).cloned();
                    if let Some(n) = n {
                        let g = self.to_graph(&b, cursor);
                        self.set(K_DRAG, Some(id as i64));
                        self.set(K_GX, Some(((g.x - n.pos.x) * 16.0) as i64));
                        self.set(K_GY, Some(((g.y - n.pos.y) * 16.0) as i64));
                    }
                    self.emit(out, GraphAction::Select(Some(id)));
                } else if let Some(i) = self.wire_at(&b, cursor) {
                    self.set(K_WIRE, Some(i as i64));
                    self.emit(out, GraphAction::Select(None));
                } else {
                    self.set(K_PANNING, Some(1));
                    self.set(K_LX, Some((cursor.x * 16.0) as i64));
                    self.set(K_LY, Some((cursor.y * 16.0) as i64));
                    self.set(K_DOWN, Some(1));
                    if self.selected.is_some() {
                        self.emit(out, GraphAction::Select(None));
                    }
                }
                true
            }
            Event::CursorMoved(p) => {
                if self.slot(K_MAP).is_some() {
                    if let Some(m) = self.minimap_rect(&b) {
                        self.recenter_from_minimap(&b, &m, *p);
                    }
                    return true;
                }
                if let Some(id) = self.slot(K_DRAG).map(|v| v as u64) {
                    let g = self.to_graph(&b, *p);
                    let (gx, gy) = (
                        self.slot(K_GX).unwrap_or(0) as f32 / 16.0,
                        self.slot(K_GY).unwrap_or(0) as f32 / 16.0,
                    );
                    self.emit(
                        out,
                        GraphAction::Move {
                            id,
                            pos: Point::new((g.x - gx).round(), (g.y - gy).round()),
                        },
                    );
                    return true;
                }
                if self.slot(K_PANNING).is_some() {
                    let (lx, ly) = (
                        self.slot(K_LX).unwrap_or(0) as f32 / 16.0,
                        self.slot(K_LY).unwrap_or(0) as f32 / 16.0,
                    );
                    let pan = self.pan();
                    self.set_view(Point::new(pan.x + p.x - lx, pan.y + p.y - ly), self.zoom());
                    self.set(K_LX, Some((p.x * 16.0) as i64));
                    self.set(K_LY, Some((p.y * 16.0) as i64));
                    return true;
                }
                self.slot(K_CONN).is_some()
            }
            Event::MouseReleased(MouseButton::Left) => {
                let conn = self.slot(K_CONN);
                let busy = conn.is_some()
                    || self.slot(K_DRAG).is_some()
                    || self.slot(K_PANNING).is_some()
                    || self.slot(K_MAP).is_some();
                if let Some(c) = conn {
                    let from = ((c as u64) >> 8, (c as u64 & 0xFF) as usize);
                    if let Some((node, port, false)) = self.port_at(&b, cursor) {
                        if node != from.0 {
                            self.emit(
                                out,
                                GraphAction::Connect(Wire {
                                    from,
                                    to: (node, port),
                                }),
                            );
                        }
                    }
                }
                for k in [
                    K_CONN, K_DRAG, K_PANNING, K_DOWN, K_GX, K_GY, K_LX, K_LY, K_MAP,
                ] {
                    self.set(k, None);
                }
                busy
            }
            Event::Scrolled { dy, .. } if b.contains(cursor) => {
                self.zoom_about(&b, cursor, 1.0 + dy * 0.0015)
            }
            Event::KeyPressed(key, _) if self.ui.is_focused(Some(self.id)) => {
                let delete =
                    matches!(key, Key::Backspace) || matches!(key, Key::Named(n) if n == "Delete");
                if delete {
                    if let Some(i) = self.slot(K_WIRE) {
                        if let Some(w) = self.wires.get(i as usize).copied() {
                            self.set(K_WIRE, None);
                            self.emit(out, GraphAction::Disconnect(w));
                            return true;
                        }
                    }
                    if let Some(id) = self.selected {
                        self.emit(out, GraphAction::Delete(id));
                        return true;
                    }
                    false
                } else if matches!(key, Key::Character(c) if c == "0") {
                    self.set(K_PAN, None);
                    self.set(K_PAN_Y, None);
                    self.set(K_ZOOM, None);
                    true
                } else if matches!(key, Key::Character(c) if c == "f" || c == "F") {
                    self.fit(&b)
                } else if matches!(key, Key::Character(c) if c == "+" || c == "=") {
                    let c = Point::new(
                        b.origin.x + b.size.width / 2.0,
                        b.origin.y + b.size.height / 2.0,
                    );
                    self.zoom_about(&b, c, 1.2)
                } else if matches!(key, Key::Character(c) if c == "-" || c == "_") {
                    let c = Point::new(
                        b.origin.x + b.size.width / 2.0,
                        b.origin.y + b.size.height / 2.0,
                    );
                    self.zoom_about(&b, c, 1.0 / 1.2)
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        self.ui.register(self.id);
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let lab = t.typography.label;
        let m = self.ui.metrics();
        let b = layout.bounds;
        let z = self.zoom();
        let cur = self.ui.cursor();
        self.ui.describe(
            Role::Label,
            &format!(
                "Node graph, {} nodes, {} wires",
                self.nodes.len(),
                self.wires.len()
            ),
            &b,
            |_| {},
        );
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: p.surface,
            radius: t.radii.large,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: None,
        });
        out.push(DrawCommand::PushClip(b));

        // Dot grid that pans and zooms with the content.
        let step = 24.0 * z;
        if step >= 8.0 {
            let pan = self.pan();
            let (ox, oy) = (
                b.origin.x + pan.x.rem_euclid(step),
                b.origin.y + pan.y.rem_euclid(step),
            );
            let (cols, rows) = (
                (b.size.width / step) as i32 + 1,
                (b.size.height / step) as i32 + 1,
            );
            for r in 0..=rows {
                for c in 0..=cols {
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(ox + c as f32 * step - 1.0, oy + r as f32 * step - 1.0),
                            Size::new(2.0, 2.0),
                        ),
                        fill: p.border_strong.with_alpha(0.28),
                        radius: 1.0,
                        border: None,
                        shadow: None,
                    });
                }
            }
        }

        // Wires under nodes.
        let sel_wire = self.slot(K_WIRE).map(|v| v as usize);
        for (i, w) in self.wires.iter().enumerate() {
            let Some((a, e)) = self.wire_ends(&b, w) else {
                continue;
            };
            let tone = self.node(w.from.0).map_or(Tone::Neutral, |n| n.tone);
            let selected = sel_wire == Some(i);
            let hot = !selected && self.wire_at(&b, cur) == Some(i) && self.slot(K_CONN).is_none();
            let color = if selected {
                p.accent
            } else if hot {
                p.border_strong
            } else {
                mix(p.border_strong, tone.color(&t), 0.55)
            };
            let pts = Self::sample(&self.curve(a, e), 36);
            for s in pts.windows(2) {
                out.push(DrawCommand::Line {
                    from: s[0],
                    to: s[1],
                    width: if selected { 3.0 } else { 2.0 } * z.max(0.7),
                    color,
                });
            }
        }
        // Wire being dragged.
        let from_port = self
            .slot(K_CONN)
            .map(|c| ((c as u64) >> 8, (c as u64 & 0xFF) as usize));
        let snap = self
            .port_at(&b, cur)
            .filter(|(n, _, o)| !*o && Some(*n) != from_port.map(|f| f.0));
        if let Some((nid, port)) = from_port {
            if let Some(n) = self.node(nid) {
                let a = self.port_pos(&b, n, true, port);
                let end = snap
                    .and_then(|(id, i, _)| self.node(id).map(|nn| self.port_pos(&b, nn, false, i)))
                    .unwrap_or(cur);
                for s in Self::sample(&self.curve(a, end), 36).windows(2) {
                    out.push(DrawCommand::Line {
                        from: s[0],
                        to: s[1],
                        width: 2.0 * z.max(0.7),
                        color: p.accent,
                    });
                }
            }
        }

        // Nodes.
        for n in &self.nodes {
            let r = self.node_rect(&b, n);
            if r.origin.x > b.origin.x + b.size.width
                || r.origin.y > b.origin.y + b.size.height
                || r.origin.x + r.size.width < b.origin.x
                || r.origin.y + r.size.height < b.origin.y
            {
                continue;
            }
            let selected = self.selected == Some(n.id);
            let dragging = self.slot(K_DRAG) == Some(n.id as i64);
            let radius = t.radii.large * z;
            out.push(DrawCommand::Quad {
                bounds: r,
                fill: p.surface_raised,
                radius,
                border: Some(Border {
                    color: if selected { p.accent } else { p.border_control },
                    width: if selected { 2.0 } else { 1.0 },
                }),
                shadow: Some(if dragging {
                    t.elevation.high
                } else {
                    t.elevation.low
                }),
            });
            // Header band: the rounded quad clipped to the header strip.
            let hr = Rectangle::new(r.origin, Size::new(r.size.width, HEADER * z));
            out.push(DrawCommand::PushClip(hr));
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(r.origin.x + 1.0, r.origin.y + 1.0),
                    Size::new(r.size.width - 2.0, HEADER * z + radius),
                ),
                fill: mix(p.surface_raised, n.tone.color(&t), 0.09),
                radius: (radius - 1.0).max(0.0),
                border: None,
                shadow: None,
            });
            out.push(DrawCommand::PopClip);
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(r.origin.x + 1.0, r.origin.y + HEADER * z - 0.5),
                    Size::new(r.size.width - 2.0, 1.0),
                ),
                fill: p.border,
                radius: 0.0,
                border: None,
                shadow: None,
            });
            let ts = lab.size * z;
            out.push(DrawCommand::PushClip(r));
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(
                        r.origin.x + 12.0 * z,
                        r.origin.y + (HEADER * z - 8.0 * z) / 2.0,
                    ),
                    Size::new(8.0 * z, 8.0 * z),
                ),
                fill: n.tone.color(&t),
                radius: 4.0 * z,
                border: None,
                shadow: None,
            });
            out.push(DrawCommand::Text {
                origin: Point::new(
                    r.origin.x + 28.0 * z,
                    r.origin.y + (HEADER * z - lab.line_height * z) / 2.0,
                ),
                content: n.title.clone(),
                size: ts,
                color: p.text,
                max_width: None,
                weight: 600,
                family: FontFamily::Sans,
                line_height: Some(lab.line_height * z),
            });
            let cs = cap.size * z;
            for (output, names) in [(false, &n.inputs), (true, &n.outputs)] {
                for (i, name) in names.iter().enumerate() {
                    let pp = self.port_pos(&b, n, output, i);
                    let w = m.width_styled(name, cs, 400, FontFamily::Sans);
                    out.push(DrawCommand::Text {
                        origin: Point::new(
                            if output {
                                pp.x - 14.0 * z - w
                            } else {
                                pp.x + 14.0 * z
                            },
                            pp.y - cap.line_height * z / 2.0,
                        ),
                        content: name.clone(),
                        size: cs,
                        color: p.text_secondary,
                        max_width: None,
                        weight: 400,
                        family: FontFamily::Sans,
                        line_height: Some(cap.line_height * z),
                    });
                }
            }
            out.push(DrawCommand::PopClip);
            // Ports sit on the edge, drawn after the clip so they overhang.
            for (output, names) in [(false, &n.inputs), (true, &n.outputs)] {
                for i in 0..names.len() {
                    let pp = self.port_pos(&b, n, output, i);
                    let connected = self.wires.iter().any(|w| {
                        if output {
                            w.from == (n.id, i)
                        } else {
                            w.to == (n.id, i)
                        }
                    });
                    let hot = snap == Some((n.id, i, false)) && !output
                        || (pp.x - cur.x).hypot(pp.y - cur.y) < (PORT_R + 4.0) * z;
                    let rad = (PORT_R + if hot { 1.5 } else { 0.0 }) * z.max(0.8);
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(pp.x - rad, pp.y - rad),
                            Size::new(2.0 * rad, 2.0 * rad),
                        ),
                        fill: if connected || hot {
                            p.accent
                        } else {
                            p.surface_raised
                        },
                        radius: rad,
                        border: Some(Border {
                            color: if connected || hot {
                                p.accent
                            } else {
                                p.border_strong
                            },
                            width: 1.5,
                        }),
                        shadow: None,
                    });
                }
            }
        }
        if let Some(m) = self.minimap_rect(&b) {
            out.push(DrawCommand::Quad {
                bounds: m,
                fill: p.surface_raised.with_alpha(0.94),
                radius: t.radii.medium,
                border: Some(Border {
                    color: p.border_strong,
                    width: 1.0,
                }),
                shadow: None,
            });
            let (k, o) = self.minimap_xf(&b, &m);
            out.push(DrawCommand::PushClip(m));
            for n in &self.nodes {
                let tile = Rectangle::new(
                    Point::new(o.x + n.pos.x * k, o.y + n.pos.y * k),
                    Size::new((n.width * k).max(3.0), (Self::node_h(n) * k).max(2.0)),
                );
                let picked = self.selected == Some(n.id);
                out.push(DrawCommand::Quad {
                    bounds: tile,
                    fill: if picked {
                        p.accent
                    } else if n.tone == Tone::Neutral {
                        p.border_strong
                    } else {
                        n.tone.color(&t)
                    },
                    radius: 1.5,
                    border: None,
                    shadow: None,
                });
            }
            let a = self.to_graph(&b, b.origin);
            let vp = Rectangle::new(
                Point::new(o.x + a.x * k, o.y + a.y * k),
                Size::new(b.size.width / z * k, b.size.height / z * k),
            );
            out.push(DrawCommand::Quad {
                bounds: vp,
                fill: p.accent.with_alpha(0.10),
                radius: 2.0,
                border: Some(Border {
                    color: p.accent,
                    width: 1.5,
                }),
                shadow: None,
            });
            out.push(DrawCommand::PopClip);
        }
        out.push(DrawCommand::PopClip);
    }
}
