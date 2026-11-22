//! Hover colours on boxes: `.hover_bg(colour)` fades in under the pointer and out again.
use alloy_ui::{prelude::*, testing::Harness};
use alloy_ui_core::{Color, DrawCommand};

/// Alpha of the most opaque quad with the given RGB, or 0 if there is none.
fn alpha_of(ui: &Harness<impl App>, rgb: Color) -> f32 {
    let same = |c: &Color| {
        (c.r - rgb.r).abs() < 0.02 && (c.g - rgb.g).abs() < 0.02 && (c.b - rgb.b).abs() < 0.02
    };
    ui.draw_list()
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Quad { fill, .. } if same(fill) => Some(fill.a),
            _ => None,
        })
        .fold(0.0, f32::max)
}

#[test]
fn a_plain_box_fades_its_hover_colour_in_and_out() {
    let hot = Color::hex(0x2E9E5B);
    let mut ui = Harness::new(
        sketch((), move |_| text("Row").pad(12.0).hover_bg(hot)),
        240.0,
        100.0,
    );
    ui.advance(16).advance(400);
    assert_eq!(alpha_of(&ui, hot), 0.0, "invisible until hovered");
    ui.hover("Row").advance(16).advance(400);
    assert!(alpha_of(&ui, hot) > 0.95, "fully shown while hovered");
    ui.move_to(230.0, 95.0).advance(16).advance(400);
    assert_eq!(
        alpha_of(&ui, hot),
        0.0,
        "gone again after the pointer leaves"
    );
}

#[test]
fn a_box_with_a_background_mixes_towards_the_hover_colour() {
    let (base, hot) = (Color::hex(0x202020), Color::hex(0xE0E0E0));
    let mut ui = Harness::new(
        sketch((), move |_| text("Card").pad(12.0).bg(base).hover_bg(hot)),
        240.0,
        100.0,
    );
    ui.advance(16).advance(400);
    assert!(alpha_of(&ui, base) > 0.99);
    ui.hover("Card").advance(16).advance(400);
    assert!(alpha_of(&ui, hot) > 0.99 && alpha_of(&ui, base) == 0.0);
}
