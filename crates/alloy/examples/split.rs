use alloy_ui::prelude::*;

state! {
    struct Split {
        bill: f64 = 84.5,
        tip: f32 = 15.0,
        people: u32 = 2,
        round_up: bool,
    }
}

impl Split {
    fn tip_amount(&self) -> f64 {
        self.bill * self.tip as f64 / 100.0
    }

    fn share(&self) -> f64 {
        let share = (self.bill + self.tip_amount()) / self.people as f64;
        if self.round_up {
            share.ceil()
        } else {
            share
        }
    }
}

fn view(s: &Split) -> impl View<Act<Split>> {
    col![
        field("Bill", number(at!(s.bill)).hint("0.00"), None),
        slider(0.0..=30.0, at!(s.tip))
            .step(1.0)
            .label("Tip")
            .unit("%"),
        stepper(at!(s.people)).min(1).labelled("People"),
        check("Round each share up", at!(s.round_up)),
        col![
            format!("Tip: {}", money(s.tip_amount())),
            format!("Total: {}", money(s.bill + s.tip_amount())),
            heading(format!("Each pays {}", money(s.share()))),
        ]
        .card(),
    ]
    .page()
}

fn main() {
    sketch(Split::default(), view).run_sized("Split the bill", 480.0, 520.0)
}
