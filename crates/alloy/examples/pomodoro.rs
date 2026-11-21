use alloy_ui::prelude::*;

choices! { enum Mode { Focus, Short = "Short break", Long = "Long break" } }

state! {
    struct Timer {
        mode: Mode,
        minutes: [f32; 3] = [25.0, 5.0, 15.0],
        elapsed: u32,
        running: bool,
        done: u32,
        auto: bool,
    }
}

impl Timer {
    fn total(&self) -> u32 {
        self.minutes[self.mode as usize] as u32 * 60
    }

    fn switch(&mut self, mode: Mode) {
        self.mode = mode;
        self.elapsed = 0;
        self.running = false;
    }

    fn tick(&mut self) {
        self.elapsed += 1;
        if self.elapsed >= self.total() {
            self.done += (self.mode == Mode::Focus) as u32;
            self.switch(match self.mode {
                Mode::Focus if self.done % 4 == 0 => Mode::Long,
                Mode::Focus => Mode::Short,
                _ => Mode::Focus,
            });
            self.running = self.auto;
        }
    }
}

fn view(s: &Timer) -> impl View<Act<Timer>> {
    let left = s.total() - s.elapsed;
    col![
        options(at!(s.mode)).act(Timer::switch),
        heading(clock(left)).display(),
        progress(0.0..=s.total() as f32, s.elapsed as f32),
        row![
            button(if s.running { "Pause" } else { "Start" }).toggle(at!(s.running)),
            button("Reset")
                .ghost()
                .act(|s: &mut Timer| s.switch(s.mode)),
        ],
        format!("Sessions completed: {}", s.done),
        col(Mode::all().iter().map(|&(name, m)| {
            slider(1.0..=60.0, at!(s.minutes[m as usize]))
                .step(1.0)
                .label(name)
                .unit("min")
        })),
        check("Auto-start next session", at!(s.auto)),
    ]
    .page()
    .align_center()
}

fn main() {
    sketch(Timer::default(), view)
        .tick_while(|t| t.running, secs(1.0), Timer::tick)
        .run_sized("Pomodoro", 480.0, 560.0)
}
