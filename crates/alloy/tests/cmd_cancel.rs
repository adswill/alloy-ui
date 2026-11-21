//! `Cmd::perform_keyed` and `Cmd::cancel`: a newer future replaces the running one, and
//! cancelled futures never deliver a message.
use alloy_ui::{prelude::*, testing::Harness};
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};

static STARTED: AtomicU32 = AtomicU32::new(0);
static FINISHED: AtomicU32 = AtomicU32::new(0);
/// The counters are shared, so the tests take turns.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Default)]
struct Search {
    shown: String,
}

#[derive(Clone)]
enum Msg {
    Type(&'static str),
    Found(String),
    Stop,
    Plain,
}

async fn lookup(q: &'static str) -> Msg {
    STARTED.fetch_add(1, SeqCst);
    sleep(secs(0.15)).await;
    FINISHED.fetch_add(1, SeqCst);
    Msg::Found(format!("found {q}"))
}

impl App for Search {
    type Msg = Msg;
    fn update_cmd(&mut self, m: Msg) -> Cmd<Msg> {
        match m {
            Msg::Type(q) => Cmd::perform_keyed("search", lookup(q)),
            Msg::Found(s) => {
                self.shown = s;
                Cmd::none()
            }
            Msg::Stop => Cmd::cancel("search"),
            Msg::Plain => Cmd::perform(lookup("plain")),
        }
    }
    fn view(&self) -> impl View<Msg> {
        col![
            button("a").on(Msg::Type("a")),
            button("ab").on(Msg::Type("ab")),
            button("stop").on(Msg::Stop),
            button("plain").on(Msg::Plain),
            text(self.shown.clone()),
        ]
    }
}

#[test]
fn newer_keyed_future_replaces_older() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = Harness::new(Search::default(), 300.0, 300.0);
    let (s0, f0) = (STARTED.load(SeqCst), FINISHED.load(SeqCst));
    h.click("a").click("ab").settle();
    h.expect("found ab").expect_no("found a");
    // The first one may be cancelled before its thread ever polls it.
    assert!(STARTED.load(SeqCst) - s0 >= 1);
    assert_eq!(
        FINISHED.load(SeqCst) - f0,
        1,
        "the first search was dropped mid-flight"
    );
}

#[test]
fn cancel_stops_the_running_future() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = Harness::new(Search::default(), 300.0, 300.0);
    let f0 = FINISHED.load(SeqCst);
    h.click("a").click("stop").settle();
    h.expect_no("found a");
    assert_eq!(FINISHED.load(SeqCst), f0);
    // The key is free again afterwards.
    h.click("ab").settle().expect("found ab");
}

#[test]
fn plain_perform_still_delivers() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = Harness::new(Search::default(), 300.0, 300.0);
    h.click("plain").settle().expect("found plain");
}
