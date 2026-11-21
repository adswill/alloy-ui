//! `resource_for`: a changed key cancels the running fetch; stale results never show.
use alloy_ui::{prelude::*, testing::Harness};
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};

static STARTED: AtomicU32 = AtomicU32::new(0);
static FINISHED: AtomicU32 = AtomicU32::new(0);

async fn search(q: String) -> Result<String, String> {
    STARTED.fetch_add(1, SeqCst);
    // The first part is the debounce: a newer key cancels us before any "work" happens.
    sleep(secs(0.15)).await;
    FINISHED.fetch_add(1, SeqCst);
    Ok(format!("found {q}"))
}

#[test]
fn newer_key_cancels_older_fetch() {
    let app = sketch(String::new(), |q: &String| {
        let r = resource_for(q.clone(), search);
        col![
            button("a").act(|q: &mut String| *q = "a".into()),
            button("ab").act(|q: &mut String| *q = "ab".into()),
            r.show(|d| text(d.clone()).into()),
        ]
    });
    let mut h = Harness::new(app, 400.0, 300.0);
    h.settle().expect_containing("found"); // the empty first key
    let (s0, f0) = (STARTED.load(SeqCst), FINISHED.load(SeqCst));
    h.click("a");
    // The fetch for "a" runs on a worker thread; let it really start before "ab" cancels it,
    // or a slow thread start (seen on Windows runners) means it never ran at all.
    let t = std::time::Instant::now();
    while STARTED.load(SeqCst) == s0 && t.elapsed() < std::time::Duration::from_secs(5) {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    h.click("ab").wait_for("found ab");
    h.expect_no("found a");
    // Two keys started a fetch each, but only the last one ran to completion.
    assert_eq!(STARTED.load(SeqCst) - s0, 2);
    assert_eq!(FINISHED.load(SeqCst) - f0, 1);
}

#[test]
fn cancel_keeps_old_data() {
    let app = sketch((), |_| {
        let r = resource(|| async {
            sleep(secs(0.1)).await;
            Ok::<_, String>("done".to_string())
        });
        let stop = r.clone();
        col![
            button("Stop").act(move |_| stop.cancel()),
            r.show(|d| text(d.clone()).into()),
        ]
    });
    let mut h = Harness::new(app, 400.0, 300.0);
    h.click("Stop").settle();
    h.expect("Loading…"); // cancelled before any data arrived, so nothing to show
}
