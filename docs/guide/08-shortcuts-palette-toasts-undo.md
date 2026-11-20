# Shortcuts, command palette, toasts and undo

Four small features that make an app feel finished, each a line or two.

## Shortcuts

`mod` is Cmd on macOS and Ctrl on Linux and Windows. A focused text field keeps its own shortcuts (`mod+z` inside an input edits the input), then the app's shortcuts run.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| {
        col![text(n.to_string()), button("Add").act(|n| *n += 1)]
    })
    .shortcut("mod+up", |n| *n += 1)
    .shortcut_as("mod+r", "Reset", |n| *n = 0)
    .run("Shortcuts")
}
```

`shortcut_as` also lists the entry in the menu bar. Inside a view, `page.shortcut("mod+n", |s| ..)` (from the `Shortcuts` trait) registers one while that view is shown.

## Command palette

`mod+k` opens a searchable list of every command the app declared, with fuzzy matching, arrow keys and Enter. Every `shortcut_as` shows up there with its keys. Add entries that have no key:

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(Vec::<String>::new(), |items| col(items.iter().map(|i| text(i.clone()))))
        .command("Add item", |items| items.push("new".into()))
        .command("Clear", |items| items.clear())
        .run("Palette")
}
```

For `app(..)` apps the same `.shortcut`, `.shortcut_as` and `.command` methods take a plain `|| ..` closure; they usually act on a `store` or call `toast`.

## Toasts

`toast("Saved")` shows a message that fades in at the bottom of the window and out again after a few seconds. Call it from any handler.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(String::from("draft"), |s| {
        col![
            text(s.clone()),
            button("Save").act(|_| {
                toast("Saved");
            }),
            button("Delete").act(|s: &mut String| {
                s.clear();
                toast("Deleted").undo(); // adds an Undo button
            }),
        ]
    })
    .undoable()
    .run("Toasts")
}
```

`.undo()` adds an Undo button, `.secs(n)` sets how long it stays and `.error()` styles it as a failure.

## Undo and redo

`.undoable()` keeps a snapshot per change to the state and binds `mod+z` and `mod+shift+z`. The state needs `Clone + PartialEq`. A toast's Undo button runs the same undo. For `app(..)` apps, `.undoable()` covers every `store`; the theme and the current page are not undone.
