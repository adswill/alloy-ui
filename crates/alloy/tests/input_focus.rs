//! Hover state, focusable taps, arrow-key grids, text-field gating and key names.
#![allow(dead_code)]
use alloy_ui::{prelude::*, testing::Harness, Key};
use alloy_ui_core::DrawCommand;

#[derive(Default)]
struct S {
    hover: bool,
    seen: String,
    picked: Vec<&'static str>,
    n: i32,
    typing: bool,
    query: String,
}

fn has_ring(ui: &Harness<impl App>) -> bool {
    ui.draw_list().0.iter().any(|c| {
        matches!(c, DrawCommand::Quad { border: Some(b), fill, .. }
            if b.width == 2.0 && fill.a == 0.0)
    })
}

#[test]
fn on_hover_reports_enter_and_leave() {
    let app = sketch(S::default(), |s: &S| {
        col![
            text(if s.hover { "over" } else { "off" }),
            text("Card").pad(12.0).on_hover(|s: &mut S, h| s.hover = h),
        ]
        .gap(30.0)
    });
    let mut ui = Harness::new(app, 240.0, 200.0);
    ui.expect("off");
    ui.hover("Card").expect("over");
    ui.move_to(230.0, 190.0).expect("off");
}

#[test]
fn hovered_is_readable_while_building_the_view() {
    let app = sketch((), |_: &()| {
        let over = hovered();
        col![
            text(if over.get() { "Play now" } else { "Play" })
                .pad(12.0)
                .watch_hover(over),
            text("Elsewhere"),
        ]
        .gap(40.0)
    });
    let mut ui = Harness::new(app, 240.0, 200.0);
    ui.expect("Play");
    ui.hover("Play").expect("Play now");
    ui.hover("Elsewhere").expect("Play");
}

#[test]
fn hover_follows_the_key_not_the_draw_order() {
    let app = sketch(S::default(), |s: &S| {
        let ids: Vec<u64> = if s.n == 0 { vec![1, 2] } else { vec![2, 1] };
        col(ids.into_iter().map(|id| {
            let over = hovered_for(id);
            text(format!(
                "row {id} {}",
                if over.get() { "hot" } else { "cold" }
            ))
            .pad(8.0)
            .watch_hover(over)
        }))
        .gap(10.0)
        .pad(4.0)
    });
    let mut ui = Harness::new(app, 240.0, 200.0);
    ui.hover("row 2 cold")
        .expect("row 2 hot")
        .expect("row 1 cold");
}

fn taps() -> impl App<Msg = Act<S>> {
    sketch(S::default(), |s: &S| {
        col![
            text(format!("n={}", s.n)),
            row![
                text("A").pad(10.0).tap(|s: &mut S| s.n += 1),
                text("B").pad(10.0).tap(|s: &mut S| s.n += 10),
            ]
            .gap(20.0),
        ]
        .gap(20.0)
        .pad(10.0)
    })
}

#[test]
fn tap_is_reachable_with_tab_and_pressed_with_enter_and_space() {
    let mut ui = Harness::new(taps(), 300.0, 200.0);
    assert!(!has_ring(&ui), "no ring before focus");
    ui.key("Tab").advance(16).advance(400);
    assert!(has_ring(&ui), "focus ring on the first tap");
    ui.key("Enter").expect("n=1");
    ui.key("Tab").key("space").expect("n=11");
    ui.chord("shift+tab").key("Enter").expect("n=12");
    ui.click("B").expect("n=22");
}

fn grid() -> impl App<Msg = Act<S>> {
    sketch(S::default(), |s: &S| {
        let cell = |name: &'static str| {
            text(name)
                .pad(10.0)
                .tap(move |s: &mut S| s.picked.push(name))
        };
        col![
            text(s.picked.join(",")),
            col![
                row![cell("a"), cell("b"), cell("c")].gap(10.0),
                row![cell("d"), cell("e"), cell("f")].gap(10.0),
            ]
            .gap(10.0)
            .arrow_nav(),
        ]
        .gap(20.0)
        .pad(10.0)
    })
}

#[test]
fn arrow_keys_move_through_a_grid_in_two_dimensions() {
    let mut ui = Harness::new(grid(), 300.0, 200.0);
    ui.key("Tab")
        .key("ArrowRight")
        .key("ArrowDown")
        .key("Enter");
    ui.expect("e");
    ui.key("ArrowLeft").key("ArrowUp").key("Enter");
    ui.expect("e,a");
    ui.key("ArrowUp").key("ArrowLeft").key("Enter");
    ui.expect("e,a,a");
}

#[derive(Default)]
struct Typing {
    n: i32,
    typing: bool,
    q: String,
}

fn typing_app() -> impl App<Msg = Act<Typing>> {
    sketch(Typing::default(), |s: &Typing| {
        col![
            text(format!("n={} typing={}", s.n, s.typing)),
            input(at!(s.q)).hint("Search"),
        ]
        .gap(10.0)
        .pad(10.0)
    })
    .shortcut("n", |s| s.n += 1)
    .shortcut("mod+j", |s| s.n += 100)
}

#[test]
fn plain_key_shortcuts_stay_quiet_while_a_text_field_has_focus() {
    let mut ui = Harness::new(typing_app(), 300.0, 200.0);
    ui.press(Key::Character("n".into()))
        .expect("n=1 typing=false");
    ui.click("Search");
    ui.press(Key::Character("n".into()))
        .expect("n=1 typing=false");
    ui.chord("mod+j").expect("n=101 typing=false");
}

#[test]
fn text_focused_is_a_public_query_and_shortcut_typing_opts_in() {
    let app = sketch(Typing::default(), |s: &Typing| {
        col![
            text(format!("n={} typing={}", s.n, s.typing)),
            input(at!(s.q)).hint("Search"),
        ]
        .gap(10.0)
        .pad(10.0)
    })
    .shortcut("mod+p", |s| s.typing = text_focused())
    .shortcut_as("x", "Cut", |s| s.n += 1);
    let mut ui = Harness::new(app, 300.0, 200.0);
    ui.chord("mod+p").expect("n=0 typing=false");
    ui.click("Search").chord("mod+p").expect("n=0 typing=true");
    ui.press(Key::Character("x".into()))
        .expect("n=0 typing=true");
}

#[test]
fn down_and_arrowdown_are_the_same_key_everywhere() {
    let app = sketch(S::default(), |s: &S| text(format!("n={}", s.n)))
        .shortcut("down", |s| s.n += 1)
        .shortcut("Esc", |s| s.n += 10)
        .shortcut("ArrowUp", |s| s.n += 100);
    let mut ui = Harness::new(app, 200.0, 100.0);
    ui.key("ArrowDown").expect("n=1");
    ui.key("down").expect("n=2");
    ui.chord("ArrowDown").expect("n=3");
    ui.key("Escape").expect("n=13");
    ui.key("esc").expect("n=23");
    ui.key("up").expect("n=123");
    assert_eq!(alloy_ui::canonical_key_name("pgdn"), "PageDown");
}
