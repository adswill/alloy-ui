# Records and forms

## `derive(Record)`: a CRUD screen in one call

A struct that derives `Record` knows its fields, their types and how to edit them. `crud` builds a searchable, sortable table with Add and Delete and a validated edit form for the selected row:

```rust,no_run
use alloy_ui::prelude::*;

choices! { enum Role { Engineer, Designer, Manager } }

#[derive(Record, Clone, Default)]
struct Person {
    name: String,
    email: String,
    role: Role,
    #[form(range = (40.0, 200.0), step = 5.0)]
    salary: f32,
    active: bool,
}

fn main() {
    app("Admin")
        .page("People", || {
            crud(|| {
                Records::<Person>::of([
                    vals!["Ada Lovelace", "ada@acme.dev", Role::Engineer, 150, true],
                    vals!["Grace Hopper", "grace@acme.dev", Role::Manager, 170, true],
                ])
            })
        })
        .page("Settings", || section("Theme", theme_switch()))
        .persist()
        .run()
}
```

Field kinds follow the Rust type: `String` is a text field, a field called `email` gets email validation, `choices!` enums become a picker, numbers a slider (`#[form(range = .., step = ..)]`), `bool` a checkbox, `#[form(long)]` a multi-line editor and `#[form(skip)]` hides a field. `app(..).page(..)` gives you a sidebar of pages; `.persist()` saves every `store` and the open page between runs.

## Hand-built forms

Where a derive does not fit, build the form from `field`. A `field` is a label, a control and an optional error message:

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
        field("Name", input(at!(s.name)).hint("Ada Lovelace"), valid::required(&s.name)),
        field("Email", input(at!(s.email)), valid::email(&s.email)),
        field("Password", input(at!(s.password)).secure(), short.as_deref()),
    ]
    .gap(14.0)
}
# fn main() { let _ = sketch(Signup::default(), view); }
```

The validators in `alloy_ui::valid` (`required`, `email`, `min_len`) return the message to show, or `None`.

### When errors appear

A form that opens with a red "Required" under every empty field is hostile, so `field` hides its message until the person has **touched** the control: focused it and then left it (by Tab or by clicking elsewhere). The line for the message is reserved from the start, so the form does not jump when the message appears. While someone is still typing in a field, you never scold them for an unfinished address.

To also show every error after a failed submit, pass the attempt in with `field_after` and keep it in a `local`:

```rust
use alloy_ui::{prelude::*, valid};

#[derive(Default)]
struct Login {
    user: String,
}

fn view(s: &Login) -> impl View<Act<Login>> {
    let tried = local(false);
    col![
        field_after("User", input(at!(s.user)), valid::required(&s.user), tried.get()),
        button("Sign in")
            .primary()
            .enabled(true)
            .act(move |s: &mut Login| {
                if valid::required(&s.user).is_some() {
                    tried.set(true);
                }
            }),
    ]
    .gap(12.0)
}
# fn main() { let _ = sketch(Login::default(), view); }
```

## Wizards

`wizard(at!(s.step))` is a multi-page form whose Next button stays disabled until the page says it is valid:

```rust
use alloy_ui::{prelude::*, valid};

#[derive(Default)]
struct Wizard {
    step: usize,
    name: String,
    terms: bool,
}

fn view(s: &Wizard) -> impl View<Act<Wizard>> {
    wizard(at!(s.step))
        .page(
            "Account",
            valid::required(&s.name).is_none(),
            col![field("Name", input(at!(s.name)), valid::required(&s.name))],
        )
        .page("Review", s.terms, col![check("I accept the terms", at!(s.terms))])
        .finish("Create account")
        .done(col![heading("You're all set"), format!("Welcome, {}.", s.name)])
        .pad(28.0)
}
# fn main() { let _ = sketch(Wizard::default(), view); }
```
