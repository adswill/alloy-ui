//! Counter, Elm style: state, message enum, update, view.
use alloy_ui::prelude::*;

#[derive(Default)]
struct Counter(i64);

#[derive(Clone)]
enum Msg {
    Inc,
    Dec,
    Reset,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, m: Msg) {
        match m {
            Msg::Inc => self.0 += 1,
            Msg::Dec => self.0 -= 1,
            Msg::Reset => self.0 = 0,
        }
    }

    fn view(&self) -> impl View<Msg> {
        col![
            heading("Counter"),
            text(format!("{} so far", self.0)).dim(),
            row![
                button("Subtract").on(Msg::Dec),
                button("Add").primary().on(Msg::Inc),
                button("Reset").ghost().on(Msg::Reset),
            ]
            .align_center(),
        ]
        .gap(14.0)
        .pad(24.0)
        .fill()
    }
}

fn main() {
    Counter(0).run_sized("Counter", 360.0, 180.0)
}
