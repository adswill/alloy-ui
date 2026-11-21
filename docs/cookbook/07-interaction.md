# Interaction patterns

## 31. Tabs that switch a screen

`tabs` binds a segmented control to an index. Match on the index to pick what shows underneath.

```rust
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct Account {
    page: usize,
}

fn view(s: &Account) -> impl View<Act<Account>> {
    let body: Node<Act<Account>> = match s.page {
        0 => text("Plan: Team, 12 seats").into(),
        1 => text("Last sign-in: today").into(),
        _ => text("Two-factor is on").into(),
    };
    col![tabs(["Billing", "Activity", "Security"], at!(s.page)), body].gap(16.0).pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(Account::default(), view), 420.0, 200.0);
    ui.expect("Plan: Team, 12 seats").click("Security").expect("Two-factor is on");
}
```

## 32. Ask before a destructive action

`.confirm(..)` goes last. The first click swaps the button for "prompt, confirm, Cancel"; only the second click runs the action, and Cancel puts everything back.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(files: &Vec<&'static str>) -> impl View<Act<Vec<&'static str>>> {
    col![
        col(files.iter().map(|f| text(*f))),
        button("Delete all").act(|f: &mut Vec<&'static str>| f.clear()).confirm("Delete every file?"),
    ]
    .gap(12.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(vec!["a.txt", "b.txt"], view), 360.0, 200.0);
    ui.click("Delete all").expect("Delete every file?").click("Cancel").expect("a.txt");
    ui.click("Delete all").click("Delete all").expect_no("a.txt");
}
```

## 33. A right-click menu

`context_menu(child, entries)` opens at the pointer. Entries are plain builders, and `menu_rule()` draws a divider.

```rust
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct Files {
    note: String,
}

fn view(s: &Files) -> impl View<Act<Files>> {
    col![
        context_menu(
            text("report.pdf").pad(12.0).surface().round(6.0),
            [
                menu_item("Rename").act(|s: &mut Files| s.note = "renaming".into()),
                menu_rule(),
                menu_item("Delete").danger().act(|s: &mut Files| s.note = "deleted".into()),
            ],
        ),
        caption(s.note.clone()).dim(),
    ]
    .gap(10.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(Files::default(), view), 360.0, 220.0);
    ui.right_click("report.pdf").click("Delete").expect("deleted");
}
```

## 34. A list beside its details

`split` gives the user a draggable divider. `ratio` is where it starts and `min` stops a side from collapsing.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(picked: &usize) -> impl View<Act<usize>> {
    let names = ["Ada", "Grace", "Alan"];
    split(
        col(names.iter().enumerate().map(|(i, n)| button(*n).ghost().act(move |p: &mut usize| *p = i))).gap(4.0).pad(12.0),
        col![heading(names[*picked]), text("Selected from the list.").dim()].gap(6.0).pad(20.0),
    )
    .ratio(0.3)
    .min(120.0)
}

fn main() {
    let mut ui = Harness::new(sketch(0usize, view), 560.0, 260.0);
    ui.click("Grace").expect("Selected from the list.").expect("Grace");
}
```

## 35. A section that opens and closes

`reveal(show, child)` grows and shrinks the child instead of popping it in, and the rows below glide with it.

```rust
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct Form {
    open: bool,
}

fn view(s: &Form) -> impl View<Act<Form>> {
    col![
        toggle("Advanced options", at!(s.open)),
        reveal(s.open, col![text("Retry limit: 3"), text("Timeout: 30 s")].gap(4.0).pad_xy(0.0, 8.0)),
        text("Footer"),
    ]
    .gap(8.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(Form::default(), view), 360.0, 220.0);
    ui.click("Advanced options").settle().expect("Timeout: 30 s");
}
```
