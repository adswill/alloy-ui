//! Opens a window, waits for the first redraw and a cross-thread wake, then exits 0.
//! Run under Xvfb in CI: `xvfb-run cargo run -p alloy-ui-platform --example smoke`.
use alloy_ui_core::Size;
use alloy_ui_platform::{run, PlatformEvent, ShellContext, ShellHandler, WindowSettings};

struct H {
    redrawn: bool,
    woke: bool,
}

impl ShellHandler for H {
    fn started(&mut self, ctx: &mut ShellContext) {
        let p = ctx.proxy();
        std::thread::spawn(move || {
            p.wake();
        });
    }
    fn event(&mut self, ctx: &mut ShellContext, e: PlatformEvent) {
        if e == PlatformEvent::Wake {
            self.woke = true;
        }
        if self.redrawn && self.woke {
            println!("ok: redraw + wake");
            ctx.exit();
        }
    }
    fn redraw(&mut self, ctx: &mut ShellContext) {
        self.redrawn = true;
        if self.woke {
            ctx.exit();
        }
    }
}

fn main() {
    let s = WindowSettings {
        title: "smoke".into(),
        size: Size::new(320.0, 200.0),
    };
    run(s, |_| H {
        redrawn: false,
        woke: false,
    })
    .expect("shell");
}
