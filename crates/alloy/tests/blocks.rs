//! Building blocks (table, local state, records, persistence) through the real runtime.
use alloy_ui::testing::Harness;
use alloy_ui::{prelude::*, vals};

#[derive(Record, Clone, Default)]
struct Item {
    name: String,
    #[form(range = (0.0, 100.0))]
    qty: u32,
    done: bool,
}

fn items() -> Records<Item> {
    Records::of([vals!["Pear", 30, false], vals!["Apple", 5, true]])
}

/// Y position of the first text equal to `t`, to check row order.
fn order(h: &Harness<impl App>, a: &str, b: &str) -> bool {
    h.position(a).unwrap().y < h.position(b).unwrap().y
}

#[test]
fn records_add_delete_and_sort() {
    let app = sketch(items(), |d| {
        records_in("Items", d, |d: &mut Records<Item>| d)
    });
    let mut h = Harness::new(app, 900.0, 500.0);
    assert!(order(&h, "Pear", "Apple"));
    h.click("Name");
    assert!(order(&h, "Apple", "Pear"), "ascending by name");
    h.click("Name");
    assert!(order(&h, "Pear", "Apple"), "descending by name");
    h.click("Qty");
    assert!(
        order(&h, "Apple", "Pear"),
        "numeric, not text, sort: 5 < 30"
    );
    h.click("Add");
    assert!(
        !h.has_text("Required"),
        "a fresh row's empty required name is not an error until touched"
    );
    h.click("Delete");
    assert!(h.has_text("Pear") || h.has_text("Apple"));
}

#[test]
fn local_state_survives_frames() {
    let app = sketch((), |_| {
        let open = local(false);
        let o = open;
        col![
            button("Toggle").act(move |_| o.toggle()),
            text(if open.get() { "open" } else { "closed" })
        ]
    });
    let mut h = Harness::new(app, 300.0, 100.0);
    assert!(h.has_text("closed"));
    h.click("Toggle");
    assert!(h.has_text("open"));
    h.click("Toggle");
    assert!(h.has_text("closed"));
}

#[test]
fn persist_round_trips() {
    let dir = std::env::temp_dir().join(format!("alloy-persist-{}", std::process::id()));
    std::env::set_var("XDG_CONFIG_HOME", &dir);
    std::env::set_var("APPDATA", &dir);
    std::env::set_var("HOME", &dir);
    let make = || {
        sketch(0i32, |n: &i32| {
            button(format!("n={n}")).act(|n: &mut i32| *n += 1)
        })
        .persist("t")
    };
    let mut h = Harness::new(make(), 200.0, 100.0);
    h.click("n=0").click("n=1");
    drop(h);
    let h = Harness::new(make(), 200.0, 100.0);
    assert!(h.has_text("n=2"), "state restored from disk");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pages_keep_state_per_page_and_switch() {
    let app = app("t")
        .page("A", || crud(items))
        .page("B", || text("second page"));
    let mut h = Harness::new(app, 900.0, 500.0);
    assert!(h.position("Pear").is_some());
    h.click("B");
    assert!(h.position("second page").is_some() && h.position("Pear").is_none());
    h.click("A");
    h.click("Pear");
    assert!(
        h.position("Qty").is_some(),
        "form opens after selecting, state survived the page switch"
    );
}

choices! { enum Kind2 { A, B } }

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Doc {
    body: String,
    pinned: bool,
}

fn docs() -> Local<Vec<Doc>> {
    store(|| {
        vec![
            Doc {
                body: "Alpha\nfirst".into(),
                pinned: false,
            },
            Doc {
                body: "Beta\nsecond".into(),
                pinned: true,
            },
        ]
    })
}

#[test]
fn browse_pins_searches_adds_and_confirms_delete() {
    let app = sketch((), |_| {
        let d = docs();
        browse(&d)
            .doc(|n| &n.body)
            .pin(|n| n.pinned)
            .new(Doc::default)
            .detail(move |n| {
                col![
                    text(format!("editing {}", n.index())),
                    button("Delete").act(move |_| n.remove()).confirm("Really?")
                ]
            })
    });
    let mut h = Harness::new(app, 900.0, 500.0);
    assert!(
        h.position("★ Beta").unwrap().y < h.position("Alpha").unwrap().y,
        "pinned first"
    );
    assert!(h.has_text("editing 0"));
    h.click("New");
    assert!(h.has_text("editing 2"), "new row is selected");
    assert!(h.has_text("Untitled"));
    h.click("Delete");
    assert!(h.has_text("Really?"));
    h.click("Cancel");
    assert!(!h.has_text("Really?"));
    h.click("Delete");
    h.click("Delete");
    assert!(!h.has_text("Untitled"), "second click deletes");
}

#[test]
fn filter_and_table_detail() {
    let app = sketch((), |_| {
        let f = local(None::<Kind2>);
        let rows = f.apply(&[(1, Kind2::A), (2, Kind2::B)], |r| r.1);
        col![
            filter_by(&f),
            tally(&rows, |r| r.1),
            text(format!("{} rows", rows.len()))
        ]
    });
    let mut h = Harness::new(app, 600.0, 300.0);
    assert!(h.has_text("2 rows"));
    h.click("B");
    assert!(h.has_text("1 rows"));
}

#[derive(Default)]
struct Form {
    name: String,
    email: String,
    tried: bool,
}

fn form(s: &Form) -> impl View<Act<Form>> {
    col![
        field_after(
            "Name",
            input(at!(s.name)).hint("Ada"),
            alloy_ui::valid::required(&s.name),
            s.tried
        ),
        field(
            "Email",
            input(at!(s.email)).hint("ada@x.dev"),
            alloy_ui::valid::required(&s.email)
        ),
        button("Submit").act(|s: &mut Form| s.tried = true),
    ]
}

#[test]
fn errors_wait_for_blur_or_submit() {
    let mut h = Harness::new(sketch(Form::default(), form), 400.0, 400.0);
    assert!(!h.has_text("Required"), "untouched fields open clean");

    // Focused but still being edited: no error yet.
    h.click("ada@x.dev");
    assert!(!h.has_text("Required"), "no error while the field has focus");

    // Move on to the name field: the email field is now touched and empty.
    h.click("Ada");
    assert!(h.has_text("Required"), "error shows after blur");
    h.type_text("Grace");
    assert!(h.has_text("Required"), "email is still required");

    // A failed submit reveals the error on a field nobody touched.
    let mut h = Harness::new(sketch(Form::default(), form), 400.0, 400.0);
    h.click("Submit");
    assert!(h.has_text("Required"), "submit attempt shows errors");
}

#[test]
fn tabbing_through_marks_touched() {
    let mut h = Harness::new(sketch(Form::default(), form), 400.0, 400.0);
    h.click("Ada").press(alloy_ui::Key::Tab);
    h.press(alloy_ui::Key::Tab);
    assert!(h.has_text("Required"), "tabbed past an empty required field");
}
