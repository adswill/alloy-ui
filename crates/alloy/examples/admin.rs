//! Admin panel: people table (search, sort, select, delete), edit form with validation,
//! settings page, state persisted between runs.
use alloy_ui::prelude::*;

choices! { enum Role { Engineer, Designer, Manager, Support } }

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
                    vals!["Linus Pauling", "linus@acme.dev", Role::Support, 90, false],
                    vals!["Dieter Rams", "dieter@acme.dev", Role::Designer, 120, true],
                ])
            })
        })
        .page("Settings", || section("Theme", theme_switch()))
        .persist()
        .run()
}
