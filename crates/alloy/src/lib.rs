//! Alloy facade: wires the runtime, platform shell, renderer and task runner
//! together so an app is just an [`Application`] and a call to [`run`].
#![warn(missing_docs)]

pub use alloy_ui_core::*;
pub use alloy_ui_ipc as ipc;
pub use alloy_ui_layout as layout;
pub use alloy_ui_platform as platform;
pub use alloy_ui_render as render;
pub use alloy_ui_theme as theme;

use alloy_ui_ipc::TaskRunner;
use alloy_ui_platform::{PlatformError, PlatformEvent, ShellContext, ShellHandler, WindowSettings};
use alloy_ui_render::{Renderer, WindowRenderer};

/// Run `app` in a native window until it exits.
///
/// `layouter` resolves a view tree to absolute layout (the widgets crate
/// provides one). Must be called on the main thread.
pub fn run<A>(
    settings: WindowSettings,
    app: A,
    layouter: Layouter<A::Message>,
) -> Result<(), PlatformError>
where
    A: Application + 'static,
{
    let size = settings.size;
    alloy_ui_platform::run(settings, move |proxy| Host {
        pending: Some((app, size, layouter)),
        runtime: None,
        renderer: None,
        tasks: TaskRunner::new(move || {
            proxy.wake();
        }),
    })
}

type Pending<A> = (A, Size, Layouter<<A as Application>::Message>);

struct Host<A: Application> {
    pending: Option<Pending<A>>,
    runtime: Option<Runtime<A>>,
    renderer: Option<WindowRenderer>,
    tasks: TaskRunner<A::Message>,
}

impl<A: Application + 'static> Host<A> {
    fn pump(&mut self, ctx: &mut ShellContext, outcome: Outcome) {
        if outcome.exit {
            ctx.exit();
            return;
        }
        if outcome.redraw {
            ctx.request_redraw();
        }
        if let Some(rt) = self.runtime.as_mut() {
            for fut in rt.take_futures() {
                self.tasks.spawn(fut);
            }
        }
    }
}

impl<A: Application + 'static> ShellHandler for Host<A> {
    fn started(&mut self, ctx: &mut ShellContext) {
        let (app, _, layouter) = self.pending.take().expect("started once");
        let size = ctx.logical_size();
        let (w, h) = ctx.physical_size();
        match WindowRenderer::new(ctx.window().clone(), w, h) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                log::error!("renderer init failed: {e}");
                ctx.exit();
                return;
            }
        }
        let mut rt = Runtime::new(app, size, layouter);
        for fut in rt.take_futures() {
            self.tasks.spawn(fut);
        }
        self.runtime = Some(rt);
        ctx.request_redraw();
    }

    fn event(&mut self, ctx: &mut ShellContext, event: PlatformEvent) {
        let Some(rt) = self.runtime.as_mut() else {
            return;
        };
        let outcome = match event {
            PlatformEvent::Input(Event::Resized(s)) => {
                let (w, h) = ctx.physical_size();
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(w, h);
                }
                rt.handle_event(&Event::Resized(s))
            }
            PlatformEvent::Input(e) => rt.handle_event(&e),
            PlatformEvent::Wake => {
                let msgs = self.tasks.drain();
                if msgs.is_empty() {
                    return;
                }
                rt.dispatch_all(msgs)
            }
            PlatformEvent::ScaleFactorChanged(_) => Outcome {
                redraw: true,
                exit: false,
            },
            _ => return,
        };
        self.pump(ctx, outcome);
    }

    fn redraw(&mut self, ctx: &mut ShellContext) {
        let (Some(rt), Some(r)) = (self.runtime.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        r.render(&rt.draw(), ctx.scale_factor() as f32);
    }
}
