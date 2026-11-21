# Async resources

`resource(fetch)` loads data in the background and gives you the loading state, the error card with Retry, refresh and optional auto-refresh with no state of your own.

```rust
use alloy_ui::prelude::*;

async fn load_users() -> Result<Vec<String>, String> {
    sleep(secs(0.5)).await; // stands in for a network call
    Ok(vec!["ada".into(), "grace".into()])
}

fn view(_: &()) -> impl View<Act<()>> {
    let users = resource(load_users);
    users.page("Users", |names| {
        col(names.iter().map(|n| text(n.clone())))
    })
}
# fn main() { let _ = sketch((), view); }
```

`fetch` is any `async fn() -> Result<T, E>` where `E: Display`. It starts the first time the view runs and the result is kept between frames. `page(title, view)` is a heading with a Refresh button over `show(view)`, which shows:

* "Loading…" until the first data arrives,
* an error card with a **Retry** button if the first load failed,
* your view of the data otherwise, with a "Refresh failed" note above it if a later refresh fails (the old data stays visible).

The future runs on a background thread driven by a small built-in executor, so plain `async fn` and `sleep` work with no runtime. A future that needs tokio (for example tokio sockets) should be wrapped by your app.

## Refreshing

```rust
use alloy_ui::prelude::*;

async fn load() -> Result<u32, String> { Ok(1) }

fn view(_: &()) -> impl View<Act<()>> {
    let n = resource(load).auto(secs(10.0)); // adds an "Auto refresh" switch to the header
    n.page("Counter", |v| text(format!("value {v}")))
}
# fn main() { let _ = sketch((), view); }
```

`reload()` refetches now, `every(period)` always refetches, `every_if(flag, period)` only while `flag` is true, `busy()` says whether a fetch is running and `get()` copies the latest data.

## Data that depends on something: `resource_for` and cancellation

When the data depends on a value, such as a search query or the selected row, give `resource_for` that value as the key:

```rust
use alloy_ui::prelude::*;

async fn search(q: String) -> Result<Vec<String>, String> {
    sleep(secs(0.3)).await; // debounce: a newer query cancels us during this wait
    Ok(vec![format!("result for {q}")])
}

struct Page { query: String }

fn view(s: &Page) -> impl View<Act<Page>> {
    let hits = resource_for(s.query.clone(), search);
    col![
        input(at!(s.query)).hint("Search"),
        hits.show(|rows| col(rows.iter().map(|r| text(r.clone()))).into()),
    ]
    .gap(12.0)
    .pad(24.0)
}
# fn main() { let _ = sketch(Page { query: String::new() }, view); }
```

Whenever the key differs from the last frame, the running fetch is **cancelled**: its future is dropped at the next `.await`, and a late answer can never replace newer data. The old data stays on screen while the new fetch runs (`busy()` tells you). Starting the fetch with a `sleep` therefore debounces typing for free, because a keystroke within that time cancels the fetch before it does any work.

The key must be `PartialEq + Clone + Send + Sync`. For manual control, `cancel()` stops the running fetch and keeps what is shown, and `restart()` cancels and fetches again (unlike `reload()`, which ignores the call while a fetch is running).

## One-off commands

For work started by a click rather than by showing a view, the Elm `App` trait's `Cmd::perform` runs a future and sends its result back as a message:

```rust
use alloy_ui::prelude::*;

#[derive(Clone)]
enum Msg { Fetch, Got(u32) }

#[derive(Default)]
struct Page { value: u32 }

impl App for Page {
    type Msg = Msg;

    fn update_cmd(&mut self, m: Msg) -> Cmd<Msg> {
        match m {
            Msg::Fetch => Cmd::perform(async { Msg::Got(42) }),
            Msg::Got(v) => {
                self.value = v;
                Cmd::none()
            }
        }
    }

    fn view(&self) -> impl View<Msg> {
        col![text(self.value.to_string()), button("Fetch").on(Msg::Fetch)]
    }
}
# fn main() { let _ = Page::default(); }
```

For fake data while you build the UI, `Service::mock` (a `Record` type's `mock`) gives you a backend with a delay and failures on demand: see the `status` example.

## Futures that feed your state: `Cmd::perform_keyed`

`resource` is for data a view shows. When the result must go through `update` (a search that fills your own struct, an upload that returns an id), return a command from `update_cmd`. `Cmd::perform(fut)` runs it; `Cmd::perform_keyed(key, fut)` runs it under a key, and starting another with the same key **cancels the one still running**, so only the latest result arrives. `Cmd::cancel(key)` stops it without starting another.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Search { shown: String }

#[derive(Clone)]
enum Msg { Typed(String), Found(String), Cancel }

impl App for Search {
    type Msg = Msg;
    fn update_cmd(&mut self, m: Msg) -> Cmd<Msg> {
        match m {
            Msg::Typed(q) => Cmd::perform_keyed("search", async move {
                sleep(secs(0.3)).await; // a newer keystroke cancels us during this wait
                Msg::Found(format!("results for {q}"))
            }),
            Msg::Found(r) => { self.shown = r; Cmd::none() }
            Msg::Cancel => Cmd::cancel("search"),
        }
    }
    fn view(&self) -> impl View<Msg> {
        col![button("Search").on(Msg::Typed("rust".into())), button("Cancel").on(Msg::Cancel), text(self.shown.clone())]
    }
}
# fn main() { let _ = Search::default(); }
```

The key is anything hashable (`"search"`, an enum, a `(Kind, id)` pair), so several independent streams can run at once. A cancelled future is dropped at its next wake-up, and its message is never delivered, even if it had just finished; `await` points are where it stops. Tests drive these with `Harness::settle()`, which waits for the commands too.
