use alloy_ui::prelude::*;

struct Split {
    bill: f64,
    tip: f32,
    people: u32,
    round_up: bool,
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
            format!("Tip: ${:.2}", s.tip_amount()),
            format!("Total: ${:.2}", s.bill + s.tip_amount()),
            heading(format!("Each pays ${:.2}", s.share())),
        ]
        .card(),
    ]
    .page()
}

fn main() {
    let split = Split {
        bill: 84.5,
        tip: 15.0,
        people: 2,
        round_up: false,
    };
    sketch(split, view).run_sized("Split the bill", 480.0, 520.0)
}
