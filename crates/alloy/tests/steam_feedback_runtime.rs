//! Runtime and Harness items of the Steam Oxide feedback.
use alloy_ui::{prelude::*, testing::Harness};

/// An `App` written by hand: nothing from `Sketch` is wired up.
struct ByHand;

impl App for ByHand {
    type Msg = ();
    fn view(&self) -> impl View<()> {
        let r = resource_for(7u32, |k| async move { Ok::<_, String>(k + 1) });
        let body: Node<()> = match r.get() {
            Some(v) => text(format!("loaded {v}")).into(),
            None => text("Loading").into(),
        };
        col![body]
    }
}

#[test]
fn hand_written_app_loads_a_resource() {
    let mut h = Harness::new(ByHand, 300.0, 120.0);
    h.expect("Loading");
    h.settle().expect("loaded 8");
}

#[test]
fn pump_like_runtime_applies_resources_on_the_timer() {
    let mut h = Harness::new(ByHand, 300.0, 120.0);
    h.pump_like_runtime(true);
    h.expect("Loading");
    h.settle().expect("loaded 8");
}

#[test]
fn toast_in_a_dropped_harness_does_not_panic_the_next_one() {
    let toaster = || {
        sketch((), |_: &()| {
            button("Toast").act(|_: &mut ()| {
                toast("Hello");
            })
        })
    };
    {
        let mut h = Harness::new(toaster(), 300.0, 120.0);
        h.advance(9100).click("Toast").advance(1000);
    }
    let mut h = Harness::new(toaster(), 300.0, 120.0);
    h.advance(16).advance(16).expect_no("Hello");
}

#[test]
fn settle_can_ignore_a_never_ending_command() {
    #[derive(Clone)]
    enum Msg {
        Start,
    }
    struct Forever;
    impl App for Forever {
        type Msg = Msg;
        fn update_cmd(&mut self, _: Msg) -> Cmd<Msg> {
            Cmd::perform_keyed("delivery", async {
                loop {
                    sleep(secs(0.05)).await;
                }
            })
        }
        fn view(&self) -> impl View<Msg> {
            button("Go").on(Msg::Start)
        }
    }
    let mut h = Harness::new(Forever, 200.0, 100.0);
    h.click("Go");
    let t = std::time::Instant::now();
    h.settle_ignoring(&["delivery"]);
    assert!(t.elapsed().as_secs_f32() < 2.0, "did not wait for it");
}

#[test]
fn scroll_sign_negative_shows_content_below() {
    let app = sketch((), |_: &()| {
        scroll(col((0..60).map(|i| text(format!("line {i}")))).gap(4.0)).h(200.0)
    });
    let mut h = Harness::new(app, 300.0, 300.0);
    let y = |h: &Harness<_>| h.position("line 0").unwrap().y;
    let start = y(&h);
    h.scroll("line 0", -100.0);
    assert!(y(&h) < start, "negative dy moved content up");
    h.scroll_here(100.0);
    assert_eq!(y(&h), start, "positive dy moved it back down");
}

#[test]
fn harness_reports_its_build_phases() {
    let mut h = Harness::new(ByHand, 300.0, 120.0);
    let names: Vec<_> = h.phases().iter().map(|p| p.0).collect();
    assert_eq!(names, ["harness-ui-setup", "harness-first-view"]);
    let n = h.perf_mark("noop", |h| h.phases().len());
    assert_eq!(n, 2);
    assert_eq!(h.phases().last().unwrap().0, "noop");
}
