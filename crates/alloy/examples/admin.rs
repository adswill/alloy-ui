//! Admin panel: people table (search, sort, select, delete), edit form with validation,
//! settings page, state persisted between runs.
use alloy_ui::{prelude::*, vals};
use serde::{Deserialize, Serialize};

alloy_ui::choices! { #[derive(Serialize, Deserialize)] enum Page { People, Settings } }

const ROLES: [&str; 4] = ["Engineer", "Designer", "Manager", "Support"];

#[derive(Record, Clone, Default, Serialize, Deserialize)]
struct Person {
    #[form(required)]
    name: String,
    #[form(email)]
    email: String,
    #[form(choices = ROLES)]
    role: String,
    #[form(range = (40.0, 200.0), step = 5.0)]
    salary: f32,
    active: bool,
}

#[derive(Serialize, Deserialize)]
struct Admin {
    page: Page,
    people: Records<Person>,
}

fn view(s: &Admin) -> impl View<Act<Admin>> {
    let body = match s.page {
        Page::People => records("People", &s.people, |s: &mut Admin| &mut s.people),
        Page::Settings => col![heading("Settings"), section("Theme", theme_switch())]
            .gap(20.0)
            .pad(28.0)
            .into(),
    };
    shell(&s.page, |s: &mut Admin| &mut s.page, body)
}

fn main() {
    let people = Records::of([
        vals!["Ada Lovelace", "ada@acme.dev", "Engineer", 150, true],
        vals!["Grace Hopper", "grace@acme.dev", "Manager", 170, true],
        vals!["Linus Pauling", "linus@acme.dev", "Support", 90, false],
        vals!["Dieter Rams", "dieter@acme.dev", "Designer", 120, true],
    ]);
    sketch(
        Admin {
            page: Page::People,
            people,
        },
        view,
    )
    .persist("alloy-admin")
    .run_sized("Admin", 1200.0, 560.0)
}
