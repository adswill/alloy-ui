//! Multi-line behaviour of `TextEditor`.
use alloy_ui_core::{Event, Key};
use alloy_ui_text_conformance::*;

fn ed(text: &str) -> Field {
    Field::new(Kind::Editor).with_text(text)
}
fn enter(f: &mut Field) {
    f.send(Event::KeyPressed(Key::Enter, NONE));
}

#[test]
fn enter_splits_and_backspace_joins_lines() {
    let mut f = ed("ab");
    f.named("ArrowLeft", NONE);
    enter(&mut f);
    assert_eq!(f.value(), "a\nb");
    f.named("Backspace", NONE);
    assert_eq!(f.value(), "ab");
    assert_eq!(f.caret_marked(), format!("a{CARET}b"));
}

#[test]
fn up_down_keep_the_goal_column_across_short_lines() {
    // The caret probe is itself an edit and resets the goal column, so each
    // step replays the whole key sequence on a fresh field and probes once.
    let text = "abcdef\nxy\nabcdef";
    let run = |keys: &[&str]| {
        let mut f = ed(text);
        for k in keys {
            f.named(k, NONE);
        }
        f.caret_marked()
    };
    assert_eq!(
        run(&["ArrowUp", "ArrowUp"]),
        format!("abcdef{CARET}\nxy\nabcdef")
    );
    assert_eq!(
        run(&["ArrowUp", "ArrowUp", "ArrowDown"]),
        format!("abcdef\nxy{CARET}\nabcdef")
    );
    assert_eq!(
        run(&["ArrowUp", "ArrowUp", "ArrowDown", "ArrowDown"]),
        format!("abcdef\nxy\nabcdef{CARET}")
    );
}

#[test]
fn up_on_the_first_line_and_down_on_the_last_are_clamped() {
    let mut f = ed("one\ntwo");
    f.named("ArrowDown", NONE);
    assert_eq!(f.caret_marked(), format!("one\ntwo{CARET}"));
    f.named("ArrowUp", NONE);
    f.named("ArrowUp", NONE);
    f.named("ArrowUp", NONE);
    assert_eq!(f.caret_marked(), format!("one{CARET}\ntwo"));
}

#[test]
fn home_and_end_are_per_line() {
    let mut f = ed("first\nsecond");
    f.named("Home", NONE);
    assert_eq!(f.caret_marked(), format!("first\n{CARET}second"));
    f.named("End", NONE);
    assert_eq!(f.caret_marked(), format!("first\nsecond{CARET}"));
    f.named("ArrowUp", NONE);
    f.named("Home", NONE);
    assert_eq!(f.caret_marked(), format!("{CARET}first\nsecond"));
}

#[test]
fn document_start_and_end_shortcuts() {
    let p = Plat::native();
    let mut f = ed("a\nb\nc");
    if p.mac {
        f.named("ArrowUp", CMD);
    } else {
        f.named("Home", CTRL);
    }
    assert_eq!(f.caret_marked(), format!("{CARET}a\nb\nc"));
    if p.mac {
        f.named("ArrowDown", CMD);
    } else {
        f.named("End", CTRL);
    }
    assert_eq!(f.caret_marked(), format!("a\nb\nc{CARET}"));
}

#[test]
fn shift_down_selects_across_lines_and_copy_keeps_newlines() {
    let p = Plat::native();
    let mut f = ed("one\ntwo\nthree");
    f.named("Home", CTRL); // doc start on non-mac; harmless extra on mac
    f.named("ArrowUp", NONE);
    f.named("Home", NONE);
    f.named("ArrowDown", SHIFT);
    f.named("ArrowDown", SHIFT);
    f.chord("c", p.primary);
    let got = f.clipboard().unwrap();
    assert!(got.contains('\n') && got.starts_with("one"), "{got:?}");
}

#[test]
fn pasted_crlf_becomes_a_single_newline() {
    let p = Plat::native();
    let mut f = ed("");
    f.set_clipboard("a\r\nb\nc");
    f.chord("v", p.primary);
    assert_eq!(f.value(), "a\nb\nc");
}

#[test]
fn pasted_lone_cr_becomes_a_newline() {
    let p = Plat::native();
    let mut f = ed("");
    f.set_clipboard("a\rb");
    f.chord("v", p.primary);
    assert_eq!(f.value(), "a\nb");
}

#[test]
fn page_keys_and_wheel_never_panic_on_short_text() {
    let mut f = ed("one\ntwo");
    for _ in 0..3 {
        f.named("PageDown", NONE);
        f.named("PageUp", NONE);
        f.send(Event::Scrolled {
            dx: 0.0,
            dy: -500.0,
        });
        f.send(Event::Scrolled { dx: 0.0, dy: 500.0 });
    }
    assert_eq!(f.value(), "one\ntwo");
}

#[test]
fn many_blank_lines_and_trailing_newline_are_stable() {
    let mut f = ed("\n\n\n");
    f.named("ArrowUp", NONE);
    f.named("ArrowUp", NONE);
    f.type_str("x");
    assert_eq!(f.value(), "\nx\n\n");
    f.named("ArrowDown", NONE);
    f.named("ArrowDown", NONE);
    f.named("ArrowDown", NONE);
    f.type_str("y");
    assert_eq!(f.value(), "\nx\n\ny");
}

#[test]
fn ime_commit_with_newline_inside_an_editor() {
    let mut f = ed("");
    f.preedit("ni");
    f.commit("你\n好");
    assert_eq!(f.value(), "你\n好");
}

#[test]
fn undo_of_enter_restores_the_line() {
    let p = Plat::native();
    let mut f = ed("abc");
    f.named("ArrowLeft", NONE);
    enter(&mut f);
    f.chord("z", p.primary);
    assert_eq!(f.value(), "abc");
}

#[test]
fn double_click_selects_a_word_and_triple_click_a_line() {
    let p = Plat::native();
    let mut f = ed("alpha beta\ngamma");
    let x = f.x_after("alpha b");
    f.click(x, 10.0);
    f.rapid_click(x, 10.0);
    assert_eq!(f.selection(p).as_deref(), Some("beta"));
    f.rapid_click(x, 10.0);
    assert_eq!(f.selection(p).as_deref(), Some("alpha beta"));
}
