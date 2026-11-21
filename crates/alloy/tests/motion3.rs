//! Gesture physics and route-level shared elements, through the real runtime.
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;
use alloy_ui::DrawCommand;

fn text_pos(h: &Harness<impl App>, what: &str) -> Option<(f32, f32, f32)> {
    h.draw_list().0.iter().find_map(|c| match c {
        DrawCommand::Text {
            origin,
            content,
            color,
            ..
        } if content == what => Some((origin.x, origin.y, color.a)),
        _ => None,
    })
}

fn lines() -> Harness<impl App> {
    Harness::new(
        sketch((), |_| {
            let rows: Vec<Node<Act<()>>> =
                (0..40).map(|i| text(format!("Line {i}")).into()).collect();
            scroll(col(rows).gap(12.0))
        }),
        300.0,
        240.0,
    )
}

#[test]
fn trackpad_pull_stretches_past_the_top_then_springs_back_and_goes_idle() {
    let mut h = lines();
    h.advance(16);
    let rest = text_pos(&h, "Line 0").expect("drawn").1;
    // Small deltas, as a trackpad sends: pull down past the top.
    for _ in 0..8 {
        h.scroll("Line 0", 12.0);
        h.advance(8);
    }
    let pulled = text_pos(&h, "Line 0").expect("drawn").1;
    assert!(pulled > rest + 20.0, "stretched {pulled} vs rest {rest}");
    assert!(pulled < rest + 160.0, "resists, never runs away: {pulled}");
    // Let go: it returns to the edge and stops asking for frames.
    for _ in 0..80 {
        h.advance(16);
    }
    let back = text_pos(&h, "Line 0").expect("drawn").1;
    assert!(
        (back - rest).abs() < 0.6,
        "settles on the edge: {back} vs {rest}"
    );
    assert!(!h.animating(), "idle again");
}

#[test]
fn mouse_wheel_notches_stop_hard_at_the_edge() {
    let mut h = lines();
    h.advance(16);
    let rest = text_pos(&h, "Line 0").expect("drawn").1;
    h.scroll("Line 0", 120.0);
    h.advance(16).advance(16);
    let y = text_pos(&h, "Line 0").expect("drawn").1;
    assert!((y - rest).abs() < 0.5, "a big notch does not stretch: {y}");
}

#[test]
fn touch_drag_flings_and_coasts_then_stops() {
    let mut h = Harness::new(
        sketch((), |_| {
            let rows: Vec<Node<Act<()>>> =
                (0..60).map(|i| text(format!("Row {i}")).into()).collect();
            scroll_touch(col(rows).gap(12.0))
        }),
        300.0,
        240.0,
    );
    h.advance(16);
    let start = text_pos(&h, "Row 3").expect("drawn").1;
    h.move_to(100.0, 200.0).mouse_down();
    for i in 1..=6 {
        h.advance(8);
        h.move_to(100.0, 200.0 - 14.0 * i as f32);
    }
    let held = text_pos(&h, "Row 3").expect("drawn").1;
    assert!(
        held < start - 40.0,
        "follows the pointer: {held} vs {start}"
    );
    h.advance(8);
    h.mouse_up();
    let at_release = text_pos(&h, "Row 3").map(|p| p.1);
    h.advance(100);
    let later = text_pos(&h, "Row 3").map(|p| p.1);
    // Row 3 scrolled out of view: either it is gone or it moved on after release.
    assert!(
        later.is_none() || at_release.is_none() || later.unwrap() < at_release.unwrap() - 8.0,
        "keeps coasting after release: {at_release:?} -> {later:?}"
    );
    for _ in 0..300 {
        h.advance(16);
    }
    assert!(!h.animating(), "friction brings it to rest");
}

#[test]
fn sheet_follows_a_drag_and_settles_on_the_snap_it_is_heading_for() {
    let mut h = Harness::new(
        sketch((), |_| {
            let page: Node<Act<()>> = col![text("Page")].fill().into();
            let sh: Node<Act<()>> = sheet(&[0.2, 0.6], text("Sheet body"));
            stack([Layer::from(page), Layer::from(sh)]).fill()
        }),
        400.0,
        600.0,
    );
    h.advance(16);
    let low = text_pos(&h, "Sheet body").expect("sheet drawn").1;
    // Grab the handle (panel top is at 600 - 120) and drag it up, then let go while moving.
    h.move_to(200.0, 492.0).mouse_down();
    for i in 1..=8 {
        h.advance(10);
        h.move_to(200.0, 492.0 - 24.0 * i as f32);
    }
    let mid = text_pos(&h, "Sheet body").expect("sheet drawn").1;
    assert!(mid < low - 100.0, "follows the pointer: {mid} vs {low}");
    h.advance(10);
    h.mouse_up();
    for _ in 0..120 {
        h.advance(16);
    }
    let high = text_pos(&h, "Sheet body").expect("sheet drawn").1;
    // The 0.6 snap shows 360 px: the body sits 240 + handle strip from the top.
    assert!(
        (high - (240.0 + 30.0)).abs() < 40.0,
        "rests at the upper snap: {high}"
    );
    assert!(!h.animating(), "settled sheet is idle");
    // Drag back down to the bottom: the lower snap again.
    h.move_to(200.0, 250.0).mouse_down();
    for i in 1..=8 {
        h.advance(10);
        h.move_to(200.0, 250.0 + 40.0 * i as f32);
    }
    h.advance(10);
    h.mouse_up();
    for _ in 0..120 {
        h.advance(16);
    }
    let again = text_pos(&h, "Sheet body").expect("sheet drawn").1;
    assert!(
        (again - low).abs() < 3.0,
        "back at the low snap: {again} vs {low}"
    );
}

#[derive(Clone)]
enum R {
    List,
    Detail,
}

#[test]
fn route_flies_a_shared_element_between_screens_and_settles() {
    let mut h = Harness::new(
        sketch((), |_| {
            let nav = router(R::List);
            col![nav.show_with(RouteKind::Fade, |r| match r {
                R::List => col![
                    button("Open").act(move |_: &mut ()| nav.push(R::Detail)),
                    hero("title", text("Hero")),
                ]
                .gap(8.0)
                .into(),
                R::Detail => col![
                    vspace(),
                    row![space(), hero("title", heading("Hero")), space()],
                    button("Back").act(move |_: &mut ()| nav.back()),
                ]
                .gap(8.0)
                .into(),
            })]
            .page()
        }),
        400.0,
        300.0,
    );
    h.advance(16);
    let a = text_pos(&h, "Hero").expect("hero drawn on the list");
    h.click("Open");
    h.advance(16).advance(70);
    let mid = h
        .draw_list()
        .0
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text { content, .. } if content == "Hero"))
        .count();
    assert!(
        mid >= 2,
        "both renderings are drawn while it flies, got {mid}"
    );
    h.advance(800);
    let b = text_pos(&h, "Hero").expect("hero drawn on the detail page");
    assert!(
        (b.0 - a.0).abs() > 10.0 || (b.1 - a.1).abs() > 10.0,
        "it ended up somewhere else: {a:?} -> {b:?}"
    );
    assert!(b.2 > 0.99, "fully opaque at rest");
    assert!(!h.animating(), "idle after the flight");
}

/// Writes mid-gesture frames for the gallery when `ALLOY_CAPTURE_DIR` is set; a no-op otherwise.
#[test]
fn capture_gallery() {
    let Some(dir) = std::env::var_os("ALLOY_CAPTURE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("dir");
    // A trackpad pull past the top, then the spring back.
    let mut h = lines();
    h.advance(16);
    for _ in 0..8 {
        h.scroll("Line 0", 14.0);
        h.advance(8);
    }
    h.save_png(dir.join("scroll_rubber_band_pulled.png"));
    h.advance(140);
    h.save_png(dir.join("scroll_rubber_band_returning.png"));
    // A sheet being dragged, and after the flick settles.
    let mut h = Harness::new(
        sketch((), |_| {
            let page: Node<Act<()>> = col![heading("Map"), text("Drag the handle below")]
                .fill()
                .into();
            let body: Node<Act<()>> = col![heading("Quick actions"), text("Sheet body")]
                .gap(6.0)
                .into();
            stack([
                Layer::from(page),
                Layer::from(sheet(&[0.2, 0.55, 0.9], body)),
            ])
            .fill()
        }),
        360.0,
        520.0,
    );
    h.advance(16);
    h.save_png(dir.join("sheet_rest_low.png"));
    h.move_to(180.0, 520.0 - 104.0 + 12.0).mouse_down();
    for i in 1..=6 {
        h.advance(10);
        h.move_to(180.0, 428.0 - 30.0 * i as f32);
    }
    h.save_png(dir.join("sheet_dragging.png"));
    h.advance(10);
    h.mouse_up();
    h.advance(90);
    h.save_png(dir.join("sheet_flick_mid_flight.png"));
    for _ in 0..80 {
        h.advance(16);
    }
    h.save_png(dir.join("sheet_settled_mid_snap.png"));
}
