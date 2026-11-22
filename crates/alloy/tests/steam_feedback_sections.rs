//! Steam Oxide feedback, area U: keyed collapsible sections and placeable tooltips.
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct S {
    page: u8,
}

#[test]
fn section_toggles_and_keeps_state_across_pages() {
    let app = sketch(S::default(), |s: &S| {
        col![
            button("Switch").act(|s: &mut S| s.page = 1 - s.page),
            if s.page == 0 {
                fold_section("adv", "Advanced", text("secret body"))
            } else {
                fold_section("adv", "Advanced again", text("secret body"))
            },
        ]
    });
    let mut h = Harness::new(app, 360.0, 300.0);
    h.expect_no("secret body");
    h.click("Advanced").advance(400).expect("secret body");
    // Same id on a different page keeps it open.
    h.click("Switch").advance(100).expect("secret body");
    h.click("Advanced again")
        .advance(400)
        .expect_no("secret body");
}

#[test]
fn section_open_starts_open() {
    let app = sketch(S::default(), |_: &S| {
        fold_section_open("o", "Open one", text("visible body"))
    });
    let mut h = Harness::new(app, 360.0, 200.0);
    h.expect("visible body");
}

#[test]
fn accordion_still_works() {
    let app = sketch(S::default(), |_: &S| {
        alloy_ui::accordion("More", text("acc body"))
    });
    let mut h = Harness::new(app, 360.0, 200.0);
    h.expect_no("acc body");
    h.click("▸ More").expect("acc body");
}

#[test]
fn tooltip_side_offset_and_delay() {
    let build = |side: TipSide, delay: u64| {
        sketch(S::default(), move |_: &S| {
            row![
                button("Left")
                    .tooltip("TIPTEXT")
                    .tooltip_at(side, 10.0)
                    .tooltip_delay(delay),
                button("Neighbour"),
            ]
            .gap(4.0)
        })
    };
    // Default delay: nothing before it, shown after.
    let mut h = Harness::new(build(TipSide::Above, 300), 400.0, 200.0);
    h.hover("Left").advance(100).expect_no("TIPTEXT");
    h.advance(400).expect("TIPTEXT");
    // Zero delay shows right away, on every side.
    for side in [
        TipSide::Below,
        TipSide::Above,
        TipSide::Left,
        TipSide::Right,
    ] {
        let mut h = Harness::new(build(side, 0), 400.0, 200.0);
        h.hover("Left").advance(50).advance(50).expect("TIPTEXT");
    }
}
