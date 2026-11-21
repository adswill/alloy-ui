//! Applies each window effect in turn and prints what the OS made of it, then keeps `Blur`
//! on for a few seconds so a tool (`xprop -id <printed id>`) can inspect the window.
//! Run: `cargo run -p alloy-ui-platform --example effect`.
use alloy_ui_core::{Size, WindowEffect};
use alloy_ui_platform::{
    request_window_effect, run, PlatformEvent, ShellContext, ShellHandler, WindowSettings,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

struct H(std::time::Instant);

impl ShellHandler for H {
    fn started(&mut self, ctx: &mut ShellContext) {
        for e in [
            WindowEffect::None,
            WindowEffect::Transparent,
            WindowEffect::Mica,
            WindowEffect::Acrylic,
            WindowEffect::Blur,
        ] {
            println!("{e:?} -> {:?}", ctx.set_effect(e));
        }
        if let Ok(h) = ctx.window().window_handle() {
            match h.as_raw() {
                RawWindowHandle::Xlib(x) => println!("window 0x{:x}", x.window),
                RawWindowHandle::Xcb(x) => println!("window 0x{:x}", x.window.get()),
                other => println!("window {other:?}"),
            }
        }
        let proxy = ctx.proxy();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(4));
            proxy.wake();
        });
    }
    fn event(&mut self, ctx: &mut ShellContext, event: PlatformEvent) {
        // Other wakes (the OS settings arriving) come first: only exit after the delay.
        if event == PlatformEvent::Wake && self.0.elapsed().as_secs() >= 4 {
            ctx.exit();
        }
    }
    fn redraw(&mut self, _: &mut ShellContext) {}
}

fn main() {
    request_window_effect(WindowEffect::Blur);
    let settings = WindowSettings {
        title: "effect".into(),
        size: Size::new(300.0, 200.0),
    };
    run(settings, |_| H(std::time::Instant::now())).expect("run");
}
