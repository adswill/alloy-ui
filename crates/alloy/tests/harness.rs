//! The testing helpers themselves: fill, expect, scroll and snapshots.
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct Form {
    name: String,
}

fn form() -> impl App {
    sketch(Form::default(), |s: &Form| {
        col![
            input(at!(s.name)).hint("Name"),
            text(format!("Hello {}", s.name)),
        ]
        .gap(8.0)
        .pad(16.0)
    })
}

#[test]
fn fill_replaces_contents() {
    let mut h = Harness::new(form(), 360.0, 160.0);
    h.fill("Name", "Ada").expect("Hello Ada");
    h.fill("Ada", "Grace")
        .expect("Hello Grace")
        .expect_no("Hello Ada");
}

#[test]
fn failures_list_the_screen() {
    let mut h = Harness::new(form(), 360.0, 160.0);
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        h.expect("Nope");
    }))
    .unwrap_err();
    let msg = err.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(msg.contains("Nope") && msg.contains("Hello"), "{msg}");
}

#[test]
fn texts_count_and_resize() {
    let mut h = Harness::new(form(), 360.0, 160.0);
    assert_eq!(h.count("Hello"), 1);
    assert!(h.texts().iter().any(|t| t == "Name"));
    h.resize(500.0, 200.0).expect("Name");
}

#[test]
fn snapshot_roundtrip() {
    // Write a snapshot to a scratch dir, then match it, then see a change fail.
    let dir = std::env::temp_dir().join(format!("alloy-snap-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let old = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let mut h = Harness::new(form(), 200.0, 100.0);
    std::env::set_var("UPDATE_GOLDEN", "1");
    h.snapshot("form");
    std::env::remove_var("UPDATE_GOLDEN");
    h.snapshot("form");
    h.fill("Name", "A very long name indeed");
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        h.snapshot("form");
    }));
    std::env::set_current_dir(old).unwrap();
    assert!(r.is_err(), "a changed screen must fail the snapshot");
    assert!(dir.join("tests/snapshots/form.png").exists());
}
