//! Steam feedback, area L: events and hover inside vlist/vgrid rows, `reveal_appear` in rows,
//! vgrid focus ring and Enter, carousel height, lazy slides and idle wakes.
use alloy_ui::{prelude::*, testing::Harness};
use alloy_ui_core::DrawCommand;

#[derive(Default)]
struct S {
    opened: u32,
    hot: Option<usize>,
    activated: Option<usize>,
    picked: Option<usize>,
}

fn menu_list() -> impl App {
    sketch(S::default(), |s: &S| {
        col![
            text(format!("opened {}", s.opened)),
            vlist(5, |i| context_menu(
                text(format!("row {i}")),
                [menu_item(format!("Open {i}")).act(|s: &mut S| s.opened += 1)],
            ))
            .row_height(30.0)
            .fill_h(),
        ]
    })
}

#[test]
fn escape_closes_a_row_context_menu() {
    let mut ui = Harness::new(menu_list(), 300.0, 300.0);
    ui.right_click("row 1").expect("Open 1");
    ui.key("Escape").expect_no("Open 1");
}

#[test]
fn clicking_another_row_dismisses_the_open_menu_and_only_one_is_open() {
    let mut ui = Harness::new(menu_list(), 300.0, 300.0);
    ui.right_click("row 1").expect("Open 1");
    ui.right_click("row 3").expect("Open 3").expect_no("Open 1");
    // A plain click away closes it without picking.
    ui.click("row 0").expect_no("Open 3").expect("opened 0");
}

#[test]
fn arrow_and_enter_drive_the_open_row_menu_not_the_list() {
    let mut ui = Harness::new(menu_list(), 300.0, 300.0);
    ui.right_click("row 2").expect("Open 2");
    ui.key("ArrowDown").key("Enter");
    ui.expect("opened 1").expect_no("Open 2");
}

#[test]
fn each_row_has_its_own_key_scope() {
    // Two rows' menus must not share an identity: opening row 4's menu leaves row 2's closed.
    let mut ui = Harness::new(menu_list(), 300.0, 300.0);
    ui.right_click("row 2").key("Escape");
    ui.right_click("row 4").expect("Open 4").expect_no("Open 2");
}

#[test]
fn hover_works_inside_vlist_rows() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                text(format!("hot {:?}", s.hot)),
                vlist(5, |i| {
                    let over = hovered_for(i as u64);
                    text(if over.get() { "PLAY" } else { "row" })
                        .watch_hover(over)
                        .on_hover(move |s: &mut S, h| s.hot = h.then_some(i))
                })
                .row_height(30.0)
                .fill_h(),
            ]
        }),
        300.0,
        300.0,
    );
    ui.expect("hot None");
    ui.move_to(20.0, 20.0 + 30.0 * 2.0 + 15.0).advance(16);
    ui.expect("hot Some(2)").expect("PLAY");
    ui.move_to(280.0, 5.0).advance(16);
    ui.expect("hot None").expect_no("PLAY");
}

#[test]
fn hover_bg_inside_vgrid_cells_and_popovers_open() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                text(format!("hot {:?}", s.hot)),
                vgrid(6, 2, |i| text(format!("cell {i}"))
                    .on_hover(move |s: &mut S, h| s.hot = h.then_some(i)))
                .cell_height(40.0)
                .fill_h(),
            ]
        }),
        300.0,
        300.0,
    );
    ui.hover("cell 3").advance(16).expect("hot Some(3)");
}

#[test]
fn reveal_appear_inside_a_vlist_row_paints() {
    let mut ui = Harness::new(
        sketch((), |_: &()| {
            vlist(3, |i| {
                keyed(i as u64, || reveal_appear(true, text(format!("fade {i}"))))
            })
            .row_height(30.0)
            .fill_h()
        }),
        300.0,
        200.0,
    );
    let text_cmds = |ui: &Harness<_>| {
        ui.draw_list()
            .0
            .iter()
            .filter(|c| matches!(c, DrawCommand::Text { .. }))
            .count()
    };
    ui.advance(16);
    assert!(
        ui.animating(),
        "a row that is fading in keeps frames coming"
    );
    ui.settle_motion();
    assert!(text_cmds(&ui) >= 3, "rows are painted after the ease");
    assert!(!ui.animating());
}

fn grid_app(ring: bool) -> impl App {
    sketch(S::default(), move |s: &S| {
        let g = vgrid(12, 3, |i| text(format!("cell {i}")))
            .cell_height(40.0)
            .act(|s: &mut S, i| s.picked = Some(i))
            .on_activate(|s: &mut S, i| s.activated = Some(i));
        col![
            text(format!("picked {:?} opened {:?}", s.picked, s.activated)),
            if ring { g } else { g.focus_ring(false) }.fill_h()
        ]
    })
}

fn ring_quads(ui: &Harness<impl App>) -> usize {
    ui.draw_list()
        .0
        .iter()
        .filter(|c| {
            matches!(c, DrawCommand::Quad { border: Some(b), fill, .. }
                if b.width == 2.0 && fill.a == 0.0)
        })
        .count()
}

#[test]
fn vgrid_focus_ring_can_be_hidden() {
    let mut ui = Harness::new(grid_app(true), 300.0, 300.0);
    ui.click("cell 0");
    assert!(ring_quads(&ui) > 0);
    let mut ui = Harness::new(grid_app(false), 300.0, 300.0);
    ui.click("cell 0");
    assert_eq!(ring_quads(&ui), 0);
}

#[test]
fn vgrid_enter_activates_the_highlighted_cell_and_arrows_do_not() {
    let mut ui = Harness::new(grid_app(false), 300.0, 300.0);
    ui.click("cell 0").key("ArrowRight").key("ArrowDown");
    ui.expect("picked Some(4) opened None");
    ui.key("Enter").expect("picked Some(4) opened Some(4)");
}

fn slides_app(lazy: bool) -> impl App {
    sketch((), move |_: &()| {
        let c = if lazy {
            carousel_with(5, |i| text(format!("Slide {i}")))
        } else {
            carousel((0..5).map(|i| text(format!("Slide {i}"))))
        };
        col![c.auto(secs(2.0)), text("below")]
    })
}

#[test]
fn carousel_without_height_is_the_first_slide_tall_not_huge() {
    let ui = Harness::new(slides_app(false), 300.0, 800.0);
    let below = ui.position("below").unwrap().y;
    assert!(below < 200.0, "default height is small, got {below}");
}

#[test]
fn lazy_carousel_builds_only_current_and_next_and_still_advances() {
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    static BUILT: AtomicUsize = AtomicUsize::new(0);
    let mut ui = Harness::new(
        sketch((), |_: &()| {
            col![
                carousel_with(6, |i| {
                    BUILT.fetch_add(1, SeqCst);
                    text(format!("Slide {i}"))
                })
                .height(120.0),
                text("below"),
            ]
            .w(300.0)
        }),
        300.0,
        300.0,
    );
    ui.advance(16);
    // First, current and next per build pass: slide 0 and 1 only.
    assert_eq!(BUILT.load(SeqCst) % 2, 0);
    ui.expect("Slide 0").expect_no("Slide 3");
    let before = BUILT.load(SeqCst);
    // Click the next arrow: slide 1 shows (and slide 2 is built for next time).
    let n = ui.draw_list().0.iter().find_map(|c| match c {
        DrawCommand::Quad { bounds, radius, .. }
            if bounds.size.width == 32.0 && *radius == 16.0 && bounds.origin.x > 200.0 =>
        {
            Some((bounds.origin.x + 16.0, bounds.origin.y + 16.0))
        }
        _ => None,
    });
    let (x, y) = n.expect("next arrow");
    ui.click_at(x, y).advance(16).advance(600);
    ui.expect("Slide 1").expect_no("Slide 0");
    assert!(BUILT.load(SeqCst) > before);
}

#[test]
fn an_auto_carousel_stops_asking_for_frames_while_hovered() {
    let mut ui = Harness::new(slides_app(true), 300.0, 300.0);
    ui.advance(16);
    assert!(ui.animating(), "the timer needs ticks when nobody hovers");
    ui.hover("Slide 0").advance(16);
    assert!(!ui.animating(), "held still while the pointer is over it");
}
