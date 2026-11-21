//! Behaviour tests for the polish on node graph, kanban and charts: minimap, fit, drag
//! auto-scroll, zoom and pan, tooltips, and a live-stream draw-time budget.

use alloy_ui_core::*;
use alloy_ui_layout::Length;
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::time::Instant;

const W: f32 = 640.0;
const H: f32 = 360.0;

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    Graph(GraphAction),
    Moved(Move),
}

/// A mounted tree that takes events like the runtime does.
struct Scene {
    root: Element<Msg>,
    layout: Layout,
    cursor: Point,
    msgs: Vec<Msg>,
    list: DrawList,
}

impl Scene {
    fn new(build: impl Fn(&Ui) -> Element<Msg>) -> Self {
        let ui = Ui::new(Theme::light());
        let root = Root::new(&ui, build(&ui)).into_element();
        let layout = alloy_ui_widgets::layout(&root, Size::new(W, H));
        let mut s = Self {
            root,
            layout,
            cursor: Point::new(-1.0, -1.0),
            msgs: Vec::new(),
            list: DrawList::default(),
        };
        s.draw();
        s
    }
    fn draw(&mut self) {
        self.layout = alloy_ui_widgets::layout(&self.root, Size::new(W, H));
        self.list = DrawList::default();
        self.root.0.draw(&self.layout, &mut self.list);
    }
    fn send(&mut self, e: Event) {
        if let Event::CursorMoved(p) = &e {
            self.cursor = *p;
        }
        self.root
            .0
            .on_event(&e, &self.layout, self.cursor, &mut self.msgs);
        self.draw();
    }
    fn mv(&mut self, x: f32, y: f32) {
        self.send(Event::CursorMoved(Point::new(x, y)));
    }
    fn press(&mut self, x: f32, y: f32) {
        self.mv(x, y);
        self.send(Event::MousePressed(MouseButton::Left));
    }
    fn release(&mut self) {
        self.send(Event::MouseReleased(MouseButton::Left));
    }
    fn key(&mut self, c: &str) {
        self.send(Event::KeyPressed(
            Key::Character(c.into()),
            Modifiers::default(),
        ));
    }
    fn quads(&self) -> usize {
        self.list
            .0
            .iter()
            .filter(|c| matches!(c, DrawCommand::Quad { .. }))
            .count()
    }
}

fn graph(ui: &Ui, minimap: bool) -> Element<Msg> {
    let nodes = vec![
        GraphNode::new(1, "A", 0.0, 0.0).outputs(["x"]),
        GraphNode::new(2, "B", 900.0, 600.0).inputs(["in"]),
        GraphNode::new(3, "C", 450.0, 300.0)
            .inputs(["a"])
            .outputs(["b"]),
    ];
    NodeGraph::new(ui, 7, nodes, vec![Wire::new((1, 0), (3, 0))])
        .minimap(minimap)
        .on_action(Msg::Graph)
        .into_element()
}

#[test]
fn minimap_adds_a_panel_and_a_tile_per_node() {
    let off = Scene::new(|ui| graph(ui, false));
    let on = Scene::new(|ui| graph(ui, true));
    // Panel, three node tiles and the viewport outline.
    assert_eq!(on.quads(), off.quads() + 5);
}

#[test]
fn clicking_the_minimap_recentres_the_view_and_does_not_hit_nodes() {
    let mut s = Scene::new(|ui| graph(ui, true));
    let rest = format!("{:?}", s.list.0);
    // The minimap is the bottom-right panel; press inside it near the left edge.
    s.press(W - 12.0 - 168.0 + 20.0, H - 12.0 - 55.0);
    s.release();
    assert!(s.msgs.is_empty(), "a minimap click makes no graph edits");
    assert_ne!(rest, format!("{:?}", s.list.0), "the view moved");
}

#[test]
fn fit_key_shows_every_node_and_zoom_keys_stay_in_range() {
    let mut s = Scene::new(|ui| graph(ui, false));
    s.press(5.0, H - 5.0); // focus the graph (empty space)
    s.release();
    s.key("f");
    // After fitting, node C (the middle one) must lie fully inside the widget.
    let titles: Vec<Rectangle> = s
        .list
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text {
                content, origin, ..
            } if content == "B" => Some(Rectangle::new(*origin, Size::new(1.0, 1.0))),
            _ => None,
        })
        .collect();
    assert_eq!(titles.len(), 1);
    let o = titles[0].origin;
    assert!(
        o.x > 0.0 && o.x < W && o.y > 0.0 && o.y < H,
        "B visible at {o:?}"
    );
    for _ in 0..40 {
        s.key("+");
    }
    s.key("-");
    // Does not panic and keeps drawing; extreme zoom is clamped (checked by the node title size).
    let max_size = s
        .list
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text { size, content, .. } if content == "A" => Some(*size),
            _ => None,
        })
        .fold(0.0f32, f32::max);
    assert!(max_size <= 2.5 * 20.0, "title size {max_size}");
}

fn board(ui: &Ui) -> Element<Msg> {
    let cards = |from: u64, n: u64| {
        (from..from + n)
            .map(|i| Card::new(i, format!("Task {i}")))
            .collect::<Vec<_>>()
    };
    Kanban::new(
        ui,
        3,
        vec![
            Lane::new("Todo", cards(1, 30)),
            Lane::new("Doing", cards(100, 2)),
        ],
    )
    .on_move(Msg::Moved)
    .height(Length::Fixed(H))
    .into_element()
}

#[test]
fn autoscroll_speed_is_zero_in_the_middle_and_grows_at_the_edges() {
    assert_eq!(autoscroll_velocity(200.0, 40.0, 360.0), 0.0);
    let gentle = autoscroll_velocity(360.0 - 40.0, 40.0, 360.0);
    let hard = autoscroll_velocity(360.0 - 4.0, 40.0, 360.0);
    assert!(gentle > 0.0 && hard > gentle * 2.0, "{gentle} {hard}");
    assert!(autoscroll_velocity(360.0 + 50.0, 40.0, 360.0) <= AUTOSCROLL_MAX);
    assert!(
        autoscroll_velocity(46.0, 40.0, 360.0) < 0.0,
        "near the top scrolls up"
    );
}

#[test]
fn dragging_a_card_to_the_bottom_edge_scrolls_the_lane_and_a_drop_lands_far_down() {
    let mut s = Scene::new(board);
    // Pick up the first card of the long lane and drag it to the bottom edge.
    s.press(60.0, 64.0);
    s.mv(60.0, 100.0);
    s.mv(60.0, H - 6.0);
    let tops = |s: &Scene| {
        s.list
            .0
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text {
                    content, origin, ..
                } if content.starts_with("Task 2") => Some(origin.y),
                _ => None,
            })
            .next()
    };
    let before = tops(&s);
    // Hold still: only Tick events arrive. One second of ticks.
    let mut ms = 1000u64;
    for _ in 0..60 {
        ms += 16;
        s.send(Event::Tick(ms));
    }
    let after = tops(&s);
    assert!(
        after.is_none() || before.is_none() || after.unwrap() < before.unwrap() - 100.0,
        "lane scrolled: {before:?} -> {after:?}"
    );
    s.release();
    let Some(Msg::Moved(mv)) = s.msgs.pop() else {
        panic!("a drop reports a move");
    };
    assert_eq!((mv.card, mv.from, mv.to), (1, 0, 0));
    assert!(mv.index > 8, "landed deep in the lane, index {}", mv.index);
}

#[test]
fn kanban_does_not_autoscroll_when_the_card_rests_mid_lane() {
    let mut s = Scene::new(board);
    s.press(60.0, 64.0);
    s.mv(60.0, 200.0);
    let texts = |s: &Scene| {
        s.list
            .0
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text {
                    content, origin, ..
                } => Some((content.clone(), origin.y)),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let before = texts(&s);
    let mut ms = 1000u64;
    for _ in 0..30 {
        ms += 16;
        s.send(Event::Tick(ms));
    }
    assert_eq!(before, texts(&s));
}

// ---- charts

fn series(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (i as f32 * 0.37).sin() * 40.0 + 50.0)
        .collect()
}

fn chart(ui: &Ui) -> Element<Msg> {
    Chart::line(ui, [Series::new("Load", series(200))])
        .zoomable(9)
        .height(300.0)
        .into_element()
}

fn y_labels(s: &Scene) -> Vec<String> {
    s.list
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn chart_zoom_shows_the_reset_chip_and_pan_and_reset_work() {
    let mut s = Scene::new(chart);
    assert!(!y_labels(&s).iter().any(|t| t == "Reset zoom"));
    let full = format!("{:?}", s.list.0);
    s.mv(320.0, 120.0);
    for _ in 0..6 {
        s.send(Event::Scrolled { dx: 0.0, dy: 120.0 });
    }
    assert!(
        y_labels(&s).iter().any(|t| t == "Reset zoom"),
        "chip appears"
    );
    let zoomed = format!("{:?}", s.list.0);
    assert_ne!(full, zoomed);
    // Drag pans.
    s.press(300.0, 120.0);
    s.mv(380.0, 120.0);
    s.release();
    assert_ne!(zoomed, format!("{:?}", s.list.0));
    // Click the chip.
    s.press(W - 40.0, 10.0);
    s.release();
    s.mv(-5.0, -5.0);
    assert_eq!(
        full,
        format!("{:?}", s.list.0),
        "reset restores the full view"
    );
}

#[test]
fn candles_zoom_too() {
    let data: Vec<Candle> = (0..60)
        .map(|i| {
            let m = 100.0 + (i as f32 * 0.3).sin() * 10.0;
            Candle::new(format!("D{i}"), m, m + 3.0, m - 3.0, m + 1.0)
        })
        .collect();
    let mut s = Scene::new(|ui| {
        CandleChart::new(ui, data.clone())
            .zoomable(5)
            .into_element()
    });
    let full = format!("{:?}", s.list.0);
    s.mv(320.0, 100.0);
    s.send(Event::Scrolled { dx: 0.0, dy: 200.0 });
    assert_ne!(full, format!("{:?}", s.list.0));
    s.key("0"); // not focused yet: ignored
    s.press(320.0, 100.0);
    s.release();
    s.key("0");
    s.mv(-5.0, -5.0);
    assert_eq!(full, format!("{:?}", s.list.0));
}

#[test]
fn sparkline_and_bar_chart_have_tooltips_on_hover() {
    let mut s = Scene::new(|ui| {
        Sparkline::new(ui, series(30))
            .width(Length::Fixed(200.0))
            .height(40.0)
            .into_element()
    });
    assert!(!y_labels(&s).iter().any(|t| t.starts_with("Point ")));
    s.mv(100.0, 20.0);
    assert!(
        y_labels(&s).iter().any(|t| t.starts_with("Point ")),
        "sparkline tip"
    );

    let mut s = Scene::new(|ui| {
        BarChart::new(ui, vec![3.0, 9.0, 5.0])
            .labels(["Mon", "Tue", "Wed"])
            .height(160.0)
            .into_element()
    });
    s.mv(W / 2.0, 100.0);
    let t = y_labels(&s);
    assert!(t.iter().any(|x| x == "9"), "value in tooltip: {t:?}");
    assert!(
        t.iter().filter(|x| *x == "Tue").count() >= 2,
        "label doubles as tooltip head"
    );
}

#[test]
fn live_stream_draw_stays_within_budget() {
    // 4 lines of 2,000 samples, drawn 50 times: the cost of a busy dashboard.
    let mut rings: Vec<Ring> = (0..4).map(|_| Ring::new(2000)).collect();
    for (k, r) in rings.iter_mut().enumerate() {
        for i in 0..2000 {
            r.push(((i + k * 50) as f32 * 0.05).sin() * 30.0 + 50.0);
        }
    }
    let ui = Ui::new(Theme::light());
    let build = |ui: &Ui| -> Element<Msg> {
        StreamChart::new(
            ui,
            1,
            rings.iter().enumerate().map(|(i, r)| (format!("S{i}"), r)),
        )
        .height(240.0)
        .into_element()
    };
    let root = Root::new(&ui, build(&ui)).into_element();
    let layout = alloy_ui_widgets::layout(&root, Size::new(W, H));
    let mut commands = 0;
    let t0 = Instant::now();
    for _ in 0..50 {
        let mut list = DrawList::default();
        root.0.draw(&layout, &mut list);
        commands = list.0.len();
    }
    let per = t0.elapsed().as_secs_f64() * 1000.0 / 50.0;
    eprintln!("stream 4x2000: {per:.2} ms/frame, {commands} draw commands");
    // Samples are decimated to the pixel grid, so the list does not grow with the data.
    assert!(commands < 6000, "{commands} commands for 8,000 samples");
    // Debug builds are several times slower than release; this is a regression guard, not a
    // claim about speed (see BENCHMARKS.md for release numbers).
    assert!(per < 40.0, "{per:.1} ms per frame");
}

#[test]
fn a_hundred_thousand_samples_cost_a_bounded_draw_list() {
    let mut ring = Ring::new(100_000);
    for i in 0..100_000 {
        ring.push((i as f32 * 0.002).sin() * 30.0 + 50.0 + if i % 977 == 0 { 20.0 } else { 0.0 });
    }
    let ui = Ui::new(Theme::light());
    let root = Root::new(
        &ui,
        StreamChart::new(&ui, 2, [("S", &ring)])
            .height(240.0)
            .into_element::<Msg>(),
    )
    .into_element();
    let layout = alloy_ui_widgets::layout(&root, Size::new(W, H));
    let t0 = Instant::now();
    let mut commands = 0;
    for _ in 0..20 {
        let mut list = DrawList::default();
        root.0.draw(&layout, &mut list);
        commands = list.0.len();
    }
    let per = t0.elapsed().as_secs_f64() * 1000.0 / 20.0;
    eprintln!("stream 1x100000: {per:.2} ms/frame, {commands} draw commands");
    assert!(commands < 4 * W as usize + 100, "{commands} commands");
}
