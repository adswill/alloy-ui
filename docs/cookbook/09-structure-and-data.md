# Structure and data at scale

## 41. List, detail, and Back

`router` keeps a stack of screens. `push` drills in, `back_button` and `alt+←` come back.

```rust
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Clone)]
enum Route {
    List,
    Item(u32),
}

fn view(_: &()) -> impl View<Act<()>> {
    let nav = router(Route::List);
    nav.show(|r| match r {
        Route::List => col![
            heading("Orders"),
            button("Open order 7").act(move |_| nav.push(Route::Item(7))),
        ]
        .gap(10.0)
        .pad(20.0)
        .into(),
        Route::Item(id) => col![nav.back_button(), heading(format!("Order {id}"))].gap(10.0).pad(20.0).into(),
    })
}

fn main() {
    let mut ui = Harness::new(sketch((), view), 360.0, 200.0);
    ui.click("Open order 7").expect("Order 7").chord("alt+ArrowLeft").expect("Orders");
}
```

## 42. Show a hundred thousand rows

`vlist(count, row)` builds only the rows on screen, so a huge list costs the same as a small one. The row closure gets an index and returns the row's view.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn row_of(i: usize) -> impl Into<Node<Act<usize>>> {
    row![caption(format!("#{i}")).dim().w(70.0), text(format!("Event {i}")).fill_w()]
}

fn view(picked: &usize) -> impl View<Act<usize>> {
    col![
        caption(format!("Selected row {picked}")).dim(),
        vlist(100_000, row_of)
            .ruled()
            .row_height(32.0)
            .selected(Some(*picked))
            .act(|p: &mut usize, i| *p = i),
    ]
    .gap(8.0)
    .pad(16.0)
}

fn main() {
    let mut ui = Harness::new(sketch(0usize, view), 420.0, 320.0);
    ui.click("Event 2").expect("Selected row 2");
}
```

## 43. Edit a table in place

`data_grid` sorts by header click, resizes columns and edits cells. Describe each column with `.col(name, |row| value)`; editable ones report the new text to `.edit(..)`.

```rust
use alloy_ui::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
struct Person {
    name: String,
    team: &'static str,
}

struct People {
    rows: Arc<Vec<Person>>,
}

fn view(s: &People) -> impl View<Act<People>> {
    data_grid(&s.rows)
        .col("Name", |p: &Person| p.name.clone())
        .editable()
        .flex()
        .col("Team", |p: &Person| p.team)
        .width(120.0)
        .edit(|s: &mut People, row, _, text| Arc::make_mut(&mut s.rows)[row].name = text)
}

fn main() {
    let rows = vec![Person { name: "Ada".into(), team: "Core" }, Person { name: "Grace".into(), team: "Tools" }];
    let _ = sketch(People { rows: Arc::new(rows) }, view);
}
```

## 44. A board you drag cards across

`kanban` takes lanes of cards and reports each move. `mv.apply(..)` updates your lanes, so the board is just data.

```rust
use alloy_ui::{card, lane, prelude::*, Lane};

struct Board {
    lanes: Vec<Lane>,
}

fn view(s: &Board) -> impl View<Act<Board>> {
    kanban(&s.lanes).act(|s: &mut Board, mv| mv.apply(&mut s.lanes))
}

fn main() {
    let lanes = vec![
        lane("To do", [card(1, "Write docs"), card(2, "Fix sync")]),
        lane("Doing", [card(3, "Ship offline mode")]),
        lane("Done", []),
    ];
    let _ = sketch(Board { lanes }, view);
}
```

## 45. A file tree

`tree` shows nested items with expand and collapse. Give every item a unique `u64` id; selection reports the id.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Files {
    open: Option<u64>,
}

fn view(s: &Files) -> impl View<Act<Files>> {
    let items = vec![
        TreeItem::new(1, "src").children([TreeItem::new(2, "main.rs"), TreeItem::new(3, "lib.rs")]),
        TreeItem::new(4, "Cargo.toml"),
    ];
    col![
        tree(items).open([1]).selected(s.open).act(|s: &mut Files, id| s.open = Some(id)),
        caption(s.open.map_or("Nothing selected".to_string(), |id| format!("Selected {id}"))).dim(),
    ]
    .gap(8.0)
    .pad(16.0)
}
# fn main() { let _ = sketch(Files::default(), view); }
```
