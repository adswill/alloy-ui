# Layout

## 6. Sidebar and content

A fixed-width column next to a column that fills the rest. `.surface()` paints the sidebar colour.

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    row![
        col![heading("Projects"), text("Inbox"), text("Today"), text("Later")]
            .gap(8.0)
            .pad(16.0)
            .w(180.0)
            .fill_h()
            .surface(),
        col![heading("Inbox"), text("Nothing waiting.").dim()].gap(8.0).pad(24.0).fill(),
    ]
    .fill()
}
# fn main() { let _ = sketch((), view); }
```

## 7. A toolbar with a right-aligned action

`space()` takes all the room that is left, which pushes whatever follows it to the far edge.

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    col![
        row![heading("Reports"), space(), button("Export").ghost(), button("New report").primary()]
            .gap(8.0)
            .align_center()
            .pad_xy(20.0, 12.0),
        rule(),
        col![text("Pick a report to begin.").dim()].pad(24.0),
    ]
}
# fn main() { let _ = sketch((), view); }
```

## 8. A log that scrolls

Wrap any view in `scroll` and give it a bounded height by putting it in a container that fills.

```rust
use alloy_ui::prelude::*;

fn view(lines: &Vec<String>) -> impl View<Act<Vec<String>>> {
    col![
        heading("Build log"),
        scroll(col(lines.iter().map(|l| text(l.clone()))).gap(2.0)).fill(),
    ]
    .gap(8.0)
    .pad(16.0)
    .fill()
}

fn main() {
    let log = (0..500).map(|i| format!("step {i}: ok")).collect::<Vec<_>>();
    let _ = sketch(log, view);
}
```

## 9. A row of cards

`.card()` draws a bordered surface. Fill children share the width equally, so the cards stay even at any window size.

```rust
use alloy_ui::prelude::*;

fn card(title: &str, value: &str) -> impl View<Act<()>> {
    col![caption(title.to_string()).dim(), heading(value.to_string())]
        .gap(4.0)
        .pad(16.0)
        .card()
        .fill_w()
}

fn view(_: &()) -> impl View<Act<()>> {
    row![card("Open issues", "42"), card("Merged this week", "17"), card("Median review", "3h")]
        .gap(12.0)
        .pad(20.0)
}
# fn main() { let _ = sketch((), view); }
```

## 10. An empty state that tells you what to do

Say what is missing and offer the one action that fixes it. Centering the container centers its content.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(items: &Vec<String>) -> impl View<Act<Vec<String>>> {
    if items.is_empty() {
        return col![
            heading("No notes yet"),
            text("Notes you write show up here.").dim(),
            button("Write the first one").primary().act(|v: &mut Vec<String>| v.push("Hello".into())),
        ]
        .gap(10.0)
        .center()
        .fill()
        .boxed();
    }
    col(items.iter().map(|i| text(i.clone()))).pad(20.0).boxed()
}

fn main() {
    let mut ui = Harness::new(sketch(Vec::new(), view), 360.0, 240.0);
    assert!(ui.has_text("No notes yet"));
    ui.click("Write the first one");
    assert!(ui.has_text("Hello") && !ui.has_text("No notes yet"));
}
```
