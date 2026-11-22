//! Section E: vgrid, stick-to-bottom scroll, split pane width, carousel, weighted coloured text.
use alloy_ui::{prelude::*, testing::Harness};
use alloy_ui_core::{Color, DrawCommand};

// ---------------------------------------------------------------- 17 vgrid

#[derive(Default)]
struct Grid {
    picked: Option<usize>,
}

fn grid_app() -> impl App {
    sketch(Grid::default(), |s: &Grid| {
        col![
            text(format!("Picked {:?}", s.picked)),
            vgrid(10_000, 4, |i| text(format!("Cell {i}")).pad(6.0))
                .cell_height(60.0)
                .act(|s: &mut Grid, i| s.picked = Some(i)),
        ]
    })
}

#[test]
fn vgrid_builds_only_the_visible_cells() {
    let mut ui = Harness::new(grid_app(), 400.0, 300.0);
    ui.advance(16)
        .expect("Cell 0")
        .expect("Cell 3")
        .expect("Cell 4");
    ui.expect_no("Cell 5000");
    let shown = ui.texts().iter().filter(|t| t.starts_with("Cell ")).count();
    assert!(shown > 0 && shown <= 40, "recycled: {shown} cells built");
}

#[test]
fn vgrid_scrolls_and_picks_a_cell() {
    let mut ui = Harness::new(grid_app(), 400.0, 300.0);
    ui.advance(16);
    ui.scroll("Cell 0", -400.0).advance(16).advance(600);
    ui.expect_no("Cell 0");
    // A cell on screen can still be clicked after scrolling.
    let name = ui
        .draw_list()
        .0
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text {
                content, origin, ..
            } if content.starts_with("Cell ") && origin.y > 60.0 && origin.y < 250.0 => {
                Some(content.clone())
            }
            _ => None,
        })
        .unwrap();
    let n: usize = name[5..].parse().unwrap();
    ui.click(&name).expect(&format!("Picked Some({n})"));
}

#[test]
fn vgrid_arrow_keys_move_in_two_dimensions() {
    let mut ui = Harness::new(grid_app(), 400.0, 300.0);
    ui.advance(16).click("Cell 1").expect("Picked Some(1)");
    ui.key("ArrowDown").expect("Picked Some(5)");
    ui.key("ArrowRight").expect("Picked Some(6)");
    ui.key("ArrowUp").expect("Picked Some(2)");
    ui.key("ArrowLeft").expect("Picked Some(1)");
    ui.key("End");
    glide(&mut ui);
    ui.expect("Picked Some(9999)");
    ui.expect("Cell 9999");
}

// ---------------------------------------------------------------- 18 stick to bottom

#[derive(Default)]
struct Chat {
    n: usize,
}

/// Play 16 ms frames until a glide has finished.
fn glide(ui: &mut Harness<impl App>) {
    for _ in 0..300 {
        ui.advance(16);
    }
}

/// True when `s` is drawn inside the window (the scroll area clips what lies outside).
fn on_screen(ui: &Harness<impl App>, s: &str, height: f32) -> bool {
    ui.draw_list().0.iter().any(|c| match c {
        DrawCommand::Text {
            content, origin, ..
        } => content == s && origin.y >= 0.0 && origin.y < height - 20.0,
        _ => false,
    })
}

fn chat_app() -> impl App {
    sketch(Chat { n: 40 }, |s: &Chat| {
        col![
            scroll_bottom(col((0..s.n).map(|i| text(format!("msg {i}"))))),
            button("Send").act(|s: &mut Chat| s.n += 1),
        ]
    })
}

#[test]
fn chat_scroll_starts_at_the_newest_and_follows_it() {
    let mut ui = Harness::new(chat_app(), 300.0, 200.0);
    ui.advance(16).advance(16);
    assert!(on_screen(&ui, "msg 39", 200.0) && !on_screen(&ui, "msg 0", 200.0));
    ui.click("Send").advance(16);
    assert!(on_screen(&ui, "msg 40", 200.0) && !on_screen(&ui, "msg 0", 200.0));
    ui.click("Send").click("Send").advance(16);
    assert!(on_screen(&ui, "msg 42", 200.0));
}

#[test]
fn chat_scroll_stops_following_once_the_user_scrolls_up() {
    let mut ui = Harness::new(chat_app(), 300.0, 200.0);
    ui.advance(16).advance(16);
    ui.scroll("msg 39", 3000.0).advance(16);
    glide(&mut ui);
    assert!(on_screen(&ui, "msg 0", 200.0) && !on_screen(&ui, "msg 39", 200.0));
    ui.click("Send").advance(16);
    glide(&mut ui);
    assert!(on_screen(&ui, "msg 0", 200.0) && !on_screen(&ui, "msg 40", 200.0));
    // Back at the end, it follows again.
    ui.scroll("msg 0", -3000.0).advance(16);
    glide(&mut ui);
    assert!(on_screen(&ui, "msg 40", 200.0));
    ui.click("Send").advance(16);
    assert!(on_screen(&ui, "msg 41", 200.0));
}

// ---------------------------------------------------------------- 19 split width

#[test]
fn the_view_reads_the_split_pane_width_and_follows_a_drag() {
    let mut ui = Harness::new(
        sketch((), |_: &()| {
            let side = split_handle();
            split(
                text(format!("Side {}", side.width().round() as i32)),
                text("Main"),
            )
            .handle(side)
            .ratio(0.3)
            .min(40.0)
        }),
        500.0,
        200.0,
    );
    ui.advance(16).advance(16);
    // (500 - 9 gutter) * 0.3 = 147.3
    ui.expect("Side 147");
    // The gutter sits right after the first pane; drag it to x = 300.
    ui.drag_px((152.0, 100.0), (300.0, 100.0))
        .advance(16)
        .advance(16);
    let w = ui
        .texts()
        .iter()
        .find_map(|t| t.strip_prefix("Side ").and_then(|n| n.parse::<i32>().ok()))
        .unwrap();
    assert!((290..=300).contains(&w), "width after drag: {w}");
    ui.resize(800.0, 200.0).advance(16).advance(16);
    let w2 = ui
        .texts()
        .iter()
        .find_map(|t| t.strip_prefix("Side ").and_then(|n| n.parse::<i32>().ok()))
        .unwrap();
    assert!(w2 > w, "grows with the window: {w2} > {w}");
}

// ---------------------------------------------------------------- 20 carousel

#[derive(Default)]
struct Show {
    at: usize,
}

fn show_app(auto: bool) -> impl App {
    sketch(Show::default(), move |s: &Show| {
        let c = carousel([text("Slide A"), text("Slide B"), text("Slide C")])
            .height(120.0)
            .act(|s: &mut Show, i| s.at = i);
        col![
            text(format!("At {}", s.at)),
            if auto { c.auto(secs(2.0)) } else { c }
        ]
        .w(300.0)
    })
}

#[test]
fn carousel_arrows_and_dots_change_slide_with_a_crossfade() {
    let mut ui = Harness::new(show_app(false), 300.0, 200.0);
    ui.advance(16);
    ui.expect("Slide A").expect_no("Slide B").expect("At 0");
    // The next arrow is a 32px disc 8px in from the right edge, centred in the 120px box.
    let (nx, ny) = next_arrow(&ui);
    ui.click_at(nx, ny).advance(16);
    // Mid-fade both slides are drawn.
    ui.advance(60);
    ui.expect("Slide B").expect("At 1");
    ui.advance(600);
    ui.expect_no("Slide A").expect("Slide B");
    // Wraps past the end.
    ui.click_at(nx, ny)
        .advance(16)
        .click_at(nx, ny)
        .advance(16)
        .advance(600);
    ui.expect("Slide A").expect("At 0");
    // Dots jump straight to a slide.
    let (dx, dy) = dot(&ui, 2);
    ui.click_at(dx, dy).advance(16).advance(600);
    ui.expect("Slide C").expect("At 2");
}

/// Centres of the carousel's drawn quads that are `size` px square discs, left to right.
fn discs(ui: &Harness<impl App>, sizes: &[f32]) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = ui
        .draw_list()
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Quad { bounds, radius, .. }
                if sizes.contains(&bounds.size.width)
                    && bounds.size.width == bounds.size.height
                    && (*radius - bounds.size.width / 2.0).abs() < 0.01 =>
            {
                Some((
                    bounds.origin.x + bounds.size.width / 2.0,
                    bounds.origin.y + bounds.size.height / 2.0,
                ))
            }
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}
fn next_arrow(ui: &Harness<impl App>) -> (f32, f32) {
    *discs(ui, &[32.0]).last().expect("arrow drawn")
}
fn dot(ui: &Harness<impl App>, i: usize) -> (f32, f32) {
    discs(ui, &[6.0, 8.0])[i]
}

#[test]
fn carousel_auto_advances_and_pauses_under_the_pointer() {
    let mut ui = Harness::new(show_app(true), 300.0, 200.0);
    ui.advance(16).expect("Slide A");
    ui.advance(2100).advance(600);
    ui.expect("Slide B").expect("At 1");
    // Parked over it, nothing moves however long we wait.
    ui.move_to(150.0, 100.0).advance(16);
    for _ in 0..4 {
        ui.advance(2100);
    }
    ui.advance(600);
    ui.expect("Slide B").expect("At 1");
    // Pointer away: it runs again.
    ui.move_to(290.0, 5.0)
        .advance(16)
        .advance(2100)
        .advance(600);
    ui.expect("Slide C").expect("At 2");
}

#[test]
fn carousel_keeps_ticking_only_when_it_auto_advances() {
    let mut idle = Harness::new(show_app(false), 300.0, 200.0);
    idle.advance(16).advance(16);
    assert!(!idle.animating(), "a manual carousel is idle");
    let mut auto = Harness::new(show_app(true), 300.0, 200.0);
    auto.advance(16).advance(16);
    assert!(auto.animating(), "an auto carousel asks for frames");
}

// ---------------------------------------------------------------- 21 text weight and colour

#[test]
fn text_takes_weight_and_colour_together() {
    let green = Color::hex(0x2E9E5B);
    let ui = Harness::new(
        sketch((), move |_: &()| {
            col![
                text("Online").weight(600).color(green),
                text("Bold").bold(),
                heading("Title").weight(300).accent(),
            ]
        }),
        300.0,
        200.0,
    );
    let find = |s: &str| {
        ui.draw_list()
            .0
            .iter()
            .find_map(|c| match c {
                DrawCommand::Text {
                    content,
                    weight,
                    color,
                    ..
                } if content == s => Some((*weight, *color)),
                _ => None,
            })
            .unwrap()
    };
    let (w, c) = find("Online");
    assert_eq!((w, c), (600, green));
    assert_eq!(find("Bold").0, 700);
    assert_eq!(find("Title").0, 300);
}
