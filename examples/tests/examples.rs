//! Each example is driven through the real runtime with real input events.
use alloy_ui_core::Key;
use alloy_ui_examples::apps::{counter, dashboard, settings, text_editor, todo};
use alloy_ui_examples::run::{apply, click_text, render_demo, runtime, Step};

#[test]
fn counter_click_path() {
    let mut rt = runtime(
        counter::Counter::new(alloy_ui_theme::Theme::light()),
        (360.0, 180.0),
    );
    assert!(click_text(&mut rt, "Add"));
    assert!(click_text(&mut rt, "Add"));
    assert!(click_text(&mut rt, "Subtract"));
    assert_eq!(rt.app().value, 1);
    assert!(click_text(&mut rt, "Reset"));
    assert_eq!(rt.app().value, 0);
}

#[test]
fn todo_add_toggle_filter() {
    let (rt, _) = render_demo(todo::demo());
    let a = rt.app();
    assert_eq!(a.tasks.len(), 4);
    assert_eq!(a.tasks[3].title, "Water the plants");
    assert!(a.tasks[2].done, "Book dentist ticked");
    assert_eq!(a.filter, todo::Filter::Open);
    assert!(a.draft.is_empty());
}

#[test]
fn editor_types_breaks_lines_and_moves() {
    let (rt, _) = render_demo(text_editor::demo());
    let a = rt.app();
    assert_eq!(a.lines[2], "Typed text lands at the caret.");
    assert_eq!(a.lines[3], "Second lin");
    assert_eq!(a.caret.0, 2);
}

#[test]
fn editor_backspace_joins_lines() {
    let mut rt = runtime(
        text_editor::TextEditor::new(alloy_ui_theme::Theme::light()),
        (400.0, 200.0),
    );
    apply(&mut rt, &Step::Press(Key::Backspace));
    assert_eq!(rt.app().caret, (1, 20));
    assert_eq!(rt.app().lines.len(), 2);
}

#[test]
fn settings_switches_section_and_toggles() {
    let (rt, _) = render_demo(settings::demo());
    let a = rt.app();
    assert_eq!(a.section, settings::Section::General);
    assert!(a.launch_at_login);
    assert_eq!(a.mode, alloy_ui_theme::ThemeMode::Dark);
}

#[test]
fn dashboard_range_switch_and_render() {
    let (rt, img) = render_demo(dashboard::demo());
    assert_eq!(rt.app().range, dashboard::Range::Days);
    assert_eq!((img.width, img.height), (1040, 800));
}

#[test]
fn idle_events_do_not_redraw() {
    let mut rt = runtime(
        counter::Counter::new(alloy_ui_theme::Theme::light()),
        (360.0, 180.0),
    );
    let o = rt.handle_event(&alloy_ui_core::Event::CursorMoved(
        alloy_ui_core::Point::new(1.0, 1.0),
    ));
    assert!(!o.redraw);
}
