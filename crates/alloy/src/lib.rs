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
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

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
    alloy_ui_platform::run(settings, move |proxy| {
        let task_proxy = proxy.clone();
        let (sub_tx, sub_rx) = mpsc::channel();
        Host {
            pending: Some((app, size, layouter)),
            runtime: None,
            renderer: None,
            tasks: TaskRunner::new(move || {
                task_proxy.wake();
            }),
            proxy,
            start: Instant::now(),
            tick_due: Arc::new(AtomicBool::new(false)),
            tick_pending: false,
            subs: HashMap::new(),
            sub_rx,
            sub_tx,
        }
    })
}

type Pending<A> = (A, Size, Layouter<<A as Application>::Message>);

struct Host<A: Application> {
    pending: Option<Pending<A>>,
    runtime: Option<Runtime<A>>,
    renderer: Option<WindowRenderer>,
    tasks: TaskRunner<A::Message>,
    proxy: alloy_ui_platform::shell::ShellProxy,
    start: Instant,
    tick_due: Arc<AtomicBool>,
    tick_pending: bool,
    subs: HashMap<u64, Arc<AtomicBool>>,
    sub_rx: mpsc::Receiver<A::Message>,
    sub_tx: mpsc::Sender<A::Message>,
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
        if outcome.tick && !self.tick_pending {
            // Animation is active: one short-lived timer thread per frame, so an
            // idle app has no timer at all.
            self.tick_pending = true;
            let (due, proxy) = (self.tick_due.clone(), self.proxy.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(16));
                due.store(true, Ordering::Release);
                proxy.wake();
            });
        }
        if let Some(rt) = self.runtime.as_mut() {
            for fut in rt.take_futures() {
                self.tasks.spawn(fut);
            }
        }
        self.sync_subscriptions();
    }

    /// Start new timers and stop those no longer requested.
    fn sync_subscriptions(&mut self) {
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        let wanted = rt.subscriptions();
        let ids: Vec<u64> = wanted.iter().map(|s| s.id).collect();
        self.subs.retain(|id, stop| {
            let keep = ids.contains(id);
            if !keep {
                stop.store(true, Ordering::Release);
            }
            keep
        });
        for sub in wanted {
            if self.subs.contains_key(&sub.id) {
                continue;
            }
            let stop = Arc::new(AtomicBool::new(false));
            self.subs.insert(sub.id, stop.clone());
            let SubscriptionKind::Every { interval_ms, make } = sub.kind;
            let (tx, proxy) = (self.sub_tx.clone(), self.proxy.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(interval_ms));
                    if stop.load(Ordering::Acquire) || tx.send(make()).is_err() {
                        break;
                    }
                    proxy.wake();
                }
            });
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
        self.sync_subscriptions();
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
                let mut outcome = Outcome::default();
                if self.tick_due.swap(false, Ordering::AcqRel) {
                    self.tick_pending = false;
                    let ms = self.start.elapsed().as_millis() as u64;
                    outcome = rt.handle_event(&Event::Tick(ms));
                }
                let mut msgs = self.tasks.drain();
                msgs.extend(self.sub_rx.try_iter());
                if !msgs.is_empty() {
                    let o = rt.dispatch_all(msgs);
                    outcome = Outcome {
                        redraw: outcome.redraw || o.redraw,
                        exit: outcome.exit || o.exit,
                        tick: outcome.tick || o.tick,
                    };
                }
                outcome
            }
            PlatformEvent::ScaleFactorChanged(_) => Outcome {
                redraw: true,
                ..Outcome::default()
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
