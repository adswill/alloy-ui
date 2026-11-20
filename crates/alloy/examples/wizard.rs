use alloy_ui::{prelude::*, valid};

choices! { enum Plan { Free, Pro, Team } }
const PRICES: [u32; 3] = [0, 12, 40];

#[derive(Default)]
struct Wizard {
    step: usize,
    name: String,
    email: String,
    password: String,
    plan: Plan,
    yearly: bool,
    terms: bool,
}

fn view(s: &Wizard) -> impl View<Act<Wizard>> {
    let short = valid::min_len(&s.password, 8);
    let errors = (
        valid::required(&s.name),
        valid::email(&s.email),
        short.as_deref(),
    );
    let price = PRICES[s.plan as usize] * if s.yearly { 10 } else { 12 } / 12;
    wizard(at!(s.step))
        .page(
            "Account",
            errors == (None, None, None),
            col![
                field("Name", input(at!(s.name)).hint("Ada Lovelace"), errors.0),
                field(
                    "Email",
                    input(at!(s.email)).hint("ada@example.com"),
                    errors.1
                ),
                field("Password", input(at!(s.password)).secure(), errors.2),
            ],
        )
        .page(
            "Plan",
            true,
            col![
                radios(
                    at!(s.plan),
                    Plan::all()
                        .iter()
                        .map(|&(n, p)| (format!("{n} (${}/mo)", PRICES[p as usize]), p))
                ),
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
        .pad(28.0)
}

fn main() {
    let wizard = Wizard {
        plan: Plan::Pro,
        ..Default::default()
    };
    sketch(wizard, view).run_sized("Sign up", 520.0, 520.0)
}
