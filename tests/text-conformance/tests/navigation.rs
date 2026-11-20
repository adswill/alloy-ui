//! Word and line navigation with the platform's own modifiers
//! (Option/Cmd on macOS, Ctrl elsewhere).
use alloy_ui_core::Modifiers;
use alloy_ui_text_conformance::*;

fn with(m: Modifiers, shift: bool) -> Modifiers {
    Modifiers { shift, ..m }
}

/// Jump to the start of the line the way this platform does it.
fn line_start(f: &mut Field, p: Plat, shift: bool) {
    if p.mac {
        f.named("ArrowLeft", with(CMD, shift));
    } else {
        f.named("Home", with(NONE, shift));
    }
}
fn line_end(f: &mut Field, p: Plat, shift: bool) {
    if p.mac {
        f.named("ArrowRight", with(CMD, shift));
    } else {
        f.named("End", with(NONE, shift));
    }
}

#[test]
fn word_left_and_right_with_native_modifier() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("alpha beta  gamma");
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("alpha beta  {CARET}gamma"));
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("alpha {CARET}beta  gamma"));
        f.named("ArrowRight", p.word);
        assert_eq!(f.caret_marked(), format!("alpha beta{CARET}  gamma"));
        f.named("ArrowRight", p.word);
        f.named("ArrowRight", p.word);
        assert_eq!(f.caret_marked(), format!("alpha beta  gamma{CARET}"));
    });
}

#[test]
fn shift_word_selects_whole_words() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("one two three");
        f.named("ArrowLeft", with(p.word, true));
        f.named("ArrowLeft", with(p.word, true));
        assert_eq!(f.selection(p).as_deref(), Some("two three"));
    });
}

#[test]
fn word_delete_backward_and_forward() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("one two three");
        f.named("Backspace", p.word);
        assert_eq!(f.value(), "one two ");
        f.named("Backspace", p.word);
        assert_eq!(f.value(), "one ");
        f.named("Home", NONE);
        f.named("Delete", p.word);
        assert_eq!(f.value(), " ");
    });
}

#[test]
fn word_moves_skip_punctuation_runs() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("foo, (bar) -- baz");
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("foo, (bar) -- {CARET}baz"));
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("foo, ({CARET}bar) -- baz"));
    });
}

#[test]
fn word_moves_over_digits_underscores_and_accents() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("naïve café 42nd");
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("naïve café {CARET}42nd"));
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("naïve {CARET}café 42nd"));
    });
}

#[test]
#[ignore = "BUG(widgets): apostrophes split words; UAX #29 keeps \"don't\" as one word"]
fn word_navigation_follows_uax29_for_apostrophes_and_underscores() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("we don't know");
        f.named("ArrowLeft", p.word);
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("we {CARET}don't know"));
    });
}

#[test]
#[ignore = "BUG(widgets): CJK runs are one \"word\"; platforms break at dictionary words"]
fn word_navigation_in_cjk_stops_at_more_than_the_whole_run() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("今日は天気がいい");
        f.named("Home", NONE);
        f.named("ArrowRight", p.word);
        assert_ne!(f.caret_marked(), format!("今日は天気がいい{CARET}"));
    });
}

#[test]
fn line_start_and_end_with_native_shortcut() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("hello world");
        line_start(&mut f, p, false);
        assert_eq!(f.caret_marked(), format!("{CARET}hello world"));
        line_end(&mut f, p, false);
        assert_eq!(f.caret_marked(), format!("hello world{CARET}"));
    });
}

#[test]
fn shift_line_selects_to_the_edge() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("hello world");
        f.named("ArrowLeft", NONE);
        line_start(&mut f, p, true);
        assert_eq!(f.selection(p).as_deref(), Some("hello worl"));
        line_end(&mut f, p, true);
        assert_eq!(f.selection(p).as_deref(), Some("d"));
    });
}

#[test]
fn home_and_end_keys_work_on_every_platform() {
    // Mac keyboards without Home/End still send them from external keyboards.
    both(|k| {
        let mut f = Field::new(k).with_text("abc");
        f.named("Home", NONE);
        assert_eq!(f.caret_marked(), format!("{CARET}abc"));
        f.named("End", NONE);
        assert_eq!(f.caret_marked(), format!("abc{CARET}"));
    });
}

#[test]
fn both_platform_conventions_for_shortcuts_are_accepted() {
    // The widgets do not know the OS, so Cmd and Ctrl both act as the primary key.
    both(|k| {
        for primary in [CMD, CTRL] {
            let mut f = Field::new(k).with_text("abc");
            f.chord("a", primary);
            f.chord("c", primary);
            assert_eq!(f.clipboard().as_deref(), Some("abc"));
        }
    });
}

#[test]
fn cmd_arrow_goes_to_the_line_edge_not_the_word_edge_on_mac() {
    both(|k| {
        let mut f = Field::new(k).with_text("one two");
        f.named("ArrowLeft", CMD);
        assert_eq!(f.caret_marked(), format!("{CARET}one two"));
    });
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "BUG(widgets): Ctrl+A selects all on macOS; Cocoa text fields use it for 'start of line'"]
fn mac_ctrl_a_is_not_select_all() {
    both(|k| {
        let mut f = Field::new(k).with_text("abc");
        f.chord("a", CTRL);
        assert_eq!(f.selection(Plat::MAC), None);
    });
}

#[cfg(not(target_os = "macos"))]
#[test]
#[ignore = "BUG(widgets): the Windows/Super key acts as the shortcut key (Super+A selects all); Super is the OS's"]
fn super_key_shortcuts_are_not_text_shortcuts_off_macos() {
    both(|k| {
        let mut f = Field::new(k).with_text("abc");
        f.chord("a", CMD);
        assert_eq!(f.selection(Plat::OTHER), None);
    });
}

#[cfg(not(target_os = "macos"))]
#[test]
#[ignore = "BUG(widgets): Alt+Arrow moves by word off macOS; on Windows/Linux Alt+Left is 'back'"]
fn alt_arrow_is_not_word_movement_off_macos() {
    both(|k| {
        let mut f = Field::new(k).with_text("one two");
        f.named("ArrowLeft", ALT);
        assert_eq!(f.caret_marked(), format!("one tw{CARET}o"));
    });
}
