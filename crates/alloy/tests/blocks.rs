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
    assert!(h.has_text("Required"), "new row has an empty required name");
    h.click("Delete");
    assert!(h.has_text("Pear") || h.has_text("Apple"));
}

#[test]
fn local_state_survives_frames() {
    let app = sketch((), |_| {
        let open = local(false);
        let o = open.clone();
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
