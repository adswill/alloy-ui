# Finished-app details

## 16. Keyboard shortcuts

`mod` is Cmd on macOS and Ctrl elsewhere. `shortcut_as` also lists the entry in the menu bar and the command palette.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| col![heading(n.to_string()), button("Add").act(|n| *n += 1)].gap(8.0).pad(20.0))
        .shortcut("mod+up", |n| *n += 1)
        .shortcut_as("mod+0", "Reset", |n| *n = 0)
        .run("Shortcuts")
}
```

## 17. A command palette

`mod+k` opens a fuzzy-searchable list of every shortcut and every `.command`. Commands need no key.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(Vec::<String>::new(), |items| col(items.iter().map(|i| text(i.clone()))).pad(20.0))
        .command("Add an item", |items| items.push(format!("Item {}", items.len() + 1)))
        .command("Clear all", |items| items.clear())
        .run("Palette")
}
```

## 18. A toast with Undo

`toast` shows a message at the bottom of the window. `.undo()` adds a button that runs the app's undo.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(vec!["Invoice 41".to_string(), "Invoice 42".to_string()], |rows| {
        col![
            col(rows.iter().map(|r| text(r.clone()))),
            button("Delete last").act(|rows: &mut Vec<String>| {
                rows.pop();
                toast("Deleted").undo();
            }),
        ]
        .gap(10.0)
        .pad(20.0)
    })
    .undoable()
    .run("Invoices")
}
```

## 19. Undo and redo for free

`.undoable()` keeps a snapshot per change and binds `mod+z` and `mod+shift+z`. The state only needs `Clone + PartialEq`. The test drives the same chord.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn main() {
    let app = sketch(0i64, |n| col![heading(format!("n = {n}")), button("Add").act(|n| *n += 1)]).undoable();
    let mut ui = Harness::new(app, 300.0, 160.0);
    ui.click("Add").click("Add");
    assert!(ui.has_text("n = 2"));
    ui.chord("ctrl+z");
    assert!(ui.has_text("n = 1"));
}
```

## 20. State that survives a restart

`.persist("name")` reads the state from the user's config directory at start and writes it after every change. Derive serde on your state and that is all.

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Draft {
    title: String,
    body: String,
}

fn main() {
    sketch(Draft::default(), |d| {
        col![input(at!(d.title)).hint("Title"), editor(at!(d.body)).fill()].gap(8.0).pad(16.0)
    })
    .persist("cookbook-draft")
    .run("Draft")
}
```
