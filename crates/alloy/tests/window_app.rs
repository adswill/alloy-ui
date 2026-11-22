//! Section A: close hook and focus signal on `sketch`, through the headless harness.
use alloy_ui::{app_focused, prelude::*, testing::Harness};

#[derive(Default)]
struct S {
    hidden: bool,
    focus_events: u32,
    unsaved: bool,
}

#[test]
fn on_close_requested_can_cancel_the_close() {
    let app =
        sketch(S::default(), |_: &S| text("running")).on_close_requested(|cx| cx.cancel_close());
    let mut ui = Harness::new(app, 200.0, 100.0);
    assert!(!ui.close(), "the hook kept the window open");
    ui.expect("running");
}

#[test]
fn doing_nothing_lets_it_close_and_state_decides() {
    let app = sketch(S::default(), |s: &S| {
        text(if s.hidden { "hidden" } else { "shown" })
    })
    .on_close_requested(|cx| {
        if cx.state().unsaved {
            cx.cancel_close_then(|s| s.hidden = true);
        }
    });
    let mut ui = Harness::new(app, 200.0, 100.0);
    assert!(ui.close(), "nothing unsaved: closes");
    let app = sketch(
        S {
            unsaved: true,
            ..S::default()
        },
        |s: &S| text(if s.hidden { "hidden" } else { "shown" }),
    )
    .on_close_requested(|cx| {
        if cx.state().unsaved {
            cx.cancel_close_then(|s| s.hidden = true);
        }
    });
    let mut ui = Harness::new(app, 200.0, 100.0);
    assert!(!ui.close());
    ui.expect("hidden");
}

#[test]
fn app_focused_reaches_the_view_and_the_event_reaches_the_handler() {
    let app = sketch(S::default(), |s: &S| {
        col![
            text(if app_focused() { "front" } else { "background" }),
            text(format!("events {}", s.focus_events)),
        ]
    })
    .on_focus_changed(|s, _| s.focus_events += 1);
    let mut ui = Harness::new(app, 200.0, 100.0);
    ui.expect("front")
        .focus(false)
        .expect("background")
        .expect("events 1");
    ui.focus(true).expect("front").expect("events 2");
}
