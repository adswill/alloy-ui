# Data

## 11. A sortable, filterable table

Click a header to sort. `.filter(query)` keeps only rows where some cell contains the text, and `.status()` draws a dot before the words `Up`, `Degraded` and `Down`.

```rust
use alloy_ui::{prelude::*, testing::Harness};

struct Service {
    name: &'static str,
    state: &'static str,
    ms: u32,
}

fn view(rows: &Vec<Service>) -> impl View<Act<Vec<Service>>> {
    let q = local(String::new());
    col![
        input(q.get()).local(&q).hint("Filter services"),
        table(rows)
            .col("Service", |s: &Service| s.name.to_string())
            .col("State", |s: &Service| s.state.to_string())
            .status()
            .num("Latency (ms)", |s: &Service| s.ms.to_string())
            .filter(&q.get()),
    ]
    .gap(10.0)
    .pad(16.0)
}

fn main() {
    let rows = vec![
        Service { name: "api", state: "Up", ms: 38 },
        Service { name: "search", state: "Degraded", ms: 410 },
        Service { name: "billing", state: "Down", ms: 0 },
    ];
    let mut ui = Harness::new(sketch(rows, view), 560.0, 320.0);
    ui.click("Filter services").type_text("sea");
    assert!(ui.has_text("search") && !ui.has_text("billing"));
}
```

## 12. Master and detail

`list` takes any views as rows and tells you which one was picked. Keep the selection in state and the detail pane is just a function of it.

```rust
use alloy_ui::{prelude::*, testing::Harness};

struct Notes {
    titles: Vec<&'static str>,
    current: usize,
}

fn view(s: &Notes) -> impl View<Act<Notes>> {
    row![
        list(s.titles.iter().map(|t| text(*t)), Some(s.current))
            .act(|s: &mut Notes, i| s.current = i)
            .w(200.0),
        col![heading(s.titles[s.current]), text("Edited just now").dim()].gap(6.0).pad(20.0).fill(),
    ]
    .fill()
}

fn main() {
    let s = Notes { titles: vec!["Groceries", "Ideas", "Trip"], current: 0 };
    let mut ui = Harness::new(sketch(s, view), 520.0, 280.0);
    ui.click("Trip");
    assert!(ui.has_text("Trip") && ui.has_text("Edited just now"));
}
```

## 13. A CRUD screen from a struct

Derive `Record` and `crud` gives a searchable, sortable table with Add, Delete and a validated edit form. Field types decide the controls: text, a picker for a `choices!` enum, a slider for a ranged number, a checkbox for a `bool`.

```rust,no_run
use alloy_ui::prelude::*;

choices! { enum Plan { Free, Team, Business } }

#[derive(Record, Clone, Default)]
struct Customer {
    name: String,
    email: String,
    plan: Plan,
    #[form(range = (1.0, 500.0), step = 1.0)]
    seats: f32,
    active: bool,
}

fn main() {
    app("Customers")
        .page("Customers", || {
            crud(|| {
                Records::<Customer>::of([
                    vals!["Ada Lovelace", "ada@acme.dev", Plan::Team, 12, true],
                    vals!["Grace Hopper", "grace@navy.mil", Plan::Business, 240, true],
                ])
            })
        })
        .persist()
        .run()
}
```

## 14. Loading data with retry

`resource` runs an `async fn` in the background and gives you Loading, an error card with **Retry**, and a Refresh button. You write only the success view.

```rust
use alloy_ui::prelude::*;

async fn load_releases() -> Result<Vec<String>, String> {
    sleep(secs(0.4)).await; // stands in for a network call
    Ok(vec!["0.4.0".into(), "0.3.2".into(), "0.3.1".into()])
}

fn view(_: &()) -> impl View<Act<()>> {
    resource(load_releases).page("Releases", |versions| {
        col(versions.iter().map(|v| text(format!("alloy {v}")))).gap(6.0)
    })
}
# fn main() { let _ = sketch((), view); }
```

## 15. A value that refreshes itself

`.auto(period)` adds an "Auto refresh" switch to the header. `every(period)` refetches unconditionally.

```rust
use alloy_ui::prelude::*;

async fn queue_depth() -> Result<u32, String> {
    Ok(7) // a real app would ask its server
}

fn view(_: &()) -> impl View<Act<()>> {
    resource(queue_depth)
        .auto(secs(5.0))
        .page("Queue", |n| heading(format!("{n} jobs waiting")))
}
# fn main() { let _ = sketch((), view); }
```
