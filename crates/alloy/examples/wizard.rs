use alloy_ui::prelude::*;

choices! { enum Plan { Free, Pro, Team } }
choices! { enum Step { Account, Plan, Review, Done } }

#[derive(Default)]
struct Wizard {
    step: Step,
    name: String,
    email: String,
    password: String,
    plan: Plan,
    yearly: bool,
    terms: bool,
}

impl Plan {
    fn price(self) -> u32 {
        [0, 12, 40][self as usize]
    }
}

impl Wizard {
    fn email_error(&self) -> Option<&'static str> {
        match &self.email {
            e if e.is_empty() => Some("Required"),
            e if !e.contains('@') || !e.contains('.') => Some("Enter a valid email"),
            _ => None,
        }
    }

    fn password_error(&self) -> Option<&'static str> {
        (self.password.len() < 8).then_some("At least 8 characters")
    }

    fn valid(&self) -> bool {
        match self.step {
            Step::Account => {
                !self.name.trim().is_empty()
                    && self.email_error().is_none()
                    && self.password_error().is_none()
            }
            Step::Review => self.terms,
            _ => true,
        }
    }

    fn go(&mut self, to: Step) {
        self.step = to;
    }
}

fn view(s: &Wizard) -> impl View<Act<Wizard>> {
    let steps = [Step::Account, Step::Plan, Step::Review];
    let page: Node<Act<Wizard>> = match s.step {
        Step::Account => col![
            field(
                "Name",
                input(&s.name)
                    .hint("Ada Lovelace")
                    .bind(|s: &mut Wizard| &mut s.name),
                s.name.trim().is_empty().then_some("Required")
            ),
            field(
                "Email",
                input(&s.email)
                    .hint("ada@example.com")
                    .bind(|s: &mut Wizard| &mut s.email),
                s.email_error()
            ),
            field(
                "Password",
                input(&s.password)
                    .secure()
                    .bind(|s: &mut Wizard| &mut s.password),
                s.password_error()
            ),
        ]
        .gap(14.0)
        .into(),
        Step::Plan => col![
            col(Plan::all().iter().map(|&(name, p)| {
                radio(format!("{name} (${}/mo)", p.price()), s.plan == p)
                    .on(Act::new(move |s: &mut Wizard| s.plan = p))
            })),
            check("Bill yearly (2 months free)", s.yearly).bind(|s: &mut Wizard| &mut s.yearly),
        ]
        .gap(14.0)
        .into(),
        Step::Review => col![
            format!("Name: {}", s.name),
            format!("Email: {}", s.email),
            format!(
                "Plan: {:?} at ${}/mo, billed {}",
                s.plan,
                s.plan.price() * if s.yearly { 10 } else { 12 } / 12,
                if s.yearly { "yearly" } else { "monthly" }
            ),
            check("I accept the terms", s.terms).bind(|s: &mut Wizard| &mut s.terms),
        ]
        .into(),
        Step::Done => col![heading("You're all set"), format!("Welcome, {}.", s.name)]
            .align_center()
            .into(),
    };
    let i = s.step as usize;
    col![
        row(steps
            .iter()
            .enumerate()
            .map(|(n, st)| text(format!("{}. {st:?}", n + 1)).accent_if(n == i)))
        .gap(24.0),
        page,
        row![
            button("Back")
                .enabled(i == 1 || i == 2)
                .act(move |s: &mut Wizard| s.go(steps[i.saturating_sub(1)])),
            space(),
            button(if i == 2 { "Create account" } else { "Next" })
                .primary()
                .enabled(s.valid())
                .act(move |s: &mut Wizard| s.go(Step::all()[i + 1].1)),
        ]
        .fill_w(),
    ]
    .gap(24.0)
    .pad(28.0)
}

fn main() {
    sketch(
        Wizard {
            plan: Plan::Pro,
            ..Default::default()
        },
        view,
    )
    .run_sized("Sign up", 520.0, 520.0)
}
