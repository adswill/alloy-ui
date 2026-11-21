# Getting started

Alloy is a native Rust UI framework. You write state and a view function; Alloy opens a window, draws it on the GPU and calls your closures when someone clicks. There is no webview and no HTML or CSS underneath.

## Install the tool

```sh
cargo install --git https://github.com/adswill/alloy-ui alloy-ui-cli
alloy new hello --identifier com.example.hello
cd hello
alloy dev
```

`alloy new` writes a `Cargo.toml` with an `[package.metadata.alloy]` table, a capability file (`alloy.capabilities.toml`), icons and a starter `src/main.rs`. `alloy dev` validates the capability file, then runs a debug build. `alloy check` validates the metadata without building.

### Starter templates

`alloy new` writes a counter by default. Pick another starting point with `--template`:

| template | what you get |
|---|---|
| `counter` | the smallest complete app (about 20 lines) |
| `dashboard` | headline numbers, a live chart and a table of jobs |
| `editor` | a text editor that opens and saves files, with Ctrl/Cmd+S |
| `settings` | a settings panel: pages, a theme switch, controls bound with `at!` |
| `localized` | a translated app (English, French, Arabic) with live language switching and right-to-left layout, see [Localization](13-localization.md) |

```sh
alloy new inbox --template localized
```

An unknown name lists the valid ones (and suggests the closest). Every template is compiled and driven headless by this repository's tests, so none of them goes stale.

## Your first app

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| {
        col![
            heading("Counter"),
            text(format!("{n} so far")).dim(),
            row![
                button("Subtract").act(|n| *n -= 1),
                button("Add").primary().act(|n| *n += 1),
            ],
        ]
        .gap(14.0)
        .pad(24.0)
    })
    .run("Counter")
}
```

`sketch(state, view)` takes your state (here an `i64`) and a function from `&state` to a view. `button(..).act(|n| ..)` says what happens on click: a closure that gets `&mut` state. After every handler the view runs again and Alloy draws the difference. There is no message enum and no `update` function to write.

The closure parameter `n` has a different type in each place: in the view it is `&i64`, in `.act` it is `&mut i64`. If inference needs help in a handler, annotate it: `.act(|s: &mut State| ..)`.

## Run it without a screen

Every Alloy app can render itself to a PNG and click things, which is how the examples are tested. `Harness` is the same runtime with scripted input:

```rust
use alloy_ui::{prelude::*, testing::Harness};

let mut app = Harness::new(
    sketch(0i64, |n| {
        col![text(format!("{n} so far")), button("Add").act(|n| *n += 1)]
    }),
    300.0,
    200.0,
);
app.click("Add").click("Add");
assert!(app.has_text("2 so far"));
```

## Two ways to write an app

* **`sketch`**: state plus a view function, handlers are closures. Use this by default.
* **`App`**: the Elm contract (`Msg` enum, `update`, `view`), for apps that want a message log, background `Cmd`s or `subscriptions`. See [multi-window](07-multi-window.md) for an example.

Both are fully typed and share the same widgets. Next: [state and bindings](02-state-and-bindings.md).
