//! Alloy facade: wires the runtime, platform shell, renderer and task runner
//! together, and provides the concise app API.
//!
//! ```no_run
//! use alloy_ui::prelude::*;
//!
//! fn main() {
//!     sketch(0i64, |n| {
//!         row![button("-").act(|n| *n -= 1), n.to_string(), button("+").act(|n| *n += 1)]
//!             .align_center()
//!             .pad(24.0)
//!     })
//!     .run("Counter");
//! }
//! ```
//!
//! Two ways to write an app, both fully typed:
//!
//! * [`sketch`]: state plus a view function; handlers are closures over the state.
//!   No message enum.
//! * [`App`]: the Elm contract (`Msg`, `update`, `view`) for apps that want a message log,
//!   background [`Cmd`]s or subscriptions.
//!
//! Widgets are plain functions (`button`, `input`, `check`, `slider`, ...) that need
//! no `Ui` handle, no ids and no `Root` wrapper; the lower-level [`run`] and
//! [`Application`] remain for custom shells.
#![warn(missing_docs)]

mod app;
mod chart;
mod cx;
mod dsl;
pub mod testing;
pub mod view;

pub use app::{every, secs, sketch, App, Cmd, Sketch, Sub};
pub use chart::bars;
pub use cx::{palette, set_dark, theme, ui};
pub use dsl::{
    button, caption, check, choose, custom, editor, gap, heading, input, nav, options, pick,
    progress, radio, rule, scroll, section, slider, space, tabs, text, toggle, Act, Choice,
    Variant,
};
pub use view::{col, row, Boxed, Flex, Node, View};

/// One import for everything an app needs: `use alloy_ui::prelude::*;`.
pub mod prelude {
    pub use crate::{
        bars, button, caption, check, choose, custom, editor, every, gap, heading, input, nav,
        options, palette, pick, progress, radio, rule, scroll, secs, section, set_dark, sketch,
        slider, space, tabs, text, toggle, Act, App, Choice, Cmd, Node, View,
    };
    pub use crate::{col, row};
    pub use alloy_ui_core::Color;
    pub use std::time::Duration;
}

pub use alloy_ui_core::*;
pub use alloy_ui_ipc as ipc;
pub use alloy_ui_layout as layout;
pub use alloy_ui_platform as platform;
pub use alloy_ui_render as render;
pub use alloy_ui_theme as theme;
pub use alloy_ui_widgets as widgets;

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

/// Resolve a view tree to layout by arranging the root over the whole window.
///
/// Use as the layouter for widget-based apps.
pub fn layout_root<M>(root: &Element<M>, size: Size) -> Layout {
    root.0.arrange(Rectangle::new(Point::new(0.0, 0.0), size))
}

/// Run a widget-based app in a native window until it exits.
///
/// `build` receives the per-window [`widgets::Ui`], already wired to the real
/// text metrics and the OS clipboard, and returns the application. Must be
/// called on the main thread.
pub fn run<A, B>(settings: WindowSettings, build: B) -> Result<(), PlatformError>
where
    A: Application + 'static,
    B: FnOnce(&widgets::Ui) -> A + 'static,
{
    run_with(settings, build, layout_root::<A::Message>)
}

/// Like [`run`] but with a custom layouter.
pub fn run_with<A, B>(
    settings: WindowSettings,
    build: B,
    layouter: Layouter<A::Message>,
) -> Result<(), PlatformError>
where
    A: Application + 'static,
    B: FnOnce(&widgets::Ui) -> A + 'static,
{
    let size = settings.size;
    alloy_ui_platform::run(settings, move |proxy| {
        let task_proxy = proxy.clone();
        let (sub_tx, sub_rx) = mpsc::channel();
        Host {
            pending: Some((Box::new(build), size, layouter)),
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

struct MeasurerMetrics(render::TextMeasurer);

impl widgets::TextMetrics for MeasurerMetrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.0.measure(text, size, 400, None).width
    }
}

struct OsClipboard(std::cell::RefCell<Box<dyn platform::Clipboard>>);

impl widgets::Clipboard for OsClipboard {
    fn get(&self) -> Option<String> {
        self.0.borrow().read_text().ok().flatten()
    }
    fn set(&self, text: String) {
        let _ = self.0.borrow_mut().write_text(&text);
    }
}

type Build<A> = Box<dyn FnOnce(&widgets::Ui) -> A>;
type Pending<A> = (Build<A>, Size, Layouter<<A as Application>::Message>);

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
        let (build, _, layouter) = self.pending.take().expect("started once");
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
        let theme = if ctx.is_dark().unwrap_or(false) {
            theme::Theme::dark()
        } else {
            theme::Theme::light()
        };
        let ui = widgets::Ui::new(theme);
        if let Some(r) = self.renderer.as_ref() {
            ui.set_metrics(std::rc::Rc::new(MeasurerMetrics(r.text_measurer())));
        }
        ui.set_clipboard(std::rc::Rc::new(OsClipboard(std::cell::RefCell::new(
            platform::default_clipboard(),
        ))));
        let app = build(&ui);
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
