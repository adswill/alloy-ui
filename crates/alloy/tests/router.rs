//! `router`: push, back, forward, breadcrumbs and the alt+arrow keys.
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Clone, PartialEq, Debug)]
enum Route {
    List,
    Note(u32),
}

fn name(r: &Route) -> String {
    match r {
        Route::List => "Notes".into(),
        Route::Note(i) => format!("Note {i}"),
    }
}

fn app() -> impl App {
    sketch((), |_| {
        let nav = router(Route::List);
        nav.show(|route| match route {
            Route::List => col![
                heading("All notes"),
                button("Open 1").act(move |_| nav.push(Route::Note(1))),
                button("Open 2").act(move |_| nav.push(Route::Note(2))),
            ]
            .into(),
            Route::Note(i) => col![
                nav.breadcrumbs(name),
                heading(format!("Body {i}")),
                nav.back_button(),
                button("Replace").act(move |_| nav.replace(Route::Note(9))),
            ]
            .into(),
        })
    })
}

#[test]
fn push_back_forward() {
    let mut h = Harness::new(app(), 500.0, 300.0);
    h.expect("All notes")
        .click("Open 2")
        .expect("Body 2")
        .expect_no("All notes");
    h.click("Back").expect("All notes").expect_no("Body 2");
    h.chord("alt+ArrowRight").expect("Body 2");
    h.chord("alt+ArrowLeft").expect("All notes");
}

#[test]
fn push_after_back_clears_forward() {
    let mut h = Harness::new(app(), 500.0, 300.0);
    h.click("Open 1").click("Back").click("Open 2");
    h.chord("alt+ArrowRight").expect("Body 2"); // no forward history: stays put
}

#[test]
fn breadcrumbs_jump_back_and_replace_keeps_depth() {
    let mut h = Harness::new(app(), 500.0, 300.0);
    h.click("Open 1").click("Replace").expect("Body 9");
    h.expect("Note 9").click("Notes").expect("All notes");
}
