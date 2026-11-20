//! Shortcuts, toasts and undo.
use alloy_ui::{prelude::*, testing::Harness, Key};

fn demo() -> impl App<Msg = Act<i32>> {
    sketch(0, |n: &i32| -> Node<Act<i32>> {
        col![
            n.to_string(),
            button("Delete").act(|n: &mut i32| {
                *n += 10;
                toast("Deleted").undo();
            })
        ]
        .into()
    })
    .shortcut("mod+s", |n| {
        *n += 1;
        toast("Saved");
    })
    .undoable()
}

#[test]
fn shortcut_runs_and_toasts() {
    let mut h = Harness::new(demo(), 400.0, 300.0);
    assert!(h.has_text("0"));
    h.chord("mod+s");
    assert!(h.has_text("1"));
    h.advance(10).advance(200);
    assert!(h.has_text("Saved"));
    h.advance(4000).advance(500);
    assert!(!h.has_text("Saved"), "toast should auto-dismiss");
}

#[test]
fn undo_redo_and_toast_button() {
    let mut h = Harness::new(demo(), 400.0, 300.0);
    h.chord("mod+s").chord("mod+s");
    assert!(h.has_text("2"));
    h.chord("mod+z");
    assert!(h.has_text("1"));
    h.chord("mod+shift+z");
    assert!(h.has_text("2"));
    h.click("Delete");
    assert!(h.has_text("12"));
    h.advance(10).advance(400);
    if std::env::var_os("KEYS_PNG").is_some() {
        h.save_png("/tmp/toast.png");
    }
    h.click("Undo");
    assert!(h.has_text("2"), "toast Undo steps back");
    assert!(!h.has_text("Undo"), "clicking dismisses the toast");
}

#[test]
fn no_undo_without_undoable() {
    let a = sketch(0, |n: &i32| -> Node<Act<i32>> {
        col![n.to_string()].into()
    })
    .shortcut("mod+s", |n| *n += 1);
    let mut h = Harness::new(a, 400.0, 300.0);
    h.chord("mod+s").chord("mod+z");
    assert!(h.has_text("1"));
}

#[derive(Record, Clone, Default)]
struct Note {
    #[form(long)]
    body: String,
}

#[test]
fn pages_new_delete_undo() {
    let pages = app("Notes")
        .page("Notes", || {
            let notes = store(|| Note::rows([vals!["First\nhello"], vals!["Second\nworld"]]));
            browse_of(&notes)
        })
        .undoable();
    let mut h = Harness::new(pages, 1100.0, 600.0);
    assert!(h.has_text("Second"));
    h.chord("mod+n");
    assert!(h.has_text("Untitled") || h.has_text("New note") || h.has_text("First"));
    h.chord("mod+z");
    assert!(h.has_text("First") && h.has_text("Second"));
    h.click("Delete");
    h.advance(10).advance(400);
    assert!(
        h.has_text("Deleted"),
        "delete shows a toast (no confirm with undo)"
    );
    h.click("Undo");
    assert!(h.has_text("First") && h.has_text("Second"));
}

fn with_command() -> impl App<Msg = Act<i32>> {
    sketch(0, |n: &i32| -> Node<Act<i32>> { n.to_string().into() })
        .shortcut_as("mod+s", "Save", |n| *n += 1)
        .command("Reset counter", |n| *n = 0)
        .command("Double it", |n| *n *= 2)
}

#[test]
fn palette_lists_filters_and_runs() {
    let mut h = Harness::new(with_command(), 700.0, 400.0);
    h.chord("mod+s").chord("mod+s").chord("mod+s");
    assert!(h.has_text("3"));
    h.chord("mod+k");
    for t in ["Save", "Reset counter", "Double it"] {
        assert!(h.has_text(t), "palette should list {t}");
    }
    // Fuzzy: "dbl" finds "Double it" only, and Enter runs it.
    h.type_text("dbl");
    assert!(h.has_text("Double it") && !h.has_text("Reset counter"));
    h.press(Key::Enter);
    assert!(!h.has_text("Double it"), "palette closes after running");
    assert!(h.has_text("6"));
    // Escape closes without running; mod+k reopens with a clean query.
    h.chord("mod+k").press(Key::Escape);
    assert!(!h.has_text("Reset counter"));
    // Shortcuts do not leak through while it is open.
    h.chord("mod+k").chord("mod+s");
    assert!(h.has_text("6"));
}
