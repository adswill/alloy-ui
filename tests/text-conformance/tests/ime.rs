//! Input-method behaviour: preedit/commit sequences as winit delivers them,
//! dead keys, and what the field must do with keys while composing.
use alloy_ui_core::Event;
use alloy_ui_text_conformance::*;

fn focused(k: Kind) -> Field {
    Field::new(k).with_text("")
}

#[test]
fn preedit_never_touches_the_value() {
    both(|k| {
        let mut f = focused(k);
        f.type_str("ab");
        let before = f.edits();
        for s in ["n", "ni", "ni h", "你"] {
            f.preedit(s);
            assert_eq!(f.value(), "ab");
        }
        assert_eq!(f.edits(), before, "preedit must not emit edits");
    });
}

#[test]
fn pinyin_sequence_commits_once_at_the_caret() {
    both(|k| {
        let mut f = focused(k);
        f.type_str("a");
        for s in ["n", "ni", "ni h", "ni hao"] {
            f.preedit(s);
        }
        f.commit("你好");
        assert_eq!(f.value(), "a你好");
        assert_eq!(f.caret_marked(), format!("a你好{CARET}"));
        f.type_str("b");
        assert_eq!(f.value(), "a你好b");
    });
}

#[test]
fn japanese_conversion_with_candidate_change() {
    both(|k| {
        let mut f = focused(k);
        for s in ["に", "にほ", "にほん", "日本", "二本", "日本"] {
            f.preedit(s);
        }
        f.commit("日本");
        for s in ["ご", "ご"] {
            f.preedit(s);
        }
        f.commit("語");
        assert_eq!(f.value(), "日本語");
    });
}

#[test]
fn hangul_syllable_building_with_continuous_composition() {
    // macOS-style: each syllable is committed while the next jamo is already in preedit.
    both(|k| {
        let mut f = focused(k);
        f.preedit("ㅎ");
        f.preedit("하");
        f.preedit("한");
        f.send(Event::TextInput("한".into()));
        f.preedit("ㄱ");
        f.preedit("그");
        f.preedit("글");
        f.commit("글");
        assert_eq!(f.value(), "한글");
    });
}

#[test]
fn commit_order_either_way_leaves_no_stale_preedit() {
    both(|k| {
        let mut f = focused(k);
        f.preedit("ka");
        f.send(Event::TextInput("か".into())); // text first...
        f.send(Event::ImePreedit(String::new())); // ...then the clear
        f.type_str("x");
        assert_eq!(f.value(), "かx");
        assert_eq!(f.caret_marked(), format!("かx{CARET}"));
    });
}

#[test]
fn cancelled_composition_leaves_value_unchanged() {
    both(|k| {
        let mut f = focused(k);
        f.type_str("ok");
        f.preedit("ni");
        f.preedit("");
        assert_eq!(f.value(), "ok");
        assert_eq!(f.edits(), 3); // seed + two typed characters, nothing from the IME
    });
}

#[test]
fn commit_replaces_the_selection() {
    both(|k| {
        let mut f = Field::new(k).with_text("hello world");
        for _ in 0..5 {
            f.named("ArrowLeft", SHIFT);
        }
        f.preedit("shi");
        f.commit("世界");
        assert_eq!(f.value(), "hello 世界");
    });
}

#[test]
fn commit_in_the_middle_keeps_the_tail() {
    both(|k| {
        let mut f = Field::new(k).with_text("ac");
        f.named("ArrowLeft", NONE);
        f.preedit("b");
        f.commit("乙");
        assert_eq!(f.value(), "a乙c");
        assert_eq!(f.caret_marked(), format!("a乙{CARET}c"));
    });
}

#[test]
fn preedit_and_commit_for_an_unfocused_field_are_ignored() {
    both(|k| {
        let mut f = Field::new(k);
        f.preedit("ni");
        f.commit("你");
        assert_eq!(f.value(), "");
    });
}

#[test]
fn a_click_cancels_the_composition_and_text_goes_to_the_new_caret() {
    both(|k| {
        let mut f = Field::new(k).with_text("abcd");
        f.preedit("ni");
        let x = f.x_after("ab");
        f.click(x, 8.0);
        f.commit("你");
        assert_eq!(f.value(), "ab你cd");
    });
}

#[test]
fn undo_removes_a_whole_ime_commit_in_one_step() {
    both(|k| {
        let p = Plat::native();
        let mut f = focused(k);
        f.preedit("nihao");
        f.commit("你好");
        f.chord("z", p.primary);
        assert_eq!(f.value(), "");
    });
}

#[test]
fn emoji_picker_commit_inserts_a_whole_cluster() {
    both(|k| {
        let mut f = focused(k);
        f.commit(samples::FAMILY);
        f.type_str("!");
        assert_eq!(f.value(), format!("{}!", samples::FAMILY));
    });
}

#[test]
fn dead_key_then_vowel_inserts_one_composed_character() {
    // Platform sequence on a US-International layout: dead key, then 'e' yields "é".
    both(|k| {
        let mut f = focused(k);
        f.send(Event::KeyPressed(named("Dead"), NONE));
        f.send(Event::KeyReleased(named("Dead"), NONE));
        assert_eq!(f.value(), "", "a dead key alone inserts nothing");
        f.send(Event::KeyPressed(ch("e"), NONE));
        f.send(Event::TextInput("é".into()));
        f.send(Event::KeyReleased(ch("e"), NONE));
        assert_eq!(f.value(), "é");
    });
}

#[test]
fn dead_key_with_a_non_combining_follow_up_inserts_both() {
    both(|k| {
        let mut f = focused(k);
        f.send(Event::KeyPressed(named("Dead"), NONE));
        f.send(Event::KeyPressed(ch("x"), NONE));
        f.send(Event::TextInput("´x".into()));
        assert_eq!(f.value(), "´x");
    });
}

#[test]
fn dead_key_preedit_variant_shows_then_commits() {
    // macOS reports the pending accent as a preedit.
    both(|k| {
        let mut f = focused(k);
        f.preedit("´");
        assert_eq!(f.value(), "");
        f.commit("é");
        assert_eq!(f.value(), "é");
    });
}

#[test]
fn option_composed_characters_are_plain_text() {
    // macOS Option+e then e, or AltGr combinations: only TextInput carries them.
    both(|k| {
        let mut f = focused(k);
        f.send(Event::KeyPressed(ch("e"), ALT));
        f.send(Event::TextInput("€".into()));
        assert_eq!(f.value(), "€");
    });
}

#[test]
#[ignore = "BUG(widgets/platform): Backspace while composing deletes committed text instead of editing the preedit"]
fn backspace_during_preedit_does_not_delete_committed_text() {
    // Some platforms (X11/XIM, Windows TSF "Process" keys) still deliver the key.
    both(|k| {
        let mut f = focused(k);
        f.type_str("keep");
        f.preedit("ni");
        f.named("Backspace", NONE);
        assert_eq!(f.value(), "keep");
    });
}

#[test]
#[ignore = "BUG(widgets/platform): arrow keys while composing move the caret under the preedit"]
fn arrows_during_preedit_do_not_move_the_caret() {
    both(|k| {
        let mut f = focused(k);
        f.type_str("ab");
        f.preedit("ni");
        f.named("ArrowLeft", NONE);
        f.commit("你");
        assert_eq!(f.value(), "ab你");
    });
}

#[test]
#[ignore = "BUG(widgets/platform): Enter while composing submits the form (it should confirm the candidate)"]
fn enter_during_preedit_does_not_submit() {
    let mut f = focused(Kind::Input);
    f.preedit("ni");
    f.send(Event::KeyPressed(alloy_ui_core::Key::Enter, NONE));
    assert_eq!(f.submits(), 0);
}

#[test]
#[ignore = "BUG(widgets/platform): shortcuts while composing act on the field (Cmd/Ctrl+A with a pending preedit)"]
fn select_all_during_preedit_is_deferred_to_the_ime() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("abc");
        f.preedit("ni");
        f.chord("a", p.primary);
        f.commit("你");
        assert_eq!(f.value(), "abc你");
    });
}
