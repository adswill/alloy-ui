//! Interaction tests for the visual widgets, driven through the real runtime.

use alloy_ui::prelude::*;
use alloy_ui::rgb;
use alloy_ui::testing::Harness;
use alloy_ui::widgets::{Date, Tone};
use alloy_ui::{
    CalAction, CalView, Entry, GraphAction, GraphNode, Lane, MediaAction, Playback, Pointer, Ring,
    TestCard, Wire,
};

const DAY: Date = Date {
    year: 2026,
    month: 11,
    day: 21,
};

// ------------------------------------------------------------------ kanban

#[derive(Default)]
struct Board {
    lanes: Vec<Lane>,
}

fn board_view(s: &Board) -> impl View<Act<Board>> {
    kanban(&s.lanes).act(|s: &mut Board, mv| mv.apply(&mut s.lanes))
}

fn lanes() -> Vec<Lane> {
    vec![
        lane(
            "Todo",
            [card(1, "Alpha"), card(2, "Bravo"), card(3, "Charlie")],
        ),
        lane("Doing", [card(4, "Delta")]),
        lane("Done", []),
    ]
}

#[test]
fn kanban_drag_moves_a_card_to_another_lane_and_position() {
    let mut ui = Harness::new(sketch(Board { lanes: lanes() }, board_view), 720.0, 420.0);
    let a = ui.position("Alpha").unwrap();
    let doing = ui.position("Doing").unwrap();
    let delta = ui.position("Delta").unwrap();
    // Drop Alpha on the upper half of Delta: it lands before Delta in "Doing".
    ui.drag_px((a.x + 4.0, a.y + 4.0), (delta.x + 10.0, delta.y - 6.0));
    let moved = ui.position("Alpha").unwrap();
    assert!(
        moved.x > doing.x - 20.0,
        "Alpha is now in the Doing lane: {moved:?}"
    );
    assert!(moved.y < ui.position("Delta").unwrap().y, "and above Delta");
    assert!(ui.position("Bravo").unwrap().x < doing.x, "others stay put");
}

#[test]
fn kanban_drop_on_an_empty_lane_and_reorder_within_a_lane() {
    let mut ui = Harness::new(sketch(Board { lanes: lanes() }, board_view), 720.0, 420.0);
    let done = ui.position("Done").unwrap();
    let c = ui.position("Charlie").unwrap();
    ui.drag_px((c.x + 4.0, c.y + 4.0), (done.x + 30.0, done.y + 90.0));
    assert!(
        ui.position("Charlie").unwrap().x > done.x - 20.0,
        "into the empty lane"
    );
    // Reorder: drag Bravo below the remaining last card of the same lane.
    let a0 = ui.position("Alpha").unwrap();
    let b = ui.position("Bravo").unwrap();
    assert!(a0.y < b.y);
    ui.drag_px((a0.x + 4.0, a0.y + 4.0), (b.x + 10.0, b.y + 80.0));
    assert!(ui.position("Alpha").unwrap().y > ui.position("Bravo").unwrap().y);
}

#[test]
fn kanban_click_without_dragging_does_not_move_anything() {
    let mut ui = Harness::new(sketch(Board { lanes: lanes() }, board_view), 720.0, 420.0);
    let before = ui.position("Bravo").unwrap();
    ui.click("Bravo");
    assert_eq!(ui.position("Bravo").unwrap(), before);
}

// ---------------------------------------------------------------- calendar

struct Cal {
    view: CalView,
    day: Date,
    entries: Vec<Entry>,
}

fn cal_view(s: &Cal) -> impl View<Act<Cal>> {
    calendar(s.day, s.view, &s.entries)
        .today(DAY)
        .hours(7, 19)
        .act(|s: &mut Cal, a| match a {
            CalAction::View(v) => s.view = v,
            CalAction::Next => s.day = Date::from_days(s.day.to_days() + 7),
            CalAction::Move { id, date, start } => {
                let e = s.entries.iter_mut().find(|e| e.id == id).unwrap();
                let d = e.end - e.start;
                (e.date, e.start, e.end) = (date, start, start + d);
            }
            CalAction::Resize { id, end } => {
                s.entries.iter_mut().find(|e| e.id == id).unwrap().end = end;
            }
            CalAction::Create { date, minute } => {
                s.entries
                    .push(Entry::new(99, "Created", date, minute, minute + 60));
            }
            CalAction::Day(d) => {
                s.day = d;
                s.view = CalView::Day;
            }
            _ => {}
        })
}

fn week() -> Cal {
    Cal {
        view: CalView::Week,
        day: DAY,
        entries: vec![
            Entry::new(1, "Standup", DAY, 9 * 60, 10 * 60).tone(Tone::Accent),
            Entry::new(2, "Review", DAY, 14 * 60, 15 * 60),
        ],
    }
}

#[test]
fn calendar_drag_moves_an_entry_to_the_next_day_and_snaps_time() {
    let mut ui = Harness::new(sketch(week(), cal_view), 1000.0, 700.0);
    let p = ui.position("Standup").unwrap();
    let mon = ui.position("Mon").unwrap();
    let tue = ui.position("Tue").unwrap();
    let col = tue.x - mon.x;
    // One column right, one hour (52 px) down.
    ui.drag_px(
        (p.x + 6.0, p.y + 14.0),
        (p.x + 6.0 + col, p.y + 14.0 + 52.0),
    );
    let q = ui.position("Standup").unwrap();
    assert!(
        (q.x - p.x - col).abs() < 4.0,
        "moved one column: {q:?} vs {p:?}"
    );
    assert!(
        (q.y - p.y - 52.0).abs() < 8.0,
        "moved about an hour: {q:?} vs {p:?}"
    );
    assert!(ui.has_text("10:00 – 11:00"));
}

#[test]
fn calendar_resize_by_the_bottom_edge_and_create_on_an_empty_slot() {
    let mut ui = Harness::new(sketch(week(), cal_view), 1000.0, 700.0);
    let p = ui.position("Standup").unwrap();
    // The entry is one hour tall (52 px): grab 3 px above its bottom edge, drag 52 px down.
    let bottom = p.y - 4.0 + 52.0 - 4.0;
    ui.drag_px((p.x + 6.0, bottom), (p.x + 6.0, bottom + 52.0));
    assert!(ui.has_text("09:00 – 11:00"), "resized to two hours");
    // A click on an empty slot creates an entry there.
    let r = ui.position("Review").unwrap();
    ui.click_at(r.x + 6.0, r.y + 160.0);
    assert!(ui.has_text("Created"));
}

#[test]
fn calendar_toolbar_switches_view_and_month_cells_drill_into_the_day() {
    let mut ui = Harness::new(sketch(week(), cal_view), 1000.0, 700.0);
    ui.click("Month");
    assert!(ui.has_text("November 2026"));
    assert!(ui.has_text("Standup") || ui.has_text("09:00  Standup"));
    ui.click("Day");
    assert!(ui.has_text("Sat 21 November"));
}

// --------------------------------------------------------------- node graph

struct G {
    nodes: Vec<GraphNode>,
    wires: Vec<Wire>,
}

fn g_view(s: &G) -> impl View<Act<G>> {
    col![
        text(format!("wires {}", s.wires.len())),
        node_graph(&s.nodes, &s.wires).act(|s: &mut G, a| match a {
            GraphAction::Move { id, pos } =>
                s.nodes.iter_mut().find(|n| n.id == id).unwrap().pos = pos,
            GraphAction::Connect(w) => {
                s.wires.retain(|x| x.to != w.to);
                s.wires.push(w);
            }
            _ => {}
        })
    ]
    .fill()
}

fn graph() -> G {
    G {
        nodes: vec![
            node(1, "Source", 0.0, 0.0).outputs(["Out"]),
            node(2, "Sink", 300.0, 40.0).inputs(["In"]),
        ],
        wires: vec![],
    }
}

#[test]
fn node_graph_drag_header_moves_the_node_and_port_drag_connects() {
    let mut ui = Harness::new(sketch(graph(), g_view), 800.0, 500.0);
    assert!(ui.has_text("wires 0"));
    let t = ui.position("Sink").unwrap();
    ui.drag_px(
        (t.x + 10.0, t.y + 6.0),
        (t.x + 10.0 - 60.0, t.y + 6.0 + 40.0),
    );
    let t2 = ui.position("Sink").unwrap();
    assert!(
        (t2.x - (t.x - 60.0)).abs() < 3.0 && (t2.y - (t.y + 40.0)).abs() < 3.0,
        "{t:?} -> {t2:?}"
    );
    // Ports: the input sits on the node's left edge, the output on the source's right edge.
    let inp = ui.position("In").unwrap();
    let out_label = ui.position("Out").unwrap();
    let from = (out_label.x + 40.0 + 14.0, out_label.y + 8.0); // label is right-aligned 14 px inside
    let _ = from;
    let src = ui.position("Source").unwrap();
    // Source node: left edge = title.x - 28; width 180; row 0 port y = label centre.
    let out_port = (src.x - 28.0 + 180.0, out_label.y + 8.0);
    let in_port = (inp.x - 14.0, inp.y + 8.0);
    ui.drag_px(out_port, in_port);
    assert!(
        ui.has_text("wires 1"),
        "dragging output to input adds a wire"
    );
    // Releasing on empty space connects nothing.
    ui.drag_px(out_port, (400.0, 450.0));
    assert!(ui.has_text("wires 1"));
}

#[test]
fn node_graph_scroll_zooms_without_losing_the_nodes() {
    let mut ui = Harness::new(sketch(graph(), g_view), 800.0, 500.0);
    let a = ui.position("Source").unwrap();
    ui.move_to(a.x + 40.0, a.y + 30.0).scroll_here(120.0);
    let b = ui.position("Source").unwrap();
    assert_ne!(a, b, "zoom changes the layout");
}

// -------------------------------------------------------------------- pie

struct P {
    picked: String,
}

fn p_view(s: &P) -> impl View<Act<P>> {
    col![
        text(format!("picked {}", s.picked)),
        pie([("Rent", 60.0), ("Food", 30.0), ("Fun", 10.0)])
            .donut()
            .act(|s: &mut P, i| s.picked = ["Rent", "Food", "Fun"][i].into()),
    ]
    .pad(20.0)
}

#[test]
fn pie_legend_row_click_selects_the_slice() {
    let mut ui = Harness::new(
        sketch(
            P {
                picked: "none".into(),
            },
            p_view,
        ),
        700.0,
        320.0,
    );
    assert!(ui.has_text("picked none"));
    ui.click("Food");
    assert!(ui.has_text("picked Food"));
}

// ----------------------------------------------------------------- canvas

struct C {
    pts: usize,
}

fn c_view(s: &C) -> impl View<Act<C>> {
    col![
        text(format!("pts {}", s.pts)),
        canvas(200.0, 120.0, |f| f.circle(
            pt(100.0, 60.0),
            20.0,
            rgb(0x3558A8)
        ))
        .act(|s: &mut C, p| {
            if let Pointer::Down(_) | Pointer::Move(_) = p {
                s.pts += 1;
            }
        }),
    ]
}

#[test]
fn canvas_reports_pointer_events_in_local_coordinates() {
    let mut ui = Harness::new(sketch(C { pts: 0 }, c_view), 300.0, 220.0);
    ui.drag_px((30.0, 60.0), (150.0, 100.0));
    assert!(!ui.has_text("pts 0"), "down and moves reached the app");
}

// ------------------------------------------------------------------ media

struct M {
    play: Playback,
}

fn m_view(s: &M) -> impl View<Act<M>> {
    col![
        text(if s.play.playing() {
            "playing"
        } else {
            "paused"
        }),
        media(TestCard::video(60), s.play)
            .height(260.0)
            .act(|s: &mut M, a: MediaAction| s.play.apply(a, alloy_ui::ui().now(), 60_000)),
    ]
}

#[test]
fn media_click_toggles_playback_and_the_bar_seeks() {
    let mut ui = Harness::new(
        sketch(
            M {
                play: Playback::new(),
            },
            m_view,
        ),
        520.0,
        360.0,
    );
    assert!(ui.has_text("paused"));
    ui.click_at(260.0, 100.0);
    assert!(ui.has_text("playing"));
    ui.click_at(260.0, 100.0);
    assert!(ui.has_text("paused"));
    let t = ui.position("00:00 / 01:00").unwrap();
    ui.click_at(t.x + 180.0, t.y + 6.0);
    assert!(!ui.has_text("00:00 / 01:00"), "seek bar moved the position");
}

// ----------------------------------------------------------------- stream

#[test]
fn ring_keeps_only_the_newest_samples() {
    let mut r = Ring::new(3);
    for v in [1.0, 2.0, 3.0, 4.0] {
        r.push(v);
    }
    assert_eq!(r.iter().collect::<Vec<_>>(), vec![2.0, 3.0, 4.0]);
    assert_eq!(r.last(), Some(4.0));
}
