//! Steam feedback, area O: toast placement, modal height, reopen and default action, portal
//! Escape, and `Btn::confirm` Escape.
use alloy_ui::{prelude::*, testing::Harness};
use alloy_ui_core::DrawCommand;

#[derive(Default)]
struct S {
    open: bool,
    done: u32,
    cleared: u32,
    escaped: u32,
    n: u32,
}

fn scrim_alpha(ui: &Harness<impl App>) -> f32 {
    ui.draw_list()
        .0
        .iter()
        .find_map(|c| match c {
            DrawCommand::Quad { fill, bounds, .. }
                if bounds.size.width >= 399.0 && fill.r == 0.0 && fill.a > 0.0 =>
            {
                Some(fill.a)
            }
            _ => None,
        })
        .unwrap_or(0.0)
}

fn toasts_app(inset: Option<f32>, max: Option<usize>) -> impl App {
    let mut sk = sketch(S::default(), |_: &S| {
        col![button("Go").act(|s: &mut S| {
            s.n += 1;
            toast(format!("Toast {}", s.n));
        })]
    });
    if let Some(i) = inset {
        sk = sk.toast_inset(i);
    }
    if let Some(m) = max {
        sk = sk.toast_max(m);
    }
    sk
}

#[test]
fn toast_inset_lifts_the_stack_off_the_bottom() {
    let mut a = Harness::new(toasts_app(None, None), 400.0, 300.0);
    a.click("Go").advance(10).advance(300);
    let low = a.position("Toast 1").expect("toast shown").y;
    let mut b = Harness::new(toasts_app(Some(60.0), None), 400.0, 300.0);
    b.click("Go").advance(10).advance(300);
    let high = b.position("Toast 1").expect("toast shown").y;
    assert!(
        (low - high - 36.0).abs() < 1.5,
        "inset 60 vs 24 moves it 36 px: {low} vs {high}"
    );
}

#[test]
fn toast_max_caps_the_visible_stack() {
    let mut ui = Harness::new(toasts_app(None, Some(2)), 400.0, 300.0);
    for _ in 0..3 {
        ui.click("Go");
    }
    ui.advance(10)
        .advance(300)
        .expect("Toast 3")
        .expect("Toast 2")
        .expect_no("Toast 1");
}

fn dialog(s: &S) -> impl View<Act<S>> {
    col![
        button("Open").act(|s: &mut S| s.open = true),
        modal(
            s.open,
            col![
                heading("Tall"),
                scroll(col((0..60).map(|i| text(format!("row {i}"))))).fill_h(),
                button("Done").act(|s: &mut S| s.done += 1),
            ]
        )
        .max_height(240.0)
        .width(300.0)
        .close(|s: &mut S| s.open = false)
        .default_action(|s: &mut S| {
            s.done += 100;
            s.open = false;
        }),
    ]
}

#[test]
fn modal_max_height_squeezes_content_and_enter_runs_default_action() {
    let mut ui = Harness::new(sketch(S::default(), dialog), 400.0, 700.0);
    ui.click("Open").settle();
    let done = ui.position("Done").expect("footer visible").y;
    assert!(
        done < 350.0 + 120.0 + 2.0,
        "card stays near 240 px tall: {done}"
    );
    ui.key("enter").settle().expect_no("Tall");
    ui.click("Open").settle().expect("Tall");
}

#[test]
fn modal_reopen_restarts_the_ease_even_when_built_only_while_open() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                button("Open").act(|s: &mut S| s.open = true),
                if s.open {
                    Some(modal(true, text("Body")).close(|s: &mut S| s.open = false))
                } else {
                    None
                },
            ]
        }),
        400.0,
        300.0,
    );
    for _ in 0..2 {
        ui.click("Open");
        let first = scrim_alpha(&ui);
        assert!(first < 0.44, "scrim starts faded, got {first}");
        ui.settle();
        ui.key("esc").settle();
    }
}

#[test]
fn modal_can_keep_its_content_while_it_leaves() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                button("Open").act(|s: &mut S| s.open = true),
                modal(s.open, if s.open { text("Body") } else { text("") })
                    .keep_content_on_exit()
                    .close(|s: &mut S| s.open = false),
            ]
        }),
        400.0,
        300.0,
    );
    ui.click("Open").advance(10).advance(400).expect("Body");
    ui.key("esc").advance(10).advance(60).expect("Body");
    ui.advance(600).expect_no("Body");
}

#[test]
fn portal_closes_on_escape() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                button("Open").act(|s: &mut S| s.open = true),
                portal(s.open, text("Lightbox").fill()).close(|s: &mut S| s.open = false),
            ]
        }),
        400.0,
        300.0,
    );
    ui.click("Open").expect("Lightbox");
    ui.key("esc").expect_no("Lightbox");
}

#[test]
fn confirm_cancels_on_escape() {
    let mut ui = Harness::new(
        sketch(S::default(), |s: &S| {
            col![
                button("Clear")
                    .act(|s: &mut S| s.cleared += 1)
                    .on_escape(Act::new(|s: &mut S| s.escaped += 1))
                    .confirm("Sure?"),
                text(format!("cleared {} escaped {}", s.cleared, s.escaped)),
            ]
        }),
        400.0,
        200.0,
    );
    ui.click("Clear").expect("Sure?");
    ui.key("esc")
        .expect_no("Sure?")
        .expect("cleared 0 escaped 1");
}
