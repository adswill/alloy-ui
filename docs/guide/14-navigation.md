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

For a sidebar app with fixed sections use [`app(..).page(..)`](04-widgets.md) instead; the router is for drill-down flows (list, detail, edit) where Back matters. Both can be mixed: a page body may own a router.

Test a flow like a person would: `Harness::click("Open message 7").expect("Message 7").chord("alt+ArrowLeft").expect("Inbox")` (see [testing](15-testing.md)).
