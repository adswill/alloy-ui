# Motion and loading

## 36. A skeleton while data loads

Show the shape of the content before it exists. `skeleton_text(n)` draws `n` shimmering lines, so the screen does not jump when the real text lands.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(loaded: &bool) -> impl View<Act<bool>> {
    col![
        heading("Release notes"),
        if *loaded {
            col![text("Faster startup."), text("Smoother scrolling.")].gap(6.0).into()
        } else {
            Node::from(skeleton_text(2).gap(8.0))
        },
        button("Load").act(|l: &mut bool| *l = true),
    ]
    .gap(12.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(false, view), 360.0, 220.0);
    ui.expect_no("Faster startup.").click("Load").expect("Faster startup.");
}
```

## 37. A number that rolls

`counter(value)` animates between values digit by digit. Chain `.prefix`, `.suffix` and `.decimals` for money and units.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(n: &i64) -> impl View<Act<i64>> {
    col![
        counter(*n as f64).large().prefix("$"),
        button("Add 250").act(|n: &mut i64| *n += 250),
    ]
    .gap(10.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(1200i64, view), 280.0, 160.0);
    ui.click("Add 250").settle();
    assert!(!ui.png().is_empty());
}
```

## 38. A list that arrives one row at a time

`stagger` delays each child a little more than the one before it, so a list reads as arriving instead of appearing all at once.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(shown: &bool) -> impl View<Act<bool>> {
    let rows = ["Build passed", "Tests passed", "Deployed"];
    col![
        button("Show activity").act(|s: &mut bool| *s = true),
        if *shown {
            Node::from(stagger(rows.map(|r| text(r).pad_xy(12.0, 8.0).surface().round(6.0))).gap(6.0))
        } else {
            Node::from(text(""))
        },
    ]
    .gap(12.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(false, view), 360.0, 260.0);
    ui.click("Show activity").settle().expect("Deployed");
}
```

## 39. A stat card with a trend line

`stat` is a label over a big number; add a `sparkline` beside it for the shape of the last few days.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(visits: &Vec<f32>) -> impl View<Act<Vec<f32>>> {
    let total: f32 = visits.iter().sum();
    row![stat("Visits this week", total as i64), space(), sparkline(visits.clone()).w(120.0)]
        .align_center()
        .card()
        .w(360.0)
}

fn main() {
    let ui = Harness::new(sketch(vec![12.0, 18.0, 15.0, 24.0, 30.0], view), 420.0, 140.0);
    assert!(ui.has_text("99"));
}
```

## 40. A donut chart you can click

`pie` draws slices from `(name, value)` pairs. `.donut()` leaves a hole for a title, and `.act(..)` reports which slice was clicked.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Spend {
    picked: Option<usize>,
}

fn view(s: &Spend) -> impl View<Act<Spend>> {
    let names = ["Rent", "Food", "Travel"];
    col![
        pie([("Rent", 1200.0), ("Food", 540.0), ("Travel", 310.0)])
            .donut()
            .title("This month")
            .unit(" €")
            .selected(s.picked)
            .act(|s: &mut Spend, i| s.picked = Some(i))
            .height(200.0),
        caption(s.picked.map_or("Click a slice".to_string(), |i| names[i].to_string())).dim(),
    ]
    .gap(8.0)
    .pad(20.0)
}
# fn main() { let _ = sketch(Spend::default(), view); }
```
