//! Section G: view-registered timers, richer Harness input, motion control, window material.
use alloy_ui::{prelude::*, testing::Harness, DrawCommand, WindowEffect};

#[derive(Default)]
struct Feed {
    live: bool,
    n: u32,
    page: usize,
    clicks: u32,
}

fn feed_view(s: &Feed) -> impl View<Act<Feed>> {
    if s.live {
        // A page asks for its own timer; it stops when the page is no longer built.
        tick_while_shown(secs(1.0), |s: &mut Feed| s.n += 1);
    }
    col![
        text(format!("ticks {}", s.n)),
        button("Go live").act(|s: &mut Feed| s.live = true),
        button("Stop").act(|s: &mut Feed| s.live = false),
    ]
}

#[test]
fn a_view_can_start_and_stop_its_own_timer() {
    let mut ui = Harness::new(sketch(Feed::default(), feed_view), 300.0, 200.0);
    ui.advance(3000).expect("ticks 0");
    ui.click("Go live").advance(3000).expect("ticks 3");
    ui.click("Stop").advance(3000).expect("ticks 3");
    // Coming back starts a fresh timer.
    ui.click("Go live").advance(1000).expect("ticks 4");
}

#[derive(Default)]
struct Poll {
    on: bool,
    polled: u32,
}

#[derive(Clone)]
enum Msg {
    Poll,
}

impl App for Poll {
    type Msg = Msg;
    fn update(&mut self, _: Msg) {
        self.polled += 1;
    }
    fn view(&self) -> impl View<Msg> {
        if self.on {
            subscribe(every(secs(2.0), || Msg::Poll));
        }
        text(format!("polled {}", self.polled))
    }
}

#[test]
fn app_trait_views_subscribe_too() {
    let mut ui = Harness::new(
        Poll {
            on: true,
            polled: 0,
        },
        200.0,
        100.0,
    );
    ui.advance(2000).advance(2000).expect("polled 2");
}

#[test]
fn sketch_tick_runs_in_the_harness() {
    let app = sketch(0u32, |n: &u32| text(format!("n {n}"))).tick(secs(0.5), |n| *n += 1);
    let mut ui = Harness::new(app, 200.0, 100.0);
    ui.advance(1500).expect("n 3");
}

#[test]
fn right_and_middle_clicks_by_position() {
    let app = sketch(Feed::default(), |s: &Feed| {
        col![
            context_menu(
                text("Target"),
                [menu_item("Copy").act(|s: &mut Feed| s.clicks += 10)],
            ),
            button("Add").act(|s: &mut Feed| s.clicks += 1),
            text(format!("clicks {}", s.clicks)),
        ]
    });
    let mut ui = Harness::new(app, 300.0, 200.0);
    let p = ui.position("Target").unwrap();
    ui.right_click_at(p.x + 3.0, p.y + 3.0).expect("Copy");
    ui.key("Escape").expect_no("Copy");
    // A middle click is not a left click.
    ui.middle_click("Add").expect("clicks 0");
    let q = ui.position("Add").unwrap();
    ui.middle_click_at(q.x + 3.0, q.y + 3.0).expect("clicks 0");
    ui.click("Add").expect("clicks 1");
}

fn pages(s: &Feed) -> impl View<Act<Feed>> {
    col![
        button("Next").act(|s: &mut Feed| s.page = 1),
        route(s.page, text(format!("page {}", s.page))),
    ]
}

#[test]
fn settle_motion_lets_a_route_transition_finish() {
    let mut ui = Harness::new(sketch(Feed::default(), pages), 300.0, 200.0);
    ui.click("Next").advance(16);
    assert!(ui.animating(), "the transition is running");
    ui.settle_motion();
    assert!(!ui.animating());
    ui.expect("page 1").expect_no("page 0");
}

#[test]
fn reduced_motion_switches_instantly() {
    let mut ui = Harness::new(sketch(Feed::default(), pages), 300.0, 200.0);
    ui.reduce_motion(true);
    ui.click("Next").advance(16);
    assert!(!ui.animating(), "no transition under reduced motion");
    ui.expect("page 1").expect_no("page 0");
}

/// The smallest alpha among filled boxes that are smaller than the window.
fn region_alpha(ui: &Harness<impl App>) -> f32 {
    ui.draw_list()
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Quad { bounds, fill, .. } if bounds.size.width < 250.0 => Some(fill.a),
            _ => None,
        })
        .fold(1.0, f32::min)
}

#[test]
fn surface_regions_show_the_window_material() {
    let app = sketch((), |_: &()| text("Sidebar").pad(12.0).surface().w(120.0));
    let mut ui = Harness::new(app, 400.0, 200.0);
    assert_eq!(region_alpha(&ui), 1.0, "opaque without an effect");
    ui.window_effect(WindowEffect::Blur);
    assert!(region_alpha(&ui) < 0.8, "translucent under Blur");
    ui.window_effect(WindowEffect::None);
    assert_eq!(region_alpha(&ui), 1.0);
}
