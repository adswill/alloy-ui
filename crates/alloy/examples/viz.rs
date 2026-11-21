//! Visual widgets: pie and donut, candlesticks, live charts, kanban, calendar, node graph,
//! canvas, 3D viewport and media player. Run one tab headless with
//! `cargo run -p alloy-ui --example viz -- --headless out.png --click Board`.
use alloy_ui::prelude::*;
use alloy_ui::widgets::{Date, Tone};
use alloy_ui::{
    rgb, CalAction, CalView, Candle, Entry, GraphAction, GraphNode, Lane, Mesh, Orbit, Path,
    Playback, Ring, TestCard, Wire,
};

const TODAY: Date = Date {
    year: 2026,
    month: 11,
    day: 21,
};
const TABS: [&str; 6] = [
    "Charts",
    "Board",
    "Calendar",
    "Graph",
    "Canvas & 3D",
    "Media",
];

/// Cheap deterministic noise in 0..1.
fn noise(i: u32) -> f32 {
    let x = i
        .wrapping_mul(2_654_435_761)
        .rotate_left(13)
        .wrapping_mul(0x85EB_CA6B);
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn ring(cap: usize, seed: u32, base: f32, amp: f32) -> Ring {
    let mut r = Ring::new(cap);
    for i in 0..cap as u32 {
        let t = i as f32 / 9.0;
        r.push(base + amp * (t.sin() * 0.6 + (noise(seed + i) - 0.5)));
    }
    r
}

fn market() -> Vec<Candle> {
    let mut price = 118.0;
    (0..32u32)
        .map(|i| {
            let open = price;
            let close = (open + (noise(i * 7 + 3) - 0.46) * 9.0).max(60.0);
            let high = open.max(close) + noise(i * 3 + 1) * 4.0;
            let low = open.min(close) - noise(i * 5 + 2) * 4.0;
            price = close;
            let label = format!("{:02}", i % 28 + 1);
            Candle::new(label, open, high, low, close).volume(800.0 + noise(i + 90) * 2600.0)
        })
        .collect()
}

fn lanes() -> Vec<Lane> {
    vec![
        lane(
            "Backlog",
            [
                card(1, "Plan the Q1 roadmap").detail("Product"),
                card(2, "Audit icon set")
                    .detail("Design")
                    .tag("Needs owner", Tone::Warning),
                card(3, "Retire legacy export"),
            ],
        ),
        lane(
            "In progress",
            [
                card(4, "Rebuild onboarding")
                    .detail("Maya · 3 days")
                    .tag("On track", Tone::Success),
                card(5, "Fix sync conflicts")
                    .detail("Jon · 1 day")
                    .tag("Blocked", Tone::Danger),
            ],
        ),
        lane(
            "Review",
            [card(6, "Dark theme contrast pass")
                .detail("Priya")
                .tag("Ready", Tone::Accent)],
        ),
        lane(
            "Done",
            [
                card(7, "Ship offline mode"),
                card(8, "Migrate billing webhooks").detail("Jon"),
            ],
        ),
    ]
}

fn entries() -> Vec<Entry> {
    let d = |n: i64| Date::from_days(TODAY.to_days() + n);
    let m = |h: u32, min: u32| h * 60 + min;
    vec![
        entry(1, "Standup", d(0), m(9, 30), m(10, 0)).tone(Tone::Neutral),
        entry(2, "Design review", d(0), m(11, 0), m(12, 30)).tone(Tone::Accent),
        entry(3, "Lunch with Priya", d(0), m(12, 30), m(13, 30)).tone(Tone::Success),
        entry(4, "Roadmap sync", d(0), m(12, 45), m(14, 0)).tone(Tone::Warning),
        entry(5, "Interview: staff eng", d(1), m(10, 0), m(11, 0)).tone(Tone::Accent),
        entry(6, "Release train", d(2), m(15, 0), m(16, 30)).tone(Tone::Danger),
        entry(7, "1:1 with Maya", d(-1), m(14, 0), m(14, 30)).tone(Tone::Neutral),
        entry(8, "Budget workshop", d(-2), m(9, 0), m(11, 30)).tone(Tone::Warning),
        entry(9, "Team offsite", d(8), m(9, 0), m(17, 0)).tone(Tone::Success),
        entry(10, "Demo day", d(5), m(16, 0), m(17, 0)).tone(Tone::Accent),
        entry(11, "Retro", d(-6), m(15, 0), m(16, 0)).tone(Tone::Neutral),
        entry(12, "Dentist", d(3), m(8, 0), m(9, 0)).tone(Tone::Neutral),
        entry(13, "Security training", d(3), m(13, 0), m(14, 0)).tone(Tone::Warning),
        entry(14, "Pair on parser", d(3), m(14, 0), m(15, 0)).tone(Tone::Accent),
        entry(15, "Ship notes", d(3), m(16, 0), m(16, 30)).tone(Tone::Success),
    ]
}

fn graph() -> (Vec<GraphNode>, Vec<Wire>) {
    let nodes = vec![
        node(1, "Load image", 20.0, 40.0)
            .outputs(["Image", "Alpha"])
            .tone(Tone::Accent),
        node(2, "Blur", 270.0, 20.0)
            .inputs(["Image", "Radius"])
            .outputs(["Result"])
            .tone(Tone::Neutral),
        node(3, "Threshold", 270.0, 150.0)
            .inputs(["Image", "Level"])
            .outputs(["Mask"])
            .tone(Tone::Neutral),
        node(4, "Composite", 520.0, 80.0)
            .inputs(["Base", "Overlay", "Mask"])
            .outputs(["Image"])
            .tone(Tone::Warning),
        node(5, "Save PNG", 770.0, 100.0)
            .inputs(["Image"])
            .tone(Tone::Success)
            .width(150.0),
    ];
    let wires = vec![
        Wire::new((1, 0), (2, 0)),
        Wire::new((1, 0), (3, 0)),
        Wire::new((2, 0), (4, 0)),
        Wire::new((1, 0), (4, 1)),
        Wire::new((3, 0), (4, 2)),
        Wire::new((4, 0), (5, 0)),
    ];
    (nodes, wires)
}

state! {
    struct Viz {
        tab: usize,
        cpu: Ring = ring(60, 1, 42.0, 30.0),
        gpu: Ring = ring(60, 77, 28.0, 20.0),
        market: Vec<Candle> = market(),
        picked: Option<usize>,
        lanes: Vec<Lane> = lanes(),
        opened: Option<u64>,
        day: Date = TODAY,
        cal_view: CalView = CalView::Week,
        entries: Vec<Entry> = entries(),
        nodes: Vec<GraphNode> = graph().0,
        wires: Vec<Wire> = graph().1,
        selected: Option<u64>,
        doodle: Vec<(f32, f32)>,
        play: Playback,
    }
}

impl Viz {
    fn calendar(&mut self, a: CalAction) {
        let step = |v: CalView| match v {
            CalView::Month => 0,
            CalView::Week => 7,
            CalView::Day => 1,
        };
        match a {
            CalAction::Prev | CalAction::Next => {
                let dir = if a == CalAction::Next { 1 } else { -1 };
                self.day = match self.cal_view {
                    CalView::Month => self.day.add_months(dir),
                    v => Date::from_days(self.day.to_days() + dir as i64 * step(v)),
                };
            }
            CalAction::Today => self.day = TODAY,
            CalAction::View(v) => self.cal_view = v,
            CalAction::Day(d) => {
                self.day = d;
                self.cal_view = CalView::Day;
            }
            CalAction::Create { date, minute } => {
                let id = self.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
                self.entries
                    .push(entry(id, "New event", date, minute, minute + 60));
            }
            CalAction::Move { id, date, start } => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
                    let dur = e.end - e.start;
                    (e.date, e.start, e.end) = (date, start, start + dur);
                }
            }
            CalAction::Resize { id, end } => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
                    e.end = end.max(e.start + 15);
                }
            }
            CalAction::Open(_) => {}
        }
    }

    fn graph(&mut self, a: GraphAction) {
        match a {
            GraphAction::Move { id, pos } => {
                if let Some(n) = self.nodes.iter_mut().find(|n| n.id == id) {
                    n.pos = pos;
                }
            }
            GraphAction::Select(id) => self.selected = id,
            GraphAction::Connect(w) => {
                self.wires.retain(|x| x.to != w.to);
                self.wires.push(w);
            }
            GraphAction::Disconnect(w) => self.wires.retain(|x| *x != w),
            GraphAction::Delete(id) => {
                self.nodes.retain(|n| n.id != id);
                self.wires.retain(|w| w.from.0 != id && w.to.0 != id);
                self.selected = None;
            }
        }
    }
}

fn charts(s: &Viz) -> impl View<Act<Viz>> {
    let browsers = [
        ("Chrome", 64.0),
        ("Safari", 19.0),
        ("Edge", 8.0),
        ("Firefox", 6.0),
        ("Other", 3.0),
    ];
    col![
        row![
            col![
                caption("Spending by category"),
                pie([
                    ("Rent", 1200.0),
                    ("Food", 540.0),
                    ("Travel", 310.0),
                    ("Health", 180.0),
                    ("Fun", 150.0)
                ])
                .donut()
                .title("This month")
                .unit(" €")
                .selected(s.picked)
                .act(|s: &mut Viz, i| s.picked = Some(i))
                .height(190.0),
            ]
            .gap(10.0)
            .card()
            .fill_w(),
            col![
                caption("Browser share"),
                pie(browsers).unit("%").height(190.0),
            ]
            .gap(10.0)
            .card()
            .fill_w(),
        ]
        .gap(16.0)
        .fill_w(),
        col![
            caption("Acme Corp, last 32 sessions"),
            candles(s.market.clone()).unit("$").height(250.0),
        ]
        .gap(10.0)
        .card()
        .fill_w(),
        col![
            caption("Live load"),
            stream([("CPU", &s.cpu), ("GPU", &s.gpu)])
                .unit("%")
                .window("last 60 s")
                .height(170.0),
        ]
        .gap(10.0)
        .card()
        .fill_w(),
    ]
    .gap(16.0)
}

fn board(s: &Viz) -> impl View<Act<Viz>> {
    col![
        row![
            heading("Product board").fill_w(),
            caption(match s.opened {
                Some(id) => format!("Opened card {id}"),
                None =>
                    "Drag cards between columns. Focus the board and use Alt+arrows to move one."
                        .into(),
            })
            .dim(),
        ]
        .align_center()
        .fill_w(),
        kanban(&s.lanes)
            .act(|s: &mut Viz, mv| mv.apply(&mut s.lanes))
            .open(|s: &mut Viz, id| s.opened = Some(id)),
    ]
    .gap(14.0)
    .fill()
}

fn calendar_tab(s: &Viz) -> impl View<Act<Viz>> {
    calendar(s.day, s.cal_view, &s.entries)
        .today(TODAY)
        .now(13 * 60 + 5)
        .hours(7, 19)
        .act(|s: &mut Viz, a| s.calendar(a))
}

fn graph_tab(s: &Viz) -> impl View<Act<Viz>> {
    col![
        caption("Drag headers to move, drag from an output port to an input to connect, click a wire and press Delete. Scroll to zoom, drag the background to pan.")
            .dim(),
        node_graph(&s.nodes, &s.wires)
            .selected(s.selected)
            .act(|s: &mut Viz, a| s.graph(a)),
    ]
    .gap(10.0)
    .fill()
}

fn spiro(f: &mut alloy_ui::Frame) {
    let p = f.palette();
    let c = pt(f.size().width / 2.0, f.size().height / 2.0);
    let mut path = Path::new();
    for i in 0..=720 {
        let t = i as f32 / 720.0 * std::f32::consts::TAU * 7.0;
        let r = 58.0;
        let (k, l) = (0.62, 0.78);
        let x = r * ((1.0 - k) * t.cos() + l * k * (t * (1.0 - k) / k).cos());
        let y = r * ((1.0 - k) * t.sin() - l * k * (t * (1.0 - k) / k).sin());
        path = if i == 0 {
            path.move_to(c.x + x, c.y + y)
        } else {
            path.line_to(c.x + x, c.y + y)
        };
    }
    f.stroke(&path, 1.4, p.accent);
    // A closed curve filled with an alpha tint.
    let blob = Path::new()
        .move_to(24.0, 150.0)
        .cubic_to(24.0, 100.0, 90.0, 110.0, 110.0, 150.0)
        .cubic_to(130.0, 190.0, 40.0, 200.0, 24.0, 150.0)
        .close();
    f.fill(&blob, p.accent.with_alpha(0.16));
    f.stroke(&blob, 1.5, p.accent);
    f.text(
        pt(24.0, 14.0),
        "Paths: fill, stroke, curves",
        12.0,
        p.text_secondary,
    );
}

fn canvas_tab(s: &Viz) -> impl View<Act<Viz>> {
    let dots = s.doodle.clone();
    let mesh = |m: Mesh, c: u32, x: f32, y: f32| object(m).color(rgb(c)).at(x, y, 0.0);
    col![
        row![
            col![caption("Canvas"), canvas(300.0, 230.0, spiro),]
                .gap(10.0)
                .card(),
            col![
                caption("Click and drag to draw"),
                canvas::<Viz>(300.0, 230.0, move |f| {
                    let p = f.palette();
                    for w in dots.windows(2) {
                        if w[0].0 >= 0.0 && w[1].0 >= 0.0 {
                            f.line(pt(w[0].0, w[0].1), pt(w[1].0, w[1].1), 2.5, p.accent);
                        }
                    }
                    if dots.is_empty() {
                        f.text(pt(86.0, 108.0), "Draw here", 14.0, p.text_secondary);
                    }
                })
                .act(|s: &mut Viz, ptr| match ptr {
                    alloy_ui::Pointer::Down(p) => s.doodle.push((p.x, p.y)),
                    alloy_ui::Pointer::Move(p) if s.doodle.last().is_some_and(|l| l.0 >= 0.0) => {
                        s.doodle.push((p.x, p.y))
                    }
                    alloy_ui::Pointer::Up(_) => s.doodle.push((-1.0, -1.0)),
                    _ => {}
                }),
            ]
            .gap(10.0)
            .card(),
        ]
        .gap(16.0),
        col![
            caption("3D viewport: drag to orbit, scroll to zoom"),
            viewport3d([
                mesh(Mesh::torus(1.5, 0.52, 40), 0x3558A8, 0.0, 0.62),
                mesh(Mesh::sphere(0.9, 32), 0xC2603B, 0.0, 0.62),
                mesh(Mesh::cube(1.1), 0x4F8A6B, 3.1, 0.55),
                mesh(Mesh::pyramid(1.4, 1.5), 0x9A6BB0, -3.1, 0.75),
            ])
            .orbit(Orbit {
                yaw: 0.5,
                pitch: 0.5,
                distance: 9.5,
            })
            .height(300.0),
        ]
        .gap(10.0)
        .card()
        .fill_w(),
    ]
    .gap(16.0)
}

fn media_tab(s: &Viz) -> impl View<Act<Viz>> {
    let video = TestCard::video(90);
    let audio = TestCard::audio(212);
    col![
        row![
            media(video, s.play)
                .title("Test card")
                .height(300.0)
                .act(|s: &mut Viz, a| {
                    s.play.apply(a, alloy_ui::ui().now(), 90_000);
                }),
            media(audio, Playback::new())
                .title("Level meter")
                .height(300.0),
        ]
        .gap(16.0),
        caption(
            "Alloy ships the player; a decoder crate implements MediaSource to feed it frames."
        )
        .dim(),
    ]
    .gap(14.0)
}

fn view(s: &Viz) -> impl View<Act<Viz>> {
    let page: Node<Act<Viz>> = match s.tab {
        0 => scroll(charts(s)).fill().into(),
        1 => board(s).into(),
        2 => calendar_tab(s).into(),
        3 => graph_tab(s).into(),
        4 => scroll(canvas_tab(s)).fill().into(),
        _ => media_tab(s).into(),
    };
    col![tab_bar(TABS, at!(s.tab)), page]
        .gap(14.0)
        .pad(20.0)
        .fill()
}

fn main() {
    sketch(Viz::default(), view)
        .tick(secs(1.0), |s| {
            let n = s.cpu.len() as u32 + 100;
            let t = noise(n);
            let last = s.cpu.last().unwrap_or(40.0);
            s.cpu.push((last + (t - 0.5) * 26.0).clamp(4.0, 96.0));
            let lg = s.gpu.last().unwrap_or(30.0);
            s.gpu
                .push((lg + (noise(n + 9) - 0.5) * 18.0).clamp(2.0, 90.0));
        })
        .run_sized("Visual widgets", 1100.0, 780.0)
}
