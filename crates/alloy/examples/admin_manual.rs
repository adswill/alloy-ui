//! Admin panel: people table (search, sort, select, delete), edit form with validation,
//! settings page, everything persisted between runs.
use alloy_ui::prelude::*;
use serde::{Deserialize, Serialize};

alloy_ui::choices! { enum Page { People, Settings } }

const ROLES: [&str; 4] = ["Engineer", "Designer", "Manager", "Support"];

#[derive(Clone, Serialize, Deserialize)]
struct Person {
    name: String,
    email: String,
    role: usize,
    salary: f32,
    active: bool,
}

#[derive(Serialize, Deserialize)]
struct Admin {
    page: Page,
    people: Vec<Person>,
    sel: Option<usize>,
    dark: bool,
    compact: bool,
}

fn view(s: &Admin) -> impl View<Act<Admin>> {
    let q = local(String::new());
    let body: Node<_> = match s.page {
        Page::People => {
            let table = table(&s.people)
                .col("Name", |p| p.name.clone())
                .col("Role", |p| ROLES[p.role].into())
                .num("Salary", |p| format!("${:.0}k", p.salary))
                .col("Status", |p| {
                    if p.active { "Active" } else { "Away" }.into()
                })
                .filter(&q.get())
                .selected(s.sel)
                .on_select(|s: &mut Admin, i| s.sel = Some(i))
                .action("Delete", |s: &mut Admin, i| {
                    s.people.remove(i);
                    s.sel = None;
                });
            let detail = s.sel.and_then(|i| s.people.get(i)).map(|p| {
                let i = s.sel.unwrap();
                col![
                    heading(&p.name),
                    field(
                        "Name",
                        input(&p.name).bind(move |s: &mut Admin| &mut s.people[i].name),
                        p.name.is_empty().then_some("Required")
                    ),
                    field(
                        "Email",
                        input(&p.email).bind(move |s: &mut Admin| &mut s.people[i].email),
                        (!p.email.contains('@')).then_some("Enter a valid email")
                    ),
                    field(
                        "Role",
                        pick(ROLES, Some(p.role)).bind(move |s: &mut Admin| &mut s.people[i].role),
                        None
                    ),
                    field(
                        "Salary",
                        slider(40.0..=200.0, p.salary)
                            .step(5.0)
                            .bind(move |s: &mut Admin| &mut s.people[i].salary),
                        None
                    ),
                    check("Active", p.active).bind(move |s: &mut Admin| &mut s.people[i].active),
                ]
                .gap(14.0)
                .pad(20.0)
                .card()
                .w(300.0)
            });
            col![
                row![
                    heading("People"),
                    space(),
                    input(q.get()).hint("Search").local(&q).w(220.0),
                    button("Add person").primary().act(|s: &mut Admin| {
                        s.people.push(Person {
                            name: "New person".into(),
                            email: String::new(),
                            role: 0,
                            salary: 80.0,
                            active: true,
                        });
                        s.sel = Some(s.people.len() - 1);
                    }),
                ]
                .align_center()
                .gap(12.0)
                .fill_w(),
                row![
                    table,
                    detail.map(Node::from).unwrap_or_else(|| gap(0.0, 0.0))
                ]
                .gap(16.0)
                .fill(),
            ]
            .gap(16.0)
            .pad(24.0)
            .fill()
            .into()
        }
        Page::Settings => col![
            heading("Settings"),
            section(
                "Theme",
                choose([("Light", false), ("Dark", true)], s.dark).act(|s: &mut Admin, d| {
                    s.dark = d;
                    set_dark(d);
                })
            ),
            check("Compact rows", s.compact).bind(|s: &mut Admin| &mut s.compact),
        ]
        .gap(20.0)
        .pad(28.0)
        .fill()
        .into(),
    };
    row![
        nav(&s.page)
            .bind(|s: &mut Admin| &mut s.page)
            .surface()
            .w(170.0)
            .fill_h(),
        body
    ]
    .gap(0.0)
    .fill()
}

fn main() {
    let p = |n: &str, r, sal, a| Person {
        name: n.into(),
        email: format!("{}@acme.dev", n.to_lowercase().replace(' ', ".")),
        role: r,
        salary: sal,
        active: a,
    };
    let people = vec![
        p("Ada Lovelace", 0, 150.0, true),
        p("Grace Hopper", 2, 170.0, true),
        p("Linus Pauling", 3, 90.0, false),
        p("Dieter Rams", 1, 120.0, true),
    ];
    sketch(
        Admin {
            page: Page::People,
            people,
            sel: None,
            dark: false,
            compact: false,
        },
        view,
    )
    .persist("alloy-admin-manual")
    .run_sized("Admin", 1200.0, 560.0)
}
