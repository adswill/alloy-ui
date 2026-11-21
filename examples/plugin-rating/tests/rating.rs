//! Tests written the way any plugin author would: against the public `alloy-ui` API only.

use alloy_ui::plugin;
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;
use alloy_ui::widgets::Ui;
use alloy_ui::{DrawCommand, Event, Layout, MouseButton, Point, Rectangle, Size, Widget};
use alloy_ui_rating::{rating, Rating, RatingPlugin};

fn layout() -> Layout {
    let r = Rating::<u8>::new(&Ui::new(alloy_ui::theme::Theme::light()), 1, 0);
    let size = r.measure(Size::new(500.0, 100.0));
    Layout {
        bounds: Rectangle::new(Point::new(10.0, 10.0), size),
        children: vec![],
    }
}

fn click(w: &mut Rating<u8>, l: &Layout, p: Point) -> Vec<u8> {
    let mut out = vec![];
    w.on_event(&Event::CursorMoved(p), l, p, &mut out);
    w.on_event(&Event::MousePressed(MouseButton::Left), l, p, &mut out);
    w.on_event(&Event::MouseReleased(MouseButton::Left), l, p, &mut out);
    out
}

#[test]
fn clicking_a_star_reports_that_rating() {
    let ui = Ui::new(alloy_ui::theme::Theme::light());
    let l = layout();
    let mut w = Rating::new(&ui, 1, 2).on_change(|n| n);
    // Stars are 26 px wide with 4 px gaps, starting at x = 10: the fourth is at 100..126.
    assert_eq!(click(&mut w, &l, Point::new(112.0, 20.0)), vec![4]);
    // The gap between stars hits nothing.
    assert!(click(&mut w, &l, Point::new(37.0, 20.0)).is_empty());
}

#[test]
fn clearable_toggles_the_current_rating_off() {
    let ui = Ui::new(alloy_ui::theme::Theme::light());
    let l = layout();
    let mut w = Rating::new(&ui, 1, 3).clearable().on_change(|n| n);
    assert_eq!(
        click(&mut w, &l, Point::new(70.0, 20.0)),
        vec![0],
        "3rd star again clears"
    );
    assert_eq!(click(&mut w, &l, Point::new(100.0, 20.0)), vec![4]);
}

#[test]
fn read_only_without_a_handler() {
    let ui = Ui::new(alloy_ui::theme::Theme::light());
    let l = layout();
    let mut w = Rating::<u8>::new(&ui, 1, 3);
    assert!(click(&mut w, &l, Point::new(70.0, 20.0)).is_empty());
}

#[derive(Default)]
struct Review {
    stars: u8,
}

fn app() -> impl alloy_ui::App {
    sketch(Review::default(), |r| {
        col![
            format!("stars={}", r.stars),
            rating(r.stars).act(|r: &mut Review, n| r.stars = n),
        ]
        .pad(16.0)
    })
}

fn filled(h: &Harness<impl alloy_ui::App>) -> usize {
    // A star is lit when it is drawn in the accent colour; count the ones that differ from the rest.
    let stars: Vec<_> = h
        .draw_list()
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text { content, color, .. } if content == "\u{2605}" => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(stars.len(), 5);
    stars.iter().filter(|c| **c == stars[0]).count()
}

#[test]
fn works_inside_a_sketch_app_with_keyboard() {
    let mut h = Harness::new(app(), 300.0, 120.0);
    assert!(h.has_text("stars=0"));
    h.key("Tab"); // focus the rating
    h.key("ArrowRight").key("ArrowRight").key("ArrowRight");
    assert!(h.has_text("stars=3"), "three presses of ArrowRight");
    assert_eq!(filled(&h), 3);
    h.key("ArrowLeft");
    assert!(h.has_text("stars=2"));
    h.key("End");
    assert!(h.has_text("stars=5"));
    h.key("Home");
    assert!(h.has_text("stars=0"));
}

#[test]
fn installing_the_plugin_registers_strings_once() {
    plugin::install(&RatingPlugin);
    plugin::install(&RatingPlugin);
    let n = plugin::installed()
        .iter()
        .filter(|(n, _)| n == "rating")
        .count();
    assert_eq!(n, 1);
    assert_eq!(
        alloy_ui::i18n::translate("rating.value", &[("n", "4".into())]),
        "4 of 5 stars"
    );
}
