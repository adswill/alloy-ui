//! Forms: validated hand-built fields, a multi-step wizard, and a CRUD screen from one derive.
use crate::ui::*;
use crate::Studio;
use alloy_ui::prelude::*;
use alloy_ui::{i18n, valid};

choices! { pub enum Plan { Free, Pro, Team } }
choices! { pub enum Role { Engineer, Designer, Manager, Founder } }

#[derive(Default)]
pub struct Profile {
    name: String,
    email: String,
    bio: String,
    role: Role,
    newsletter: bool,
    digest: bool,
    volume: f32,
}

const PRICES: [u32; 3] = [0, 12, 40];

#[derive(Record, Clone, Default)]
struct Teammate {
    name: String,
    email: String,
    role: Role,
    #[form(range = (40.0, 200.0), step = 5.0)]
    salary: f32,
    active: bool,
}

fn profile(s: &Studio) -> N {
    let f = &s.form;
    let tried = local(false);
    let ok = valid::required(&f.name).is_none() && valid::email(&f.email).is_none();
    col![
        field_after(
            "Full name",
            input(at!(s.form.name)).hint("Ada Lovelace"),
            valid::required(&f.name),
            tried.get()
        ),
        field_after(
            "Email",
            input(at!(s.form.email)).hint("ada@example.com"),
            valid::email(&f.email),
            tried.get()
        ),
        field(
            "Role",
            options(f.role).bind(|s: &mut Studio| &mut s.form.role),
            None::<&str>
        ),
        field("Short bio", editor(at!(s.form.bio)).h(96.0), None::<&str>),
        row![
            toggle("Product newsletter", at!(s.form.newsletter)),
            check("Weekly digest", at!(s.form.digest)),
        ]
        .gap(24.0),
        field(
            "Notification volume",
            row![
                slider(0.0..=100.0, at!(s.form.volume)).step(5.0).w(260.0),
                caption(format!("{:.0}%", s.form.volume)).dim(),
            ]
            .gap(12.0)
            .align_center(),
            None::<&str>
        ),
        row![
            button("Save profile").primary().act(move |_: &mut Studio| {
                if ok {
                    toast("Profile saved");
                } else {
                    tried.set(true);
                    toast("Fix the highlighted fields first").error();
                }
            }),
            caption("Errors appear after you leave a field, or after a failed save.").dim(),
        ]
        .gap(12.0)
        .align_center(),
    ]
    .gap(14.0)
    .card()
    .pad(24.0)
    .w(560.0)
    .into()
}

fn signup(s: &Studio) -> N {
    let price = PRICES[s.plan as usize] * if s.yearly { 10 } else { 12 } / 12;
    wizard(at!(s.step))
        .step(
            "Account",
            col![
                field(
                    "Name",
                    input(at!(s.name)).hint("Ada Lovelace"),
                    valid::required(&s.name)
                ),
                field(
                    "Email",
                    input(at!(s.email)).hint("ada@example.com"),
                    valid::email(&s.email)
                ),
                field(
                    "Password",
                    input(at!(s.password)).secure(),
                    valid::password(&s.password)
                ),
            ],
        )
        .page(
            "Plan",
            true,
            col![
                radio_list_with(at!(s.plan), |p| format!("${}/mo", PRICES[p as usize])),
                check("Bill yearly (2 months free)", at!(s.yearly)),
            ],
        )
        .page(
            "Review",
            s.terms,
            col![
                format!("Name: {}", s.name),
                format!("Email: {}", s.email),
                format!(
                    "Plan: {:?} at ${price}/mo, billed {}",
                    s.plan,
                    if s.yearly { "yearly" } else { "monthly" }
                ),
                check("I accept the terms", at!(s.terms)),
            ],
        )
        .finish("Create account")
        .done(col![heading("You're all set"), format!("Welcome, {}.", s.name)].align_center())
        .pad(24.0)
        .into()
}

fn team() -> N {
    crud(|| {
        Records::<Teammate>::of([
            vals!["Ada Lovelace", "ada@acme.dev", Role::Engineer, 150, true],
            vals!["Grace Hopper", "grace@acme.dev", Role::Manager, 170, true],
            vals![
                "Margaret Hamilton",
                "margaret@acme.dev",
                Role::Founder,
                190,
                true
            ],
            vals!["Hedy Lamarr", "hedy@acme.dev", Role::Designer, 120, false],
        ])
    })
}

pub fn view(s: &Studio) -> N {
    let body: N = match s.forms_tab {
        0 => profile(s),
        1 => signup(s),
        _ => team(),
    };
    fixed(col![
        header(
            i18n::translate("fm.title", &[]),
            i18n::translate("fm.sub", &[]),
            text("")
        ),
        tab_bar(["Profile", "Wizard", "Team (CRUD)"], at!(s.forms_tab)),
        scroll(col![body].pad_xy(0.0, 4.0)).fill_h(),
    ])
}
