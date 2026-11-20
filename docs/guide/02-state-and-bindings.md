# State and bindings

Your state is an ordinary Rust value. The view reads it; handlers change it.

## Handlers

Most widgets take a closure over `&mut State`:

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Todo {
    draft: String,
    tasks: Vec<(String, bool)>,
}

impl Todo {
    fn add(&mut self) {
        let t = self.draft.trim().to_string();
        if !t.is_empty() {
            self.tasks.push((t, false));
            self.draft.clear();
        }
    }
}

fn view(s: &Todo) -> impl View<Act<Todo>> {
    col![
        row![
            input(&s.draft)
                .hint("What needs doing?")
                .bind(|s: &mut Todo| &mut s.draft)
                .submit(Todo::add),
            button("Add").primary().act(Todo::add),
        ],
        col(s.tasks.iter().enumerate().map(|(i, t)| {
            check(&t.0, t.1).act(move |s: &mut Todo, v| s.tasks[i].1 = v)
        })),
    ]
    .gap(12.0)
}
# fn main() { let _ = sketch(Todo::default(), view); }
```

`Act<S>` is the message type of a `sketch`: a boxed closure over `&mut S`. Your view function returns `impl View<Act<State>>`.

## Two-way binding with `at!`

`.bind(|s: &mut Todo| &mut s.draft)` is the long form. `at!` is the short one: give it the field, written as a path on the `s` your view received.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Form {
    name: String,
    dark: bool,
    volume: f32,
    scores: Vec<f32>,
}

fn view(s: &Form) -> impl View<Act<Form>> {
    col![
        input(at!(s.name)).hint("Name"),
        check("Dark", at!(s.dark)),
        slider(0.0..=11.0, at!(s.volume)).step(1.0),
        slider(0.0..=10.0, at!(s.scores[0])),
    ]
    .gap(8.0)
}
# fn main() { let _ = sketch(Form { scores: vec![0.0], ..Default::default() }, view); }
```

`at!` works for nested fields (`at!(s.user.email)`) and indexing (`at!(s.scores[i])`). It reads the current value and writes back through the same path, so there is nothing to keep in sync. Use it with `input`, `check`, `slider`, `radios`, `options`, `nav`, `number`, `stepper`, and `button(..).toggle(..)`.

## Typed numbers

`number(at!(s.price))` is a text field for any `FromStr` number; text that is not a number is refused as you type, so the field never holds garbage.

## State that is not app state

A search box, a sort order or an open panel does not belong in your state struct. `local` keeps a value with the widget tree, keyed by where you wrote it:

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    let query = local(String::new());
    let open = local(false);
    col![
        input(query.get()).local(&query).hint("Filter"),
        button(if open.get() { "Hide" } else { "Show" }).act(move |_| open.toggle()),
        text(format!("filtering by {:?}", query.get())),
    ]
}
# fn main() { let _ = sketch((), view); }
```

`Local<T>` is a small `Copy` handle, so closures capture it with no `.clone()`. `store(|| init)` is the same but saved between runs when the app uses `.persist()`.

## Keeping state between runs

`sketch(..).persist("name")` loads the state from the user's config directory at startup (keeping your initial value if nothing is saved) and writes it after every change. Your state needs `serde::Serialize + Deserialize`.

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Prefs {
    dark: bool,
}

fn main() {
    sketch(Prefs::default(), |p| check("Dark mode", at!(p.dark)))
        .persist("prefs-demo")
        .run("Prefs")
}
```
