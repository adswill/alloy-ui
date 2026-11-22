//! Steam Oxide feedback, area T: programmatic selection, `on_escape`, slider accessible label.
use alloy_ui::{prelude::*, testing::Harness, widgets::Role};
use std::{cell::RefCell, rc::Rc};

type Shared = Rc<RefCell<Option<alloy_ui::widgets::Ui>>>;

#[derive(Default)]
struct S {
    q: String,
    esc: u32,
    zoom: f32,
    range: std::ops::Range<usize>,
}

fn ui_of(h: &Shared) -> alloy_ui::widgets::Ui {
    h.borrow().clone().expect("view ran")
}

#[test]
fn selection_range_is_applied_once_and_user_can_change_it() {
    let shared: Shared = Rc::default();
    let sh = shared.clone();
    let app = sketch(
        S {
            q: "hello world".into(),
            range: 6..11,
            ..Default::default()
        },
        move |s: &S| {
            *sh.borrow_mut() = Some(alloy_ui::ui());
            input(at!(s.q)).id(5).selection(s.range.clone())
        },
    );
    let mut h = Harness::new(app, 300.0, 100.0);
    h.advance(16);
    assert_eq!(ui_of(&shared).text_selection(5), 6..11);
    ui_of(&shared).set_focus(Some(5));
    h.key("ArrowLeft");
    assert_eq!(
        ui_of(&shared).text_selection(5),
        6..6,
        "user moved the caret"
    );
}

#[test]
fn select_all_on_focus_by_click_and_by_tab() {
    let shared: Shared = Rc::default();
    let sh = shared.clone();
    let app = sketch(
        S {
            q: "steam".into(),
            ..Default::default()
        },
        move |s: &S| {
            *sh.borrow_mut() = Some(alloy_ui::ui());
            col![
                button("Other").act(|_: &mut S| {}),
                input(at!(s.q)).id(9).select_all_on_focus(),
            ]
        },
    );
    let mut h = Harness::new(app, 300.0, 200.0);
    h.click("steam");
    assert_eq!(ui_of(&shared).text_selection(9), 0..5);
    h.key("ArrowRight");
    assert_eq!(ui_of(&shared).text_selection(9), 5..5);
    // Tab away and back: focus gain selects everything again.
    ui_of(&shared).set_focus(Some(1));
    h.advance(16);
    ui_of(&shared).set_focus(Some(9));
    h.advance(16);
    assert_eq!(ui_of(&shared).text_selection(9), 0..5);
}

#[test]
fn on_escape_is_consumed_only_when_set() {
    let shared: Shared = Rc::default();
    let sh = shared.clone();
    let app = sketch(S::default(), move |s: &S| {
        *sh.borrow_mut() = Some(alloy_ui::ui());
        col![
            input(at!(s.q)).hint("Search").id(1).escape(|s: &mut S| {
                s.esc += 1;
                s.q.clear();
            }),
            text(format!("esc {}", s.esc)),
        ]
    });
    let mut h = Harness::new(app, 300.0, 200.0);
    h.click("Search").type_text("abc").key("Escape");
    h.expect("esc 1").expect("Search");
    assert_eq!(
        ui_of(&shared).focused(),
        Some(1),
        "keeps focus when handled"
    );

    let sh = shared.clone();
    let app = sketch(S::default(), move |s: &S| {
        *sh.borrow_mut() = Some(alloy_ui::ui());
        input(at!(s.q)).hint("Find").id(2)
    });
    let mut h = Harness::new(app, 300.0, 200.0);
    h.click("Find").key("Escape");
    assert_eq!(ui_of(&shared).focused(), None, "default Escape still blurs");
}

#[test]
fn slider_exposes_label_and_value_text() {
    let shared: Shared = Rc::default();
    let sh = shared.clone();
    let app = sketch(
        S {
            zoom: 25.0,
            ..Default::default()
        },
        move |s: &S| {
            *sh.borrow_mut() = Some(alloy_ui::ui());
            col![slider(0.0..=100.0, at!(s.zoom))
                .label("Grid zoom")
                .unit("%")]
        },
    );
    let h = Harness::new(app, 400.0, 100.0);
    let n = ui_of(&shared)
        .accessibility()
        .into_iter()
        .find(|n| n.role == Role::Slider)
        .expect("slider node");
    assert_eq!(n.label, "Grid zoom");
    assert_eq!(n.value.as_deref(), Some("25 %"));
    drop(h);
}
