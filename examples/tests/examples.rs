//! Each example is driven through the real runtime with real input, checked by
//! its on-screen text, and compared with a committed golden image.
//!
//! Text is shaped and rasterised with the bundled IBM Plex fonts, so layout is the same
//! on every OS; only glyph anti-aliasing may differ slightly, hence the tolerance (loose
//! enough for that, tight enough to catch any moved or recoloured widget). Regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p alloy-ui-examples`.

use alloy_ui::{prelude::App, testing::Harness};
use alloy_ui_examples::{counter, dashboard, settings, text_editor, todo};
use alloy_ui_render::testing::{assert_golden, Tolerance};
use alloy_ui_render::Image;
use std::path::Path;

const TOL: Tolerance = Tolerance {
    channel: 40,
    bad_fraction: 0.015,
};

fn open(app: impl App, size: (f32, f32)) -> Harness<impl App> {
    Harness::new(app, size.0, size.1)
}

fn golden<A: App>(name: &str, h: &Harness<A>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let img = Image::decode_png(&h.png()).expect("decode frame");
    assert_golden(&dir, name, &img, TOL);
}

#[test]
fn counter_counts() {
    let mut h = open(counter::app(), counter::SIZE);
    assert!(h.has_text("0 so far"));
    h.click("Add").click("Add").click("Subtract");
    assert!(h.has_text("1 so far"));
    golden("counter_one", &h);
    h.click("Reset");
    assert!(h.has_text("0 so far"));
}

#[test]
fn todo_add_toggle_filter() {
    let mut h = open(todo::app(), todo::SIZE);
    assert!(h.has_text("3 open"));
    h.click("Book dentist");
    assert!(h.has_text("2 open"));
    h.click("Open");
    assert!(
        !h.has_text("Book dentist"),
        "done tasks leave the Open filter"
    );
    assert!(h.has_text("Write release notes for 0.4"));
    golden("todo_open", &h);
    h.click("Done").click("Clear done").click("All");
    assert!(!h.has_text("Book dentist"));
    assert!(!h.has_text("Order replacement fan"));
}

#[test]
fn todo_input_adds_a_task() {
    let mut h = open(todo::app(), todo::SIZE);
    h.click("What needs doing?").type_text("Water the plants");
    h.click("Add task");
    assert!(h.has_text("Water the plants"));
    assert!(h.has_text("4 open"));
}

#[test]
fn editor_shows_and_clears() {
    let mut h = open(text_editor::app(), text_editor::SIZE);
    assert!(h.has_text("7 lines"));
    golden("text_editor", &h);
    h.click("Clear");
    assert!(h.has_text("0 lines"));
    assert!(h.has_text("Start typing"));
}

#[test]
fn settings_navigates_and_toggles() {
    let mut h = open(settings::app(), settings::SIZE);
    assert!(h.has_text("Text size"));
    golden("settings_appearance", &h);
    h.click("Dark");
    golden("settings_dark", &h);
    h.click("General");
    assert!(h.has_text("Launch at login"));
    h.click("Launch at login");
    golden("settings_general", &h);
    h.click("Privacy");
    assert!(h.has_text("Send crash reports"));
}

#[test]
fn dashboard_switches_range() {
    let mut h = open(dashboard::app(), dashboard::SIZE);
    assert!(h.has_text("Peak 81k per interval"));
    assert!(h.has_text("export-ledger"));
    golden("dashboard_hours", &h);
    h.click("12 days");
    assert!(h.has_text("Peak 83k per interval"));
    assert!(h.has_text("Nov 9"));
    golden("dashboard_days", &h);
}

#[test]
fn idle_key_does_not_change_the_frame() {
    let mut h = open(counter::app(), counter::SIZE);
    let before = h.png();
    h.press(alloy_ui::Key::Escape);
    assert_eq!(before, h.png(), "a no-op key must not change pixels");
}
