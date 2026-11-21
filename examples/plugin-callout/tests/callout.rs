//! Tests through the public `alloy-ui` API only, as a plugin author would write them.

use alloy_ui::plugin;
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;
use alloy_ui::widgets::{Button, Ui};
use alloy_ui::{Element, Event, MouseButton, Point, Rectangle, Size, Tone, Widget};
use alloy_ui_callout::{callout, Callout, CalloutPlugin};

#[test]
fn child_events_pass_through_the_container() {
    let ui = Ui::new(alloy_ui::theme::Theme::light());
    let btn = Button::new(&ui, "Go").on_press(7u8).into_element();
    let mut c = Callout::new(&ui, Tone::Accent, btn);
    let size = c.measure(Size::new(400.0, 200.0));
    let l = c.arrange(Rectangle::new(
        Point::new(0.0, 0.0),
        Size::new(300.0, size.height),
    ));
    let p = Point::new(
        l.children[0].bounds.origin.x + 4.0,
        l.children[0].bounds.origin.y + 4.0,
    );
    let mut out = vec![];
    c.on_event(&Event::CursorMoved(p), &l, p, &mut out);
    c.on_event(&Event::MousePressed(MouseButton::Left), &l, p, &mut out);
    c.on_event(&Event::MouseReleased(MouseButton::Left), &l, p, &mut out);
    assert_eq!(out, vec![7]);
}

#[test]
fn child_is_inset_past_the_bar() {
    let ui = Ui::new(alloy_ui::theme::Theme::light());
    let c = Callout::<u8>::new(&ui, Tone::Warning, Element::new(Button::new(&ui, "x")));
    let l = c.arrange(Rectangle::new(
        Point::new(10.0, 10.0),
        Size::new(300.0, 80.0),
    ));
    assert!(
        l.children[0].bounds.origin.x >= 10.0 + 4.0 + 8.0,
        "{:?}",
        l.children[0].bounds
    );
}

#[test]
fn installs_translations_once() {
    plugin::install(&CalloutPlugin);
    plugin::install(&CalloutPlugin);
    assert!(plugin::is_installed("callout"));
    assert_eq!(
        plugin::installed()
            .iter()
            .filter(|(n, _)| n == "callout")
            .count(),
        1
    );
    assert_eq!(alloy_ui::i18n::translate("callout.warning", &[]), "Warning");
}

#[derive(Default)]
struct App {
    pressed: u32,
}

#[test]
fn works_in_a_sketch_app() {
    let mut h = Harness::new(
        sketch(App::default(), |s| {
            col![callout(Tone::Success, "Saved").content(col![
                format!("pressed {}", s.pressed),
                button("More").act(|s: &mut App| s.pressed += 1),
            ]),]
        }),
        400.0,
        200.0,
    );
    h.click("More");
    h.click("More");
    assert!(h.has_text("pressed 2"));
}
