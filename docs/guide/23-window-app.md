# Window and app events

## Close to the tray

`.on_close_requested` runs when the user closes the window (close button, Alt+F4, Cmd+W).
Do nothing and the window closes; call `cancel_close` and it stays open, which is how an
app lives on in the menu bar. Quit for real from a tray or menu item with `quit()`.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(0, |n: &i64| text(format!("{n}")))
        .tray("My app", |t| t.item("Quit", |_| quit()))
        .on_close_requested(|cx| cx.cancel_close())
        .run("Hi")
}
```

`cx.state()` reads the state (cancel only when work is unsaved), and
`cx.cancel_close_then(|s| s.asking = true)` also runs a handler. With the `App` trait,
`on_close()` returning a message does the same. In tests, `Harness::close()` returns false
when the app kept the window open.

## Is the window focused?

`app_focused()` is true while the window is frontmost, and views rebuild when it changes:
show a notification only when it is false. `.on_focus_changed(|s, focused| ..)` runs a handler
on every change (the `App` trait has `on_focus`). In tests, `ui.focus(false)` simulates
switching apps.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(0u32, |_: &u32| text(if app_focused() { "front" } else { "away" }))
        .on_focus_changed(|n, focused| if !focused { *n += 1 })
        .run("Hi")
}
```
