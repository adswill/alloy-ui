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
