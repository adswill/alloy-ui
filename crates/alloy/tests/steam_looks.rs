//! Section F: Steam-style looks.
use alloy_ui::{prelude::*, testing::Harness};

#[test]
fn counter_with_suffix_is_one_string() {
    let app = sketch(0, |_: &i32| counter(3).suffix(" of 8"));
    let mut ui = Harness::new(app, 200.0, 60.0);
    ui.expect("3 of 8");
}

#[test]
fn counter_groups_and_prefixes_in_one_string() {
    let app = sketch(0, |_: &i32| counter(12345).prefix("$"));
    let mut ui = Harness::new(app, 200.0, 60.0);
    ui.expect("$12,345");
}

#[test]
fn square_avatar_with_in_game_presence_renders() {
    let app = sketch(0, |_: &i32| {
        row![
            avatar("Mira Okafor")
                .square()
                .size(40.0)
                .presence(Presence::InGame),
            text("hi"),
        ]
    });
    let mut ui = Harness::new(app, 200.0, 60.0);
    ui.expect("hi").expect("MO");
}
