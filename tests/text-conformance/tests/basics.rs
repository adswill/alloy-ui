//! Typing, selection, clipboard, undo, submit and read-only behaviour that a
//! text field must get right before any Unicode subtlety matters.
use alloy_ui_text_conformance::*;

#[test]
fn typing_inserts_text_and_caret_follows() {
    both(|k| {
        let mut f = Field::new(k);
        f.focus_end();
        f.type_str("hello");
        assert_eq!(f.value(), "hello");
        assert_eq!(f.caret_marked(), format!("hello{CARET}"));
    });
}

#[test]
fn key_press_alone_never_inserts_text() {
    // Platforms deliver text as TextInput; the key event is only a shortcut hook.
    both(|k| {
        let mut f = Field::new(k).with_text("");
        f.chord("a", NONE);
        f.chord("A", SHIFT);
        assert_eq!(f.value(), "");
    });
}

#[test]
fn text_is_ignored_while_unfocused() {
    both(|k| {
        let mut f = Field::new(k);
        f.type_str("nope");
        assert_eq!(f.value(), "");
        assert!(!f.focused());
    });
}

#[test]
fn caret_moves_and_inserts_in_the_middle() {
    both(|k| {
        let mut f = Field::new(k).with_text("ac");
        f.named("ArrowLeft", NONE);
        f.type_str("b");
        assert_eq!(f.value(), "abc");
        f.named("ArrowRight", NONE);
        f.named("ArrowRight", NONE); // already at the end: stays
        f.type_str("d");
        assert_eq!(f.value(), "abcd");
    });
}

#[test]
fn backspace_and_delete() {
    both(|k| {
        let mut f = Field::new(k).with_text("abcd");
        f.named("Backspace", NONE);
        assert_eq!(f.value(), "abc");
        f.named("ArrowLeft", NONE);
        f.named("Delete", NONE);
        assert_eq!(f.value(), "ab");
        f.named("Delete", NONE); // at the end: nothing
        assert_eq!(f.value(), "ab");
        let mut g = Field::new(k).with_text("x");
        g.named("Home", NONE);
        g.named("Backspace", NONE); // at the start: nothing
        assert_eq!(g.value(), "x");
    });
}

#[test]
fn shift_arrows_select_and_typing_replaces() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("hello world");
        for _ in 0..5 {
            f.named("ArrowLeft", SHIFT);
        }
        assert_eq!(f.selection(p).as_deref(), Some("world"));
        f.type_str("there");
        assert_eq!(f.value(), "hello there");
        assert_eq!(f.selection(p), None);
    });
}

#[test]
fn selection_collapses_to_the_matching_edge_on_plain_arrows() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abcdef");
        f.named("ArrowLeft", NONE);
        f.named("ArrowLeft", NONE);
        f.named("ArrowLeft", SHIFT);
        f.named("ArrowLeft", SHIFT); // selects "cd"
        assert_eq!(f.selection(p).as_deref(), Some("cd"));
        f.named("ArrowRight", NONE);
        assert_eq!(f.caret_marked(), format!("abcd{CARET}ef"));
        f.named("ArrowLeft", SHIFT);
        f.named("ArrowLeft", SHIFT);
        f.named("ArrowLeft", NONE);
        assert_eq!(f.caret_marked(), format!("ab{CARET}cdef"));
    });
}

#[test]
fn shift_arrow_back_over_the_anchor_shrinks_then_flips() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abcdef");
        f.named("ArrowLeft", NONE);
        f.named("ArrowLeft", NONE);
        f.named("ArrowRight", SHIFT);
        f.named("ArrowRight", SHIFT);
        assert_eq!(f.selection(p).as_deref(), Some("ef"));
        f.named("ArrowLeft", SHIFT);
        f.named("ArrowLeft", SHIFT);
        assert_eq!(f.selection(p), None);
        f.named("ArrowLeft", SHIFT);
        assert_eq!(f.selection(p).as_deref(), Some("d"));
    });
}

#[test]
fn select_all_copy_cut_paste_round_trip() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("clipboard text");
        f.chord("a", p.primary);
        assert_eq!(f.selection(p).as_deref(), Some("clipboard text"));
        f.chord("x", p.primary);
        assert_eq!(f.value(), "");
        assert_eq!(f.clipboard().as_deref(), Some("clipboard text"));
        f.chord("v", p.primary);
        f.chord("v", p.primary);
        assert_eq!(f.value(), "clipboard textclipboard text");
    });
}

#[test]
fn copy_without_selection_keeps_the_clipboard() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abc");
        f.set_clipboard("keep me");
        f.chord("c", p.primary);
        assert_eq!(f.clipboard().as_deref(), Some("keep me"));
        f.chord("x", p.primary);
        assert_eq!(f.value(), "abc", "cut with no selection must not delete");
        assert_eq!(f.clipboard().as_deref(), Some("keep me"));
    });
}

#[test]
fn paste_replaces_the_selection_and_places_caret_after() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("one two three");
        f.set_clipboard("2");
        for _ in 0..3 {
            f.named("ArrowLeft", NONE);
        }
        f.named("ArrowLeft", word_mods(p, true));
        f.chord("v", p.primary);
        assert_eq!(f.caret_marked(), format!("one two {}2{CARET}ree", ""));
    });
}

// word-wise shift selection helper local to this file
fn word_mods(p: Plat, shift: bool) -> alloy_ui_core::Modifiers {
    alloy_ui_core::Modifiers { shift, ..p.word }
}

#[test]
fn paste_with_empty_clipboard_does_nothing() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abc");
        f.chord("v", p.primary);
        assert_eq!(f.value(), "abc");
    });
}

#[test]
fn undo_collapses_a_typing_burst_and_redo_restores_it() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k);
        f.focus_end();
        f.type_str("hello");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "");
        f.chord(
            "z",
            alloy_ui_core::Modifiers {
                shift: true,
                ..p.primary
            },
        );
        assert_eq!(f.value(), "hello");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "");
    });
}

#[test]
fn ctrl_y_redoes_off_macos() {
    if cfg!(target_os = "macos") {
        return;
    }
    both(|k| {
        let mut f = Field::new(k);
        f.focus_end();
        f.type_str("abc");
        f.chord("z", CTRL);
        assert_eq!(f.value(), "");
        f.chord("y", CTRL);
        assert_eq!(f.value(), "abc");
    });
}

#[test]
fn undo_restores_the_caret_to_where_the_edit_began() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abcd");
        f.named("ArrowLeft", NONE);
        f.named("ArrowLeft", NONE);
        f.type_str("XY");
        assert_eq!(f.value(), "abXYcd");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "abcd");
        assert_eq!(f.caret_marked(), format!("ab{CARET}cd"));
    });
}

#[test]
fn typing_after_a_pause_starts_a_new_undo_step() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k);
        f.focus_end();
        f.tick(0);
        f.type_str("one");
        f.tick(5_000);
        f.type_str("two");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "one");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "");
    });
}

#[test]
fn undo_with_empty_history_is_harmless() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("seeded");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "seeded");
    });
}

#[test]
fn a_paste_and_a_cut_are_each_undoable() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abc");
        f.chord("a", p.primary);
        f.chord("x", p.primary);
        assert_eq!(f.value(), "");
        f.chord("v", p.primary);
        assert_eq!(f.value(), "abc");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "abc");
    });
}

#[test]
fn read_only_fields_copy_but_do_not_edit() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::read_only(k);
        f.set_value("fixed");
        f.focus_end();
        f.type_str("x");
        f.named("Backspace", NONE);
        f.chord("v", p.primary);
        f.chord("x", p.primary);
        f.chord("z", p.primary);
        assert_eq!(f.value(), "fixed");
        assert_eq!(f.edits(), 1, "only the seeding dispatch");
        f.chord("a", p.primary);
        f.chord("c", p.primary);
        assert_eq!(f.clipboard().as_deref(), Some("fixed"));
    });
}

#[test]
fn escape_releases_focus_and_tab_is_left_to_navigation() {
    both(|k| {
        let mut f = Field::new(k).with_text("x");
        f.named("Tab", NONE);
        assert_eq!(f.value(), "x", "Tab must not be inserted");
        f.send(alloy_ui_core::Event::KeyPressed(
            alloy_ui_core::Key::Escape,
            NONE,
        ));
        assert!(!f.focused());
        f.type_str("y");
        assert_eq!(f.value(), "x");
    });
}

#[test]
fn enter_submits_in_the_input_and_breaks_the_line_in_the_editor() {
    let mut i = Field::new(Kind::Input).with_text("go");
    i.send(alloy_ui_core::Event::KeyPressed(
        alloy_ui_core::Key::Enter,
        NONE,
    ));
    assert_eq!((i.submits(), i.value()), (1, "go"));
    let mut e = Field::new(Kind::Editor).with_text("go");
    e.send(alloy_ui_core::Event::KeyPressed(
        alloy_ui_core::Key::Enter,
        NONE,
    ));
    assert_eq!(e.value(), "go\n");
}

#[test]
fn control_characters_are_never_inserted() {
    both(|k| {
        let mut f = Field::new(k).with_text("");
        f.send(alloy_ui_core::Event::TextInput(
            "a\u{0}b\u{7}c\u{1b}d\u{7f}".into(),
        ));
        assert_eq!(f.value(), "abcd");
    });
}

#[test]
fn paste_normalises_line_breaks_per_field_kind() {
    let p = Plat::native();
    let mut i = Field::new(Kind::Input).with_text("");
    i.set_clipboard("one\r\ntwo\tthree\nfour");
    i.chord("v", p.primary);
    assert!(
        !i.value().chars().any(char::is_control),
        "single-line paste kept control characters: {:?}",
        i.value()
    );
    let mut e = Field::new(Kind::Editor).with_text("");
    e.set_clipboard("one\r\ntwo\rthree");
    e.chord("v", p.primary);
    assert!(
        !e.value().contains('\r'),
        "editor kept a carriage return: {:?}",
        e.value()
    );
}

#[test]
fn mouse_click_places_the_caret_between_characters() {
    both(|k| {
        let mut f = Field::new(k).with_text("abcdef");
        let x = f.x_after("abc");
        f.click(x + 0.5, 8.0);
        assert_eq!(f.caret_marked(), format!("abc{CARET}def"));
    });
}

#[test]
fn mouse_drag_selects_a_range() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abcdef");
        let (a, b) = (f.x_after("ab"), f.x_after("abcde"));
        f.drag((a, 8.0), (b, 8.0));
        assert_eq!(f.selection(p).as_deref(), Some("cde"));
    });
}

#[test]
fn clicking_outside_the_field_drops_focus() {
    // The editor fills the window, so only the single-line input has "outside".
    let mut f = Field::new(Kind::Input).with_text("x");
    f.click(200.0, 150.0);
    assert!(!f.focused());
}
