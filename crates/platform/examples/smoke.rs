//! Opens a window, waits for the first redraw and a cross-thread wake, opens a
//! second window, waits for it to paint, closes it, checks the frame counter,
//! then exits 0.
//! Run under Xvfb in CI: `xvfb-run cargo run -p alloy-ui-platform --example smoke`.
use alloy_ui_core::Size;
use alloy_ui_platform::shell::WindowKey;
use alloy_ui_platform::{run, PlatformEvent, ShellContext, ShellHandler, WindowSettings};

fn settings(title: &str) -> WindowSettings {
    WindowSettings {
        title: title.into(),
        size: Size::new(320.0, 200.0),
    }
}

#[derive(Default)]
struct H {
    main_painted: bool,
    woke: bool,
    second: Option<WindowKey>,
    second_painted: bool,
    second_closed: bool,
}

impl ShellHandler for H {
    fn started(&mut self, ctx: &mut ShellContext) {
        let p = ctx.proxy();
        std::thread::spawn(move || {
            p.wake();
        });
        self.second = Some(ctx.open_window(settings("second")));
    }
    fn event(&mut self, ctx: &mut ShellContext, e: PlatformEvent) {
        match e {
            PlatformEvent::Wake => self.woke = true,
            PlatformEvent::WindowClosed if Some(ctx.window_key()) == self.second => {
                self.second_closed = true;
                let s = ctx.frame_stats();
                assert!(s.frames >= 2, "frames counted: {}", s.frames);
                assert!(s.first_paint.is_some(), "first paint recorded");
                println!(
                    "ok: 2 windows, {} frames, first paint {:?}",
                    s.frames,
                    s.first_paint.unwrap()
                );
            }
            _ => {}
        }
        self.check(ctx);
    }
    fn redraw(&mut self, ctx: &mut ShellContext) {
        let key = ctx.window_key();
        if key == WindowKey::MAIN {
            self.main_painted = true;
        } else if Some(key) == self.second {
            self.second_painted = true;
        }
        self.check(ctx);
    }
}

impl H {
    fn check(&mut self, ctx: &mut ShellContext) {
        if self.main_painted && self.woke && self.second_painted && !self.second_closed {
            if let Some(k) = self.second {
                ctx.close_window(k);
            }
        }
        if self.second_closed {
            println!("ok: redraw + wake + close second window");
            ctx.exit();
        }
    }
}

fn main() {
    run(settings("smoke"), |_| H::default()).expect("shell");
}
