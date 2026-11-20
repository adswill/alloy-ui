//! `resource`: loading, error with retry, success.
use alloy_ui::{prelude::*, testing::Harness};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

async fn flaky() -> Result<String, String> {
    static N: AtomicU32 = AtomicU32::new(0);
    sleep(secs(0.05)).await;
    match N.fetch_add(1, Relaxed) {
        0 => Err("boom".into()),
        n => Ok(format!("ready {n}")),
    }
}

#[test]
fn loading_then_error_then_retry_then_data() {
    let app = app("t").page("P", || resource(flaky).show(|s| text(s.clone()).into()));
    let mut h = Harness::new(app, 600.0, 400.0);
    assert!(h.has_text("Loading…"));
    h.settle();
    assert!(h.has_text("Could not load") && h.has_text("boom"));
    h.click("Retry").settle();
    assert!(h.has_text("ready 1"), "retry fetched again");
}

#[test]
fn mock_fails_on_schedule_and_series_is_stable() {
    #[derive(alloy_ui::Record, Clone, Default)]
    struct Row {
        name: String,
    }
    let m = Row::mock([alloy_ui::vals!["a"], alloy_ui::vals!["b"]]).fail_every(2, "boom");
    assert_eq!(alloy_ui::block_on(m.load()).unwrap().len(), 2);
    assert_eq!(alloy_ui::block_on(m.load()).unwrap_err(), "boom");
    assert_eq!(alloy_ui::series("x", 4), alloy_ui::series("x", 4));
}
