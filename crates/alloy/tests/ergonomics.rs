//! The short-form API: `at!` bindings, `tick_while`, `wizard`, `stepper`, table helpers.

use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct St {
    name: String,
    on: bool,
    n: u32,
    pick: Pick3,
    step: usize,
    done: bool,
    running: bool,
    ticks: u32,
}
choices! { enum Pick3 { A, B, C } }

fn view(s: &St) -> impl View<Act<St>> {
    col![
        input(at!(s.name)).hint("name"),
        button("flip").toggle(at!(s.on)),
        button("plus").add(at!(s.n), 2),
        button("reset").set(at!(s.n), 0),
        radios(at!(s.pick), Pick3::all().iter().copied()),
        stepper(at!(s.n)).max(9).labelled("n"),
        format!("name={} on={} n={} pick={:?}", s.name, s.on, s.n, s.pick),
    ]
}

#[test]
fn bindings_write_back() {
    let mut ui = Harness::new(sketch(St::default(), view), 400.0, 600.0);
    ui.click("name").type_text("Ada");
    ui.click("flip").click("plus").click("plus").click("B");
    assert!(ui.has_text("name=Ada on=true n=4 pick=B"));
    ui.click("+");
    assert!(ui.has_text("name=Ada on=true n=5 pick=B"));
    ui.click("reset");
    assert!(ui.has_text("name=Ada on=true n=0 pick=B"));
}

#[test]
fn stepper_stops_at_max() {
    let st = St {
        n: 9,
        ..Default::default()
    };
    let mut ui = Harness::new(sketch(st, view), 400.0, 600.0);
    ui.click("+");
    assert!(
        ui.has_text("name= on=false n=9 pick=A"),
        "+ is disabled at max"
    );
}

#[test]
fn tick_runs_only_while_the_condition_holds() {
    let mut app = sketch(St::default(), |s: &St| text(format!("{}", s.ticks))).tick_while(
        |s| s.running,
        secs(1.0),
        |s| s.ticks += 1,
    );
    assert!(app.subscriptions().is_empty(), "no timer while stopped");
    app.update(Act::new(|s: &mut St| s.running = true));
    assert_eq!(app.subscriptions().len(), 1);
}

fn wiz(s: &St) -> impl View<Act<St>> {
    wizard(at!(s.step))
        .page("One", !s.name.is_empty(), input(at!(s.name)).hint("who"))
        .page("Two", true, "second page")
        .finish("Go")
        .on_finish(|s| s.done = true)
        .done(format!("done={} {}", s.done, s.name))
}

#[test]
fn wizard_gates_and_finishes() {
    let mut ui = Harness::new(sketch(St::default(), wiz), 400.0, 300.0);
    ui.click("Next");
    assert!(
        ui.has_text("One"),
        "Next is disabled while the page is invalid"
    );
    ui.click("who").type_text("Bo");
    ui.click("Next");
    assert!(ui.has_text("second page") && ui.has_text("Go"));
    ui.click("Back");
    assert!(ui.has_text("Bo"), "Back keeps what was typed");
    ui.click("Next").click("Go");
    assert!(ui.has_text("done=true Bo"));
}

#[test]
fn table_keep_count_and_empty() {
    #[derive(Clone)]
    struct R(&'static str);
    let rows = [R("alpha"), R("beta"), R("alps")];
    let v = move |_: &()| {
        let t: Node<Act<()>> = table(&rows)
            .col("Name", |r| r.0.to_string())
            .keep(|r| r.0.starts_with("al"))
            .count("rows")
            .empty("none")
            .into();
        t
    };
    let ui = Harness::new(sketch((), v), 400.0, 300.0);
    assert!(ui.has_text("2 rows") && !ui.has_text("beta"));
}

mod vec_ops {
    use alloy_ui::{prelude::*, testing::Harness};

    state! {
        struct S {
            tags: Vec<u8>,
            draft: String = "x".to_string(),
        }
    }

    fn view(s: &S) -> impl View<Act<S>> {
        col![
            button("Push one").push(at!(s.tags), 1),
            button("Push two").push(at!(s.tags), 2),
            button("Drop ones").keep(at!(s.tags), |t| *t != 1),
            button("Wipe").clear(at!(s.draft)),
            format!("tags={:?} draft={:?}", s.tags, s.draft),
        ]
    }

    #[test]
    fn push_keep_clear() {
        let mut ui = Harness::new(sketch(S::default(), view), 300.0, 300.0);
        ui.click("Push one").click("Push two").click("Push one");
        assert!(ui.has_text("tags=[1, 2, 1] draft=\"x\""));
        ui.click("Drop ones").click("Wipe");
        assert!(ui.has_text("tags=[2] draft=\"\""));
    }
}
