//! Golden-image tests for the visual widgets (charts, kanban, calendar, node graph, canvas,
//! 3D, media). Text is stripped (glyphs differ per OS); shapes, colours and geometry are kept.
//! Regenerate with `UPDATE_GOLDEN=1 cargo test -p alloy-ui-widgets --test golden_viz`.

use alloy_ui_core::*;
use alloy_ui_render::testing::{assert_golden, Tolerance};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::{path::Path as FsPath, rc::Rc};

const W: u32 = 640;
const H: u32 = 360;

fn render(theme: Theme, build: impl Fn(&Ui) -> Element<()>) -> alloy_ui_render::Image {
    let bg = theme.palette.background;
    let ui = Ui::new(theme);
    let root = Root::new(&ui, build(&ui)).into_element();
    let layout = alloy_ui_widgets::layout(&root, Size::new(W as f32, H as f32));
    let mut list = DrawList::default();
    root.0.draw(&layout, &mut list);
    list.0.retain(|c| !matches!(c, DrawCommand::Text { .. }));
    let mut r = CpuRenderer::new(W * 2, H * 2);
    r.set_background(bg);
    r.render_to_image(&list, W * 2, H * 2, 2.0)
}

fn check(name: &str, img: &alloy_ui_render::Image) {
    let dir = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    assert_golden(&dir, name, img, Tolerance::CPU);
}

fn pad(e: Element<()>) -> Element<()> {
    Container::new(e)
        .padding(alloy_ui_core::style::Padding::all(16.0))
        .width(alloy_ui_layout::Length::Fill(1))
        .height(alloy_ui_layout::Length::Fill(1))
        .into_element()
}

const DAY: Date = Date {
    year: 2026,
    month: 11,
    day: 21,
};

fn slices() -> Vec<Slice> {
    vec![
        Slice::new("Rent", 1200.0),
        Slice::new("Food", 540.0),
        Slice::new("Travel", 310.0),
        Slice::new("Health", 180.0),
    ]
}

#[test]
fn pie_and_donut() {
    for (name, theme) in [
        ("viz_pie_light", Theme::light()),
        ("viz_pie_dark", Theme::dark()),
    ] {
        let img = render(theme, |ui| {
            pad(Row::new(vec![
                PieChart::<()>::new(ui, slices())
                    .height(200.0)
                    .into_element(),
                PieChart::<()>::new(ui, slices())
                    .donut()
                    .selected(Some(1))
                    .height(200.0)
                    .into_element(),
            ])
            .spacing(16.0)
            .into_element())
        });
        check(name, &img);
    }
}

#[test]
fn candles_with_volume() {
    let candles: Vec<Candle> = (0..24)
        .map(|i| {
            let b = 100.0 + (i as f32 * 0.7).sin() * 12.0;
            let up = i % 3 != 0;
            let (o, c) = if up { (b, b + 4.0) } else { (b + 3.0, b - 2.0) };
            Candle::new(format!("{i}"), o, o.max(c) + 3.0, o.min(c) - 3.0, c)
                .volume(500.0 + (i * 37 % 11) as f32 * 120.0)
        })
        .collect();
    let img = render(Theme::light(), |ui| {
        pad(CandleChart::new(ui, candles.clone())
            .height(300.0)
            .into_element())
    });
    check("viz_candles_light", &img);
}

#[test]
fn stream_settled() {
    let mut cpu = Ring::new(40);
    let mut gpu = Ring::new(40);
    for i in 0..40 {
        cpu.push(40.0 + (i as f32 * 0.35).sin() * 25.0);
        gpu.push(25.0 + (i as f32 * 0.2).cos() * 12.0);
    }
    let img = render(Theme::light(), |ui| {
        pad(StreamChart::new(ui, 7, [("CPU", &cpu), ("GPU", &gpu)])
            .smooth(false)
            .height(220.0)
            .into_element())
    });
    check("viz_stream_light", &img);
}

fn lanes() -> Vec<Lane> {
    vec![
        Lane::new(
            "Todo",
            [
                Card::new(1, "Write docs").detail("Docs"),
                Card::new(2, "Fix crash").tag("Blocked", Tone::Danger),
            ],
        ),
        Lane::new(
            "Doing",
            [Card::new(3, "Build board").tag("On track", Tone::Success)],
        ),
        Lane::new("Done", [Card::new(4, "Ship")]),
    ]
}

#[test]
fn kanban_rest() {
    for (name, theme) in [
        ("viz_kanban_light", Theme::light()),
        ("viz_kanban_dark", Theme::dark()),
    ] {
        let img = render(theme, |ui| {
            pad(Kanban::<()>::new(ui, 3, lanes()).into_element())
        });
        check(name, &img);
    }
}

fn entries() -> Vec<Entry> {
    vec![
        Entry::new(1, "Standup", DAY, 9 * 60 + 30, 10 * 60),
        Entry::new(2, "Review", DAY, 11 * 60, 12 * 60 + 30).tone(Tone::Success),
        Entry::new(3, "Sync", DAY, 11 * 60 + 30, 13 * 60).tone(Tone::Warning),
        Entry::new(
            4,
            "Demo",
            Date::from_days(DAY.to_days() + 2),
            14 * 60,
            15 * 60,
        )
        .tone(Tone::Danger),
    ]
}

#[test]
fn calendar_views() {
    for (name, view) in [
        ("viz_calendar_week", CalView::Week),
        ("viz_calendar_month", CalView::Month),
        ("viz_calendar_day", CalView::Day),
    ] {
        let img = render(Theme::light(), |ui| {
            pad(Calendar::<()>::new(ui, 5, DAY, view, entries())
                .today(DAY)
                .now(13 * 60)
                .hours(8, 16)
                .into_element())
        });
        check(name, &img);
    }
}

#[test]
fn calendar_banners_in_week_and_month() {
    let mut es = entries();
    let ws = Date::from_days(DAY.to_days() - DAY.weekday() as i64);
    let d = |n: i64| Date::from_days(ws.to_days() + n);
    es.push(Entry::all_day(10, "Offsite", d(1), 3));
    es.push(Entry::all_day(11, "Release", d(2), 1).tone(Tone::Success));
    es.push(Entry::all_day(12, "Long", d(5), 5).tone(Tone::Warning));
    for (name, view) in [
        ("viz_calendar_banners_week", CalView::Week),
        ("viz_calendar_banners_month", CalView::Month),
    ] {
        let es = es.clone();
        let img = render(Theme::light(), move |ui| {
            pad(Calendar::<()>::new(ui, 5, DAY, view, es.clone())
                .today(DAY)
                .hours(8, 16)
                .into_element())
        });
        check(name, &img);
    }
}

#[test]
fn rich_editor_code_and_image_blocks() {
    let doc = RichText::from_markdown(
        "Some text above.\n\n```\nlet x = run();\nlog(x);\n```\n\n![A diagram](diagram.png)",
    );
    for (name, theme) in [
        ("viz_rich_blocks_light", Theme::light()),
        ("viz_rich_blocks_dark", Theme::dark()),
    ] {
        let doc = doc.clone();
        let img = render(theme, move |ui| {
            pad(RichEditor::<()>::new(ui, 3, &doc).into_element())
        });
        check(name, &img);
    }
}

#[test]
fn node_graph_default() {
    let nodes = vec![
        GraphNode::new(1, "A", 10.0, 20.0)
            .outputs(["x", "y"])
            .tone(Tone::Accent),
        GraphNode::new(2, "B", 250.0, 10.0)
            .inputs(["in"])
            .outputs(["out"]),
        GraphNode::new(3, "C", 250.0, 130.0).inputs(["a", "b"]),
    ];
    let wires = vec![Wire::new((1, 0), (2, 0)), Wire::new((1, 1), (3, 1))];
    let img = render(Theme::light(), |ui| {
        pad(NodeGraph::<()>::new(ui, 9, nodes.clone(), wires.clone())
            .selected(Some(2))
            .into_element())
    });
    check("viz_graph_light", &img);
}

#[test]
fn canvas_paths() {
    let img = render(Theme::light(), |ui| {
        pad(Canvas::<()>::new(
            ui,
            11,
            alloy_ui_layout::Length::Fill(1),
            alloy_ui_layout::Length::Fill(1),
            |f| {
                let p = f.palette();
                let blob = Path::new()
                    .move_to(40.0, 200.0)
                    .cubic_to(40.0, 100.0, 180.0, 120.0, 200.0, 200.0)
                    .cubic_to(220.0, 290.0, 60.0, 300.0, 40.0, 200.0)
                    .close();
                f.fill(&blob, p.accent.with_alpha(0.25));
                f.stroke(&blob, 2.0, p.accent);
                f.fill(
                    &Path::sector(Point::new(400.0, 150.0), 40.0, 90.0, 0.2, 4.0),
                    p.success,
                );
                f.stroke(
                    &Path::circle(Point::new(400.0, 150.0), 100.0),
                    1.0,
                    p.border_strong,
                );
            },
        )
        .into_element())
    });
    check("viz_canvas_light", &img);
}

#[test]
fn viewport3d_scene() {
    let img = render(Theme::light(), |ui| {
        pad(Viewport3d::new(
            ui,
            13,
            [
                Object::new(Mesh::torus(1.4, 0.5, 36))
                    .color(Color::hex(0x3558A8))
                    .at(0.0, 0.6, 0.0),
                Object::new(Mesh::cube(1.0))
                    .color(Color::hex(0x4F8A6B))
                    .at(3.0, 0.5, 0.0)
                    .rotate(0.0, 0.5, 0.0),
                Object::new(Mesh::sphere(0.8, 24))
                    .color(Color::hex(0xC2603B))
                    .at(-3.0, 0.8, 0.0),
            ],
        )
        .orbit(Orbit {
            yaw: 0.6,
            pitch: 0.5,
            distance: 9.0,
        })
        .height(alloy_ui_layout::Length::Fill(1))
        .into_element())
    });
    check("viz_3d_light", &img);
}

#[test]
fn media_player_paused_and_playing() {
    let src: Rc<dyn MediaSource> = Rc::new(TestCard::video(60));
    let img = render(Theme::light(), |ui| {
        pad(MediaPlayer::<()>::new(ui, 15, src.clone(), Playback::new())
            .height(alloy_ui_layout::Length::Fill(1))
            .into_element())
    });
    check("viz_media_paused", &img);
}

// ---------------------------------------------------------------- unit tests

#[test]
fn path_flatten_closes_and_curves_stay_smooth() {
    let sq = Path::new()
        .move_to(0.0, 0.0)
        .line_to(10.0, 0.0)
        .line_to(10.0, 10.0)
        .close();
    let f = sq.flatten();
    assert_eq!(f.len(), 1);
    assert!(f[0].1, "closed");
    assert_eq!(f[0].0.len(), 3);
    let c = Path::circle(Point::new(0.0, 0.0), 50.0).flatten();
    assert!(c[0].0.len() > 24, "a 50 px circle needs many segments");
    // Every flattened point lies on the circle within the 1/4 px tolerance.
    for p in &c[0].0 {
        assert!(((p.x * p.x + p.y * p.y).sqrt() - 50.0).abs() < 0.05);
    }
    let open = Path::polyline([Point::new(0.0, 0.0), Point::new(5.0, 5.0)]).flatten();
    assert!(!open[0].1);
}

#[test]
fn kanban_move_applies_with_index_after_removal() {
    let mut l = lanes();
    Move {
        card: 1,
        from: 0,
        to: 1,
        index: 0,
    }
    .apply(&mut l);
    assert_eq!(
        l[1].cards.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(l[0].cards.len(), 1);
    // Reordering inside a lane: card 3 moves to the front.
    Move {
        card: 3,
        from: 1,
        to: 1,
        index: 0,
    }
    .apply(&mut l);
    assert_eq!(
        l[1].cards.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![3, 1]
    );
}

#[test]
fn playback_position_is_a_pure_function_of_the_clock() {
    let mut p = Playback::new();
    assert_eq!(p.position(5_000, 60_000), 0);
    p.apply(MediaAction::Toggle, 1_000, 60_000);
    assert!(p.playing());
    assert_eq!(p.position(4_500, 60_000), 3_500);
    p.apply(MediaAction::Seek(10_000), 4_500, 60_000);
    assert_eq!(p.position(5_500, 60_000), 11_000);
    p.apply(MediaAction::Toggle, 5_500, 60_000);
    assert_eq!(p.position(99_000, 60_000), 11_000, "paused stays put");
    assert_eq!(p.position(0, 5_000), 5_000, "clamped to the duration");
    p.apply(MediaAction::Volume(2.0), 0, 1);
    assert_eq!(p.volume, 1.0);
}

#[test]
fn ring_window_and_capacity() {
    let mut r = Ring::filled(4, 1.0);
    r.push(9.0);
    assert_eq!(r.iter().collect::<Vec<_>>(), vec![1.0, 1.0, 1.0, 9.0]);
    assert_eq!(r.capacity(), 4);
}

#[test]
fn mesh_counts() {
    assert_eq!(Mesh::cube(1.0).triangle_count(), 12);
    assert!(Mesh::sphere(1.0, 16).triangle_count() > 100);
}

fn pixel(img: &alloy_ui_render::Image, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    [
        img.rgba[i],
        img.rgba[i + 1],
        img.rgba[i + 2],
        img.rgba[i + 3],
    ]
}

/// A five-point star under both fill rules, frames with holes, caps and joins, a dashed
/// curve and a gradient-filled shape.
#[test]
fn canvas_fill_rules_strokes_and_gradients() {
    let img = render(Theme::light(), |ui| {
        pad(Canvas::<()>::new(
            ui,
            12,
            alloy_ui_layout::Length::Fill(1),
            alloy_ui_layout::Length::Fill(1),
            |f| {
                let p = f.palette();
                let star = |cx: f32, cy: f32, r: f32| {
                    let mut path = Path::new();
                    for i in 0..5 {
                        let a = (i as f32 * 144.0 - 90.0).to_radians();
                        let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
                        path = if i == 0 {
                            path.move_to(x, y)
                        } else {
                            path.line_to(x, y)
                        };
                    }
                    path.close()
                };
                f.fill_with(&star(60.0, 52.0, 46.0), FillRule::NonZero, p.accent);
                f.fill_with(&star(170.0, 52.0, 46.0), FillRule::EvenOdd, p.accent);
                // A square with a square hole: opposite winding under non-zero, and the
                // same winding under even-odd.
                let frame = |x: f32, rev: bool| {
                    let outer = Path::new()
                        .move_to(x, 110.0)
                        .line_to(x + 80.0, 110.0)
                        .line_to(x + 80.0, 190.0)
                        .line_to(x, 190.0)
                        .close();
                    let (a, b) = (x + 20.0, x + 60.0);
                    if rev {
                        outer
                            .move_to(a, 130.0)
                            .line_to(a, 170.0)
                            .line_to(b, 170.0)
                            .line_to(b, 130.0)
                            .close()
                    } else {
                        outer
                            .move_to(a, 130.0)
                            .line_to(b, 130.0)
                            .line_to(b, 170.0)
                            .line_to(a, 170.0)
                            .close()
                    }
                };
                f.fill_with(&frame(10.0, true), FillRule::NonZero, p.success);
                f.fill_with(&frame(120.0, false), FillRule::EvenOdd, p.warning);
                // The same zigzag with each cap and join.
                for (i, (cap, join)) in [
                    (LineCap::Butt, LineJoin::Miter),
                    (LineCap::Round, LineJoin::Round),
                    (LineCap::Square, LineJoin::Bevel),
                ]
                .into_iter()
                .enumerate()
                {
                    let y = i as f32 * 44.0;
                    let zig = Path::polyline([
                        Point::new(290.0, 40.0 + y),
                        Point::new(330.0, 8.0 + y),
                        Point::new(370.0, 40.0 + y),
                        Point::new(410.0, 8.0 + y),
                    ]);
                    f.stroke_with(&zig, &Stroke::new(10.0).cap(cap).join(join), p.text);
                }
                // A dashed cubic and a gradient-filled rounded shape.
                let curve = Path::new()
                    .move_to(10.0, 250.0)
                    .cubic_to(100.0, 210.0, 190.0, 280.0, 270.0, 236.0);
                f.stroke_with(
                    &curve,
                    &Stroke::new(4.0)
                        .cap(LineCap::Round)
                        .dashed(&[10.0, 8.0], 0.0),
                    p.accent,
                );
                f.fill_with(
                    &Path::rounded_rect(
                        Rectangle::new(Point::new(300.0, 150.0), Size::new(170.0, 110.0)),
                        18.0,
                    ),
                    FillRule::NonZero,
                    LinearGradient::new(Point::new(300.0, 150.0), Point::new(470.0, 260.0))
                        .stop(0.0, p.accent)
                        .stop(1.0, p.success),
                );
            },
        )
        .into_element())
    });
    // At scale 2 with 16 px of padding in the container and the same again from the
    // widget, canvas pixel (x, y) is at ((x + 32) * 2, (y + 32) * 2).
    let at = |x: f32, y: f32| pixel(&img, ((x + 32.0) * 2.0) as u32, ((y + 32.0) * 2.0) as u32);
    let bg = at(3.0, 3.0);
    assert_ne!(at(60.0, 52.0), bg, "non-zero fills the star's centre");
    assert_eq!(
        at(170.0, 52.0),
        bg,
        "even-odd leaves the star's centre empty"
    );
    assert_ne!(at(170.0, 22.0), bg, "even-odd still fills a tip");
    assert_eq!(
        at(50.0, 150.0),
        bg,
        "opposite winding cuts a hole under non-zero"
    );
    assert_ne!(at(20.0, 150.0), bg, "the frame around the hole is filled");
    assert_eq!(
        at(160.0, 150.0),
        bg,
        "even-odd cuts a hole whatever the winding"
    );
    assert_ne!(
        at(310.0, 170.0),
        at(460.0, 250.0),
        "the gradient changes along its axis"
    );
    // No pinholes: every pixel inside the solid frame is the fill colour.
    let fill = at(20.0, 150.0);
    for y in 112..188 {
        for x in 12..88 {
            if (28.0..72.0).contains(&(x as f32)) && (128.0..172.0).contains(&(y as f32)) {
                continue; // the hole
            }
            assert_eq!(at(x as f32, y as f32), fill, "pinhole at {x},{y}");
        }
    }
    check("viz_canvas_rules_light", &img);
}
