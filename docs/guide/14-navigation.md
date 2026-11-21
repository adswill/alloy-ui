# Navigation: routes, back and forward

`router(home)` keeps a stack of screens for the window. A route is any type you choose, usually an enum, so the compiler checks every screen is handled and every link carries the data it needs.

```rust
use alloy_ui::prelude::*;

#[derive(Clone)]
enum Route {
    Inbox,
    Mail(u32),
}

fn view(_: &()) -> impl View<Act<()>> {
    let nav = router(Route::Inbox);
    nav.show(|route| match route {
        Route::Inbox => col![
            heading("Inbox"),
            button("Open message 7").act(move |_| nav.push(Route::Mail(7))),
        ]
        .gap(12.0)
        .pad(24.0)
        .into(),
        Route::Mail(id) => col![
            nav.back_button(),
            heading(format!("Message {id}")),
        ]
        .gap(12.0)
        .pad(24.0)
        .into(),
    })
}
# fn main() { let _ = sketch((), view); }
```

`nav` is a small `Copy` handle, so any handler can capture it with no `.clone()`. It lives with the widget tree (like [`local`](02-state-and-bindings.md)), not in your state struct.

| Call | Does |
|---|---|
| `push(route)` | go to a screen; clears the forward history, like a browser |
| `replace(route)` | swap the current screen without adding a history step |
| `back()` / `forward()` | step through the history |
| `reset(route)` | drop the history and make `route` the only screen |
| `current()`, `trail()`, `depth()`, `can_back()`, `can_forward()` | ask where you are |
| `show(\|route\| ..)` | build the current screen **and** bind `alt+←` / `alt+→` |
| `back_button()` | a Back button, disabled at the first screen |
| `breadcrumbs(\|route\| label)` | `Inbox › Message 7`, each earlier step a link back |

## Reopen where you left off

`router_saved("my-app", Route::Inbox)` is `router` that remembers. The stack is written to `<app data>/my-app/router.json` after every change and read back at launch, so the app reopens on the screen (and with the Back history) the person left. Routes must derive `Serialize` and `Deserialize`. A missing, corrupt or out-of-date file (say you renamed a variant) is ignored and the app starts at `home`; at most the last 50 steps are kept. `router_saved_at(path, home)` takes an explicit file.

```rust
use alloy_ui::prelude::*;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum Route {
    Inbox,
    Mail(u32),
}

fn view(_: &()) -> impl View<Act<()>> {
    let nav = router_saved("my-mail", Route::Inbox);
    nav.show(|route| match route {
        Route::Inbox => button("Open message 7").act(move |_| nav.push(Route::Mail(7))).into(),
        Route::Mail(id) => col![nav.back_button(), heading(format!("Message {id}"))].into(),
    })
}
# fn main() { let _ = sketch((), view); }
```

## Deep links

`.links(parse)` lets URLs open screens. Register a scheme with `url_schemes` (an `App`) or `.url_scheme("myapp")` (a `sketch`), then tell the router how to read a link. `link_path` splits `myapp://note/3?x=1` into `["note", "3"]`; return `None` for anything that is not a route and the link is ignored.

```rust
use alloy_ui::prelude::*;

#[derive(Clone, PartialEq)]
enum Route {
    Inbox,
    Mail(u32),
}

fn view(_: &()) -> impl View<Act<()>> {
    let nav = router(Route::Inbox).links(|url| match link_path(url).as_slice() {
        [kind, id] if kind == "mail" => id.parse().ok().map(Route::Mail),
        _ => None,
    });
    nav.show(|route| match route {
        Route::Inbox => heading("Inbox").into(),
        Route::Mail(id) => col![nav.back_button(), heading(format!("Message {id}"))].into(),
    })
}
# fn main() { let _ = sketch((), view); }
```

A link pushes its screen (so Back returns to where the person was) unless that screen is already showing. It works while the app runs, for a second launch of a single-instance app, and for the link that launched the app: one that arrives before the router exists waits for it. Combine with `router_saved` to restore the last session and then jump to the link. Test it with `Harness::open_url("myapp://mail/7")`, or by hand with `my-app --open myapp://mail/7`.

For a sidebar app with fixed sections use [`app(..).page(..)`](04-widgets.md) instead; the router is for drill-down flows (list, detail, edit) where Back matters. Both can be mixed: a page body may own a router.

Test a flow like a person would: `Harness::click("Open message 7").expect("Message 7").chord("alt+ArrowLeft").expect("Inbox")` (see [testing](15-testing.md)).
