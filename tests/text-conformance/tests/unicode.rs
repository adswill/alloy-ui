//! Grapheme clusters, emoji, combining marks and RTL text.
//! Passing tests pin what works; `#[ignore = "BUG..."]` tests state the
//! correct behaviour and currently fail (the widgets index by `char`).
use alloy_ui_text_conformance::{samples::*, *};

#[test]
fn clusters_survive_typing_selection_copy_and_paste_intact() {
    both(|k| {
        let p = Plat::native();
        let mut c = Cases::new();
        for (name, s) in CLUSTERS {
            let mut f = Field::new(k).with_text("");
            f.type_str(s);
            c.check(
                &format!("{name}: typed"),
                f.value().to_owned(),
                s.to_owned(),
            );
            f.chord("a", p.primary);
            f.chord("c", p.primary);
            c.check(
                &format!("{name}: copied"),
                f.clipboard(),
                Some(s.to_owned()),
            );
            f.named("ArrowRight", NONE);
            f.chord("v", p.primary);
            let want = format!("{s}{s}");
            c.check(&format!("{name}: pasted"), f.value().to_owned(), want);
        }
        c.finish();
    });
}

#[test]
fn text_around_clusters_is_never_corrupted_by_edits() {
    both(|k| {
        let mut f = Field::new(k).with_text(&format!("a{FAMILY}b{FLAG}c"));
        f.type_str("!");
        f.named("Home", NONE);
        f.type_str("^");
        assert_eq!(f.value(), format!("^a{FAMILY}b{FLAG}c!"));
    });
}

#[test]
fn mouse_clicks_on_wide_clusters_never_panic() {
    both(|k| {
        let mut f =
            Field::new(k).with_text(&format!("{FAMILY}{FLAG}{THUMB_TONE}{E_ACUTE_COMBINING}"));
        for x in (0..400).step_by(3) {
            f.click(x as f32, 8.0);
            f.drag((x as f32, 8.0), (400.0 - x as f32, 8.0));
        }
    });
}

#[test]
#[ignore = "BUG(widgets): caret indexes chars, so Backspace removes one code point of a cluster"]
fn backspace_removes_one_whole_grapheme() {
    both(|k| {
        let mut c = Cases::new();
        for (name, s) in CLUSTERS {
            let mut f = Field::new(k).with_text(&format!("x{s}"));
            f.named("Backspace", NONE);
            c.check(name, f.value().to_owned(), "x".to_owned());
        }
        c.finish();
    });
}

#[test]
#[ignore = "BUG(widgets): forward Delete removes one code point of a cluster"]
fn delete_removes_one_whole_grapheme() {
    both(|k| {
        let mut c = Cases::new();
        for (name, s) in CLUSTERS {
            let mut f = Field::new(k).with_text(&format!("{s}x"));
            f.named("Home", NONE);
            f.named("Delete", NONE);
            c.check(name, f.value().to_owned(), "x".to_owned());
        }
        c.finish();
    });
}

#[test]
#[ignore = "BUG(widgets): ArrowLeft/Right step by code point, landing inside clusters"]
fn arrow_keys_move_one_grapheme() {
    both(|k| {
        let mut c = Cases::new();
        for (name, s) in CLUSTERS {
            let mut f = Field::new(k).with_text(&format!("a{s}b"));
            f.named("ArrowLeft", NONE);
            f.named("ArrowLeft", NONE);
            c.check(
                &format!("{name}: left x2"),
                f.caret_marked(),
                format!("a{CARET}{s}b"),
            );
            f.named("ArrowRight", NONE);
            c.check(
                &format!("{name}: right"),
                f.caret_marked(),
                format!("a{s}{CARET}b"),
            );
        }
        c.finish();
    });
}

#[test]
#[ignore = "BUG(widgets): Shift+Arrow selects half a cluster"]
fn shift_arrow_selects_whole_graphemes() {
    both(|k| {
        let p = Plat::native();
        let mut c = Cases::new();
        for (name, s) in CLUSTERS {
            let mut f = Field::new(k).with_text(&format!("a{s}"));
            f.named("ArrowLeft", SHIFT);
            c.check(name, f.selection(p), Some(s.to_owned()));
        }
        c.finish();
    });
}

#[test]
#[ignore = "BUG(widgets): mouse hit-testing can put the caret inside a cluster"]
fn clicking_never_places_the_caret_inside_a_cluster() {
    both(|k| {
        let mut c = Cases::new();
        for (name, s) in CLUSTERS {
            let text = format!("a{s}b");
            let mut f = Field::new(k).with_text(&text);
            let (x0, x1) = (f.x_after("a"), f.x_after(&format!("a{s}")));
            // Sweep across the cluster; every caret must sit on a grapheme boundary.
            let n = 12;
            for i in 0..=n {
                let x = x0 + (x1 - x0) * i as f32 / n as f32;
                f.click(x, 8.0);
                let marked = f.caret_marked();
                let plain = marked.replace(CARET, "");
                let at = marked.find(CARET).unwrap();
                let on_boundary = plain
                    .grapheme_indices(true)
                    .map(|(i, _)| i)
                    .chain([plain.len()])
                    .any(|b| b == at);
                if !on_boundary {
                    c.check(
                        &format!("{name} @ {i}/{n}"),
                        marked,
                        "caret on a boundary".into(),
                    );
                    break;
                }
            }
        }
        c.finish();
    });
}

#[test]
fn rtl_text_types_and_copies_in_logical_order() {
    both(|k| {
        let p = Plat::native();
        let hebrew = "שלום";
        let arabic = "مرحبا";
        let mut f = Field::new(k).with_text("");
        f.type_str(&format!("abc {hebrew} def {arabic}"));
        assert_eq!(f.value(), format!("abc {hebrew} def {arabic}"));
        f.named("Backspace", NONE);
        assert_eq!(f.value(), format!("abc {hebrew} def {}", "مرحب"));
        f.chord("a", p.primary);
        assert_eq!(f.selection(p).unwrap(), f.value());
    });
}

#[test]
fn rtl_text_does_not_break_word_navigation_bounds() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("שלום עולם");
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("שלום {CARET}עולם"));
        f.named("ArrowLeft", p.word);
        assert_eq!(f.caret_marked(), format!("{CARET}שלום עולם"));
    });
}

#[test]
#[ignore = "BUG(widgets): no bidi support, ArrowLeft always means logically backward"]
fn arrow_left_in_an_rtl_run_moves_visually_left() {
    // In a Hebrew run the visually-left neighbour is the *next* character.
    both(|k| {
        let mut f = Field::new(k).with_text("שלום");
        f.named("Home", NONE); // logical start = visual right edge
        f.named("ArrowLeft", NONE);
        assert_eq!(f.caret_marked(), format!("ש{CARET}לום"));
    });
}

#[test]
fn zero_width_and_bidi_marks_are_ordinary_text() {
    both(|k| {
        let s = "a\u{200b}b\u{200e}c\u{202e}d\u{2066}e\u{feff}";
        let mut f = Field::new(k).with_text("");
        f.type_str(s);
        assert_eq!(f.value(), s);
    });
}

#[test]
fn nfc_and_nfd_forms_are_kept_as_typed() {
    both(|k| {
        let mut f = Field::new(k).with_text("");
        f.type_str("\u{e9}");
        f.type_str(E_ACUTE_COMBINING);
        assert_eq!(f.value(), format!("\u{e9}{E_ACUTE_COMBINING}"));
    });
}

#[test]
fn astral_plane_text_round_trips_and_deletes_per_scalar_at_least() {
    both(|k| {
        let mut f = Field::new(k).with_text(&format!("a{}b", SIMPLE_EMOJI));
        f.named("ArrowLeft", NONE);
        f.named("Backspace", NONE);
        assert_eq!(f.value(), "ab", "a single-scalar emoji is one grapheme");
        let mut g = Field::new(k).with_text("𝒳𝒴");
        g.named("Backspace", NONE);
        assert_eq!(g.value(), "𝒳");
    });
}
