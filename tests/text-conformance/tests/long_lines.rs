//! Very long lines and big documents: no panics, no quadratic blow-ups.
//! Time limits are generous and measured around one interaction, so only
//! complexity bugs trip them. On CI (shared, debug-build runners, up to 4x slower
//! than a dev machine) every limit is multiplied by `CI_HEADROOM`.
use alloy_ui_text_conformance::*;
use std::time::{Duration, Instant};

const CI_HEADROOM: u32 = 4;

fn time(what: &str, limit: Duration, f: impl FnOnce()) {
    let limit = if std::env::var_os("CI").is_some() {
        limit * CI_HEADROOM
    } else {
        limit
    };
    let t = Instant::now();
    f();
    let d = t.elapsed();
    assert!(d < limit, "{what} took {d:?} (limit {limit:?})");
}

#[test]
fn a_20k_char_line_supports_every_navigation_without_panic() {
    both(|k| {
        let p = Plat::native();
        let line = "word ".repeat(4_000);
        let mut f = Field::new(k).with_text(&line);
        f.named("Home", NONE);
        f.named("End", NONE);
        f.named("ArrowLeft", p.word);
        f.named("ArrowRight", p.word);
        f.named("ArrowLeft", shift(p.word));
        f.chord("a", p.primary);
        f.chord("c", p.primary);
        assert_eq!(f.clipboard().as_deref(), Some(line.as_str()));
        f.named("Backspace", NONE);
        assert_eq!(f.value(), "");
        f.chord("v", p.primary);
        assert_eq!(f.value(), line);
        f.chord("z", p.primary);
        assert_eq!(f.value(), "");
    });
}

#[test]
fn typing_at_the_end_of_a_long_line_stays_fast() {
    both(|k| {
        let mut f = Field::new(k).with_text(&"x".repeat(20_000));
        time(
            "100 keystrokes at the end of 20k chars",
            Duration::from_secs(10),
            || {
                f.type_str(&"y".repeat(100));
            },
        );
        assert_eq!(f.value().len(), 20_100);
    });
}

#[test]
fn ime_preedit_updates_on_a_long_line_stay_fast() {
    both(|k| {
        let mut f = Field::new(k).with_text(&"字".repeat(10_000));
        time("50 preedit updates", Duration::from_secs(10), || {
            for i in 0..50 {
                f.preedit(&"あ".repeat(i % 8));
            }
        });
    });
}

#[test]
fn pasting_one_megabyte_does_not_hang() {
    both(|k| {
        let p = Plat::native();
        let mut f = Field::new(k).with_text("");
        f.set_clipboard(&"lorem ipsum ".repeat(87_000)); // ~1 MB
        time("paste 1 MB", Duration::from_secs(20), || {
            f.chord("v", p.primary)
        });
        assert_eq!(f.value().len(), 12 * 87_000);
    });
}

#[test]
fn clicking_in_a_long_line_is_not_quadratic() {
    both(|k| {
        let mut f = Field::new(k).with_text(&"abcdefghij".repeat(1_500)); // 15k chars
        time(
            "one click and one drag on a 15k-char line",
            Duration::from_millis(500),
            || {
                f.click(200.0, 8.0);
                f.drag((20.0, 8.0), (380.0, 8.0));
            },
        );
    });
}

#[test]
fn a_5k_line_document_edits_without_panic() {
    let p = Plat::native();
    let doc = (0..5_000)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut f = Field::new(Kind::Editor).with_text(&doc);
    f.named("ArrowUp", NONE);
    f.named("PageUp", NONE);
    f.type_str("!");
    f.chord("a", p.primary);
    f.chord("c", p.primary);
    assert_eq!(f.clipboard().unwrap().matches('\n').count(), 4_999);
}
