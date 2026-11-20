use alloy_ui::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Focus,
    Short,
    Long,
}

struct Timer {
    mode: Mode,
    minutes: [f32; 3],
    left: u32,
    running: bool,
    done: u32,
    auto: bool,
}

impl Timer {
    fn total(&self) -> u32 {
        self.minutes[self.mode as usize] as u32 * 60
    }

    fn switch(&mut self, mode: Mode) {
        self.mode = mode;
        self.left = self.total();
    }

    fn reset(&mut self) {
        self.running = false;
        self.left = self.total();
    }

    fn tick(&mut self) {
        if !self.running {
            return;
        }
        self.left = self.left.saturating_sub(1);
        if self.left == 0 {
            let next = match self.mode {
                Mode::Focus => {
                    self.done += 1;
                    if self.done % 4 == 0 {
                        Mode::Long
                    } else {
                        Mode::Short
                    }
                }
                _ => Mode::Focus,
            };
            self.switch(next);
            self.running = self.auto;
        }
    }
}

fn view(s: &Timer) -> impl View<Act<Timer>> {
    let modes = [
        ("Focus", Mode::Focus),
        ("Short break", Mode::Short),
        ("Long break", Mode::Long),
    ];
    col![
        choose(modes, &s.mode).act(|s: &mut Timer, m| {
            s.switch(m);
            s.running = false;
        }),
        heading(format!("{:02}:{:02}", s.left / 60, s.left % 60)).display(),
        progress(0.0..=s.total() as f32, (s.total() - s.left) as f32),
        row![
            button(if s.running { "Pause" } else { "Start" })
                .act(|s: &mut Timer| s.running = !s.running),
            button("Reset").ghost().act(Timer::reset),
        ],
        format!("Sessions completed: {}", s.done),
        col(modes.map(|(name, m)| {
            row![
                text(name).w(110.0),
                slider(1.0..=60.0, s.minutes[m as usize])
                    .step(1.0)
                    .act(move |s: &mut Timer, v| {
                        s.minutes[m as usize] = v;
                        if m == s.mode && !s.running {
                            s.left = s.total();
                        }
                    }),
                text(format!("{} min", s.minutes[m as usize])).w(60.0),
            ]
        })),
        check("Auto-start next session", s.auto).bind(|s: &mut Timer| &mut s.auto),
    ]
    .gap(18.0)
    .pad(24.0)
    .align_center()
}

fn main() {
    let t = Timer {
        mode: Mode::Focus,
        minutes: [25.0, 5.0, 15.0],
        left: 25 * 60,
        running: false,
        done: 0,
        auto: false,
    };
    sketch(t, view)
        .tick(secs(1.0), Timer::tick)
        .run_sized("Pomodoro", 480.0, 560.0)
}
