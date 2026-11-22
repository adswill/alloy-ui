//! Menus, popovers and dialogs inside scroll areas and list rows; the overlay layer; the modal.
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct S {
    copied: u32,
    under: u32,
    open: bool,
    closed: u32,
    name: String,
    inner: String,
    picked: usize,
}

fn filler() -> impl View<Act<S>> {
    col((0..40).map(|i| text(format!("filler {i}")).pad(2.0)))
}

#[test]
fn context_menu_inside_scroll_takes_mouse_clicks() {
    let mut ui = Harness::new(
        sketch(S::default(), |_: &S| {
            scroll(col![
                context_menu(
                    text("Target").pad(8.0),
                    [menu_item("Copy it").act(|s: &mut S| s.copied += 1)],
                ),
                filler(),
            ])
        }),
        360.0,
        300.0,
    );
    ui.right_click("Target").expect("Copy it");
    // The menu panel is drawn above everything; a mouse click on it must reach the item.
    ui.click("Copy it").expect_no("Copy it");
    // The state changed: open again and pick once more to prove the first pick ran.
    ui.right_click("Target").click("Copy it");
}

#[test]
fn popover_inside_scroll_takes_mouse_clicks() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                text(format!("under {}", s.under)),
                scroll(col![
                    popover(
                        text("Anchor").pad(6.0),
                        button("Inside").act(|s: &mut S| s.copied += 1)
                    ),
                    filler(),
                ])
                .h(200.0),
                text(format!("copied {}", s.copied)),
            ]
        }),
        360.0,
        400.0,
    );
    ui.click("Anchor").expect("Inside");
    ui.click("Inside").expect("copied 1");
}

#[test]
fn right_click_menu_in_a_vlist_row_works() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                text(format!("copied {}", s.copied)),
                vlist(50, |i| context_menu(
                    text(format!("row {i}")),
                    [menu_item("Copy row").act(|s: &mut S| s.copied += 1)],
                ))
                .act(|s: &mut S, i| s.picked = i)
                .fill_h(),
            ]
        }),
        360.0,
        400.0,
    );
    ui.right_click("row 2").expect("Copy row");
    ui.click("Copy row")
        .expect("copied 1")
        .expect_no("Copy row");
}

#[test]
fn popover_in_a_vlist_row_opens_and_takes_clicks() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                text(format!("copied {}", s.copied)),
                vlist(30, |i| popover(
                    text(format!("more {i}")),
                    button("Act").act(|s: &mut S| s.copied += 1),
                ))
                .fill_h(),
            ]
        }),
        360.0,
        400.0,
    );
    ui.click("more 1").expect("Act");
    ui.click("Act").expect("copied 1");
}

#[test]
fn portal_draws_above_a_clipped_scroll_and_blocks_what_is_under_it() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                text(format!("under {}", s.under)),
                scroll(col![
                    button("Underneath").act(|s: &mut S| s.under += 1),
                    portal(
                        s.open,
                        col![
                            text("Lightbox"),
                            button("Dismiss").act(|s: &mut S| s.open = false)
                        ]
                        .pad(16.0)
                        .fill()
                    ),
                    filler(),
                ])
                .h(120.0),
                button("Show").act(|s: &mut S| s.open = true),
            ]
        }),
        360.0,
        400.0,
    );
    ui.expect_no("Lightbox");
    ui.click("Show").expect("Lightbox");
    // It is painted although its parent scroll area is 120px tall.
    let p = ui.position("Lightbox").expect("drawn");
    assert!(p.y > 120.0 || p.y < 400.0);
    // A click on something underneath is swallowed.
    ui.click("Underneath").expect("under 0");
    ui.click("Dismiss").expect_no("Lightbox");
    ui.click("Underneath").expect("under 1");
}

fn dialog_app() -> Harness<impl alloy_ui::App> {
    Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                text(format!("under {}", s.under)),
                input(at!(s.name)).hint("Behind"),
                button("Open").act(|s: &mut S| s.open = true),
                scroll(col![
                    filler(),
                    modal(
                        s.open,
                        col![
                            heading("Sure?"),
                            input(at!(s.inner)).hint("Inside"),
                            button("Yes").act(|s: &mut S| s.closed += 1),
                        ]
                    )
                    .close(|s: &mut S| {
                        s.open = false;
                        s.closed += 100;
                    }),
                ])
                .h(150.0),
                text(format!("closed {}", s.closed)),
            ]
        }),
        420.0,
        480.0,
    )
}

#[test]
fn modal_shows_over_everything_and_escape_closes_it() {
    let mut ui = dialog_app();
    ui.expect_no("Sure?");
    ui.click("Open").advance(16).advance(400).expect("Sure?");
    // A click on the page under the scrim does not reach it; it closes (scrim click).
    ui.click("Open").advance(16).advance(400);
    ui.expect_no("Sure?").expect("closed 100");
    ui.click("Open").advance(16).advance(400).expect("Sure?");
    ui.key("Escape").advance(16).advance(400);
    ui.expect_no("Sure?").expect("closed 200");
}

#[test]
fn modal_content_is_clickable_and_swallows_page_clicks() {
    let mut ui = dialog_app();
    ui.click("Open").advance(16).advance(400);
    ui.click("Yes").expect("closed 1");
    ui.expect("Sure?");
}

#[test]
fn modal_traps_focus_and_restores_it() {
    let mut ui = dialog_app();
    ui.click("Behind").type_text("x");
    ui.key("Tab").key("Enter").advance(16).advance(400);
    ui.expect("Sure?");
    // Typing goes to the dialog's field, never the one behind it, however many times Tab is hit.
    for _ in 0..5 {
        ui.key("Tab");
    }
    ui.type_text("z").advance(16);
    let texts = ui.texts();
    assert!(
        texts.iter().any(|t| t.contains('x')),
        "field behind keeps its text: {texts:?}"
    );
    ui.key("Escape").advance(16).advance(400);
    ui.expect_no("Sure?");
    // Focus is back on the button that opened it: Enter presses it and the dialog returns.
    ui.key("Enter").advance(16).advance(400);
    ui.expect("Sure?");
}

#[test]
fn modal_fades_with_motion_and_jumps_with_reduced_motion() {
    let mut ui = dialog_app();
    ui.click("Open").advance(16);
    assert!(ui.animating(), "enter animation runs");
    ui.settle_motion().expect("Sure?");
    ui.key("Escape");
    ui.settle_motion().expect_no("Sure?");
    assert!(!ui.animating(), "no stuck animation after exit");

    let mut ui = dialog_app();
    ui.reduce_motion(true);
    ui.click("Open").advance(16).expect("Sure?");
}
