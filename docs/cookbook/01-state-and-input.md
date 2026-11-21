# State and input

## 1. A counter with a step

State is a struct and a handler is a closure over `&mut` state. Keeping the step in the state lets one pair of buttons serve any step size.

```rust
use alloy_ui::{prelude::*, testing::Harness};

struct Counter {
    n: i64,
    step: i64,
}

fn view(s: &Counter) -> impl View<Act<Counter>> {
    col![
        heading(format!("Total {}", s.n)),
        row![
            button("Down").act(|s: &mut Counter| s.n -= s.step),
            button("Up").primary().act(|s: &mut Counter| s.n += s.step),
            button("Step 5").act(|s: &mut Counter| s.step = 5),
        ]
        .gap(8.0),
    ]
    .gap(12.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(Counter { n: 0, step: 1 }, view), 360.0, 200.0);
    ui.click("Up").click("Step 5").click("Up");
    assert!(ui.has_text("Total 6"));
}
```

## 2. A todo list with a filter

`choices!` turns an enum into something radio groups and segmented controls understand, and `options(..).bind(..)` ties one to a field.

```rust
use alloy_ui::{prelude::*, testing::Harness};

choices! { enum Show { All, Open, Done } }

#[derive(Default)]
struct Todos {
    draft: String,
    items: Vec<(String, bool)>,
    show: Show,
}

impl Todos {
    fn add(&mut self) {
        let t = self.draft.trim().to_string();
        if !t.is_empty() {
            self.items.push((t, false));
            self.draft.clear();
        }
    }
}

fn view(s: &Todos) -> impl View<Act<Todos>> {
    let rows = s.items.iter().enumerate().filter(|(_, t)| match s.show {
        Show::All => true,
        Show::Open => !t.1,
        Show::Done => t.1,
    });
    col![
        row![
            input(at!(s.draft)).hint("What needs doing?").submit(Todos::add).fill_w(),
            button("Add").primary().act(Todos::add),
        ]
        .gap(8.0),
        options(s.show).bind(|s: &mut Todos| &mut s.show),
        col(rows.map(|(i, t)| check(&t.0, t.1).act(move |s: &mut Todos, v| s.items[i].1 = v))),
    ]
    .gap(12.0)
    .pad(20.0)
}

fn main() {
    let mut ui = Harness::new(sketch(Todos::default(), view), 420.0, 320.0);
    ui.click("What needs doing?").type_text("Buy milk").click("Add");
    assert!(ui.has_text("Buy milk"));
    ui.click("Done");
    assert!(!ui.has_text("Buy milk"));
}
```

## 3. Search as you type

A search box is not application state. `local` keeps it with the widget tree, so the struct you persist stays about your data.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn view(names: &Vec<&'static str>) -> impl View<Act<Vec<&'static str>>> {
    let q = local(String::new());
    let hits = names.iter().filter(|n| n.to_lowercase().contains(&q.get().to_lowercase()));
    col![
        input(q.get()).local(&q).hint("Filter"),
        col(hits.map(|n| text(*n))),
    ]
    .gap(8.0)
    .pad(20.0)
}

fn main() {
    let names = vec!["Ada Lovelace", "Grace Hopper", "Alan Turing"];
    let mut ui = Harness::new(sketch(names, view), 360.0, 240.0);
    ui.click("Filter").type_text("gra");
    assert!(ui.has_text("Grace Hopper") && !ui.has_text("Alan Turing"));
}
```

## 4. A signup form that does not scold early

`field` shows a validation message only after the person has touched the control, and reserves the line so the form never jumps.

```rust
use alloy_ui::{prelude::*, valid};

#[derive(Default)]
struct Signup {
    name: String,
    email: String,
    password: String,
}

fn view(s: &Signup) -> impl View<Act<Signup>> {
    let short = valid::min_len(&s.password, 8);
    col![
        field("Name", input(at!(s.name)), valid::required(&s.name)),
        field("Email", input(at!(s.email)), valid::email(&s.email)),
        field("Password", input(at!(s.password)).secure(), short.as_deref()),
        button("Create account").primary().enabled(
            valid::required(&s.name).is_none()
                && valid::email(&s.email).is_none()
                && valid::min_len(&s.password, 8).is_none()
        ),
    ]
    .gap(14.0)
    .pad(24.0)
}
# fn main() { let _ = sketch(Signup::default(), view); }
```

## 5. A settings form with typed numbers

`number` accepts any `FromStr` number and refuses text that is not one, so the field never holds garbage. `stepper` and `slider` bind the same way.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Prefs {
    name: String,
    port: u16,
    ratio: f64,
    volume: f32,
    notify: bool,
}

fn view(s: &Prefs) -> impl View<Act<Prefs>> {
    col![
        field("Display name", input(at!(s.name)), None),
        field("Port", number(at!(s.port)), None),
        field("Ratio", number(at!(s.ratio)), None),
        field("Volume", slider(0.0..=11.0, at!(s.volume)).step(1.0), None),
        check("Send notifications", at!(s.notify)),
    ]
    .gap(14.0)
    .pad(24.0)
}
# fn main() { let _ = sketch(Prefs::default(), view); }
```
