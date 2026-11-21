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

/// The guide in `docs/guide` is compiled as doc-tests, so its snippets cannot rot.
#[cfg(doctest)]
mod guide {
    #[doc = include_str!("../../../docs/guide/01-getting-started.md")]
    struct G0;
    #[doc = include_str!("../../../docs/guide/02-state-and-bindings.md")]
    struct G1;
    #[doc = include_str!("../../../docs/guide/03-layout.md")]
    struct G2;
    #[doc = include_str!("../../../docs/guide/04-widgets.md")]
    struct G3;
    #[doc = include_str!("../../../docs/guide/05-records-and-forms.md")]
    struct G4;
    #[doc = include_str!("../../../docs/guide/06-async-resources.md")]
    struct G5;
    #[doc = include_str!("../../../docs/guide/07-multi-window.md")]
    struct G6;
    #[doc = include_str!("../../../docs/guide/08-shortcuts-palette-toasts-undo.md")]
    struct G7;
    #[doc = include_str!("../../../docs/guide/09-theming.md")]
    struct G8;
    #[doc = include_str!("../../../docs/guide/10-bundling.md")]
    struct G9;
    #[doc = include_str!("../../../docs/guide/11-capabilities-and-ipc.md")]
    struct G10;
    #[doc = include_str!("../../../docs/guide/12-localization.md")]
    struct G11;
}

// Lets the proc macros name this crate as `::alloy_ui` from inside it too.
extern crate self as alloy_ui;

mod app;
mod bind;
mod blocks;
mod chart;
mod chrome;
mod collections;
mod cx;
mod declare;
mod dsl;
pub mod fs;
pub mod i18n;
mod inputs;
mod keys;
mod mock;
mod palette;
mod persist;
mod records;
mod resource;
pub mod testing;
mod tone;
pub mod valid;
pub mod view;

pub use alloy_ui_macros::{at, state};
pub use app::{every, secs, sketch, App, Cmd, Sketch, Sub};
pub use bind::{place, Lens, Place, Val};
pub use blocks::{
    app, field, field_after, local, shell, store, store_as, table, theme_switch, wizard, Local,
    Pages, Table, Wizard,
};
pub use chart::bars;
pub use collections::{
    browse, browse_of, filter_by, stat, stats, tally, Browse, Cell, ChoiceCell, FlagCell, Item,
    Prop, TextCell,
};
pub use cx::{palette, set_dark, theme, ui};
pub use declare::{clock, money};
pub use dsl::{
    attempt, button, caption, check, choose, custom, editor, gap, heading, input, list, nav,
    options, pick, progress, radio, radio_list, radio_list_with, radios, rule, scroll, section,
    slider, space, tabs, text, toggle, vspace, Act, Choice, Variant,
};
pub use inputs::{number, stepper, Count, Stepper};
pub use keys::{redo, toast, undo, Hotkey, Meta, Shortcuts, Snap, Toast};
pub use mock::{series, Mock};
pub use records::{
    crud, crud_app, records, records_in, table_of, Board, Field, FieldInfo, Kind, Record, Records,
    Stamp, Value,
};
pub use resource::{block_on, resource, sleep, Resource};
pub use view::{col, row, Boxed, Flex, Node, View};
pub use {serde, serde_json};

/// One import for everything an app needs: `use alloy_ui::prelude::*;`.
pub mod prelude {
    pub use crate::i18n::language_picker;
    pub use crate::{
        app, attempt, bars, browse, browse_of, button, caption, check, choose, crud, crud_app,
        custom, editor, every, field, field_after, filter_by, gap, heading, input, list, local,
        nav, number, options, palette, pick, progress, radio, radio_list, radio_list_with, radios,
        records, records_in, resource, rule, scroll, secs, section, series, set_dark, shell,
        sketch, sleep, slider, space, stat, stats, stepper, store, store_as, table, table_of, tabs,
        tally, text, theme_switch, toggle, vspace, wizard, Act, App, Cell, Choice, ChoiceCell, Cmd,
        FlagCell, Item, Local, Node, Place, Record, Records, Resource, Stamp, TextCell, View,
        Wizard,
    };
    pub use crate::{
        at, choices, clock, col, money, redo, row, state, t, toast, undo, vals, Hotkey, Shortcuts,
    };
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
            renderers: HashMap::new(),
            ui: None,
            access: HashMap::new(),
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
    fn width_styled(
        &self,
        text: &str,
        size: f32,
        weight: u16,
        family: alloy_ui_core::FontFamily,
    ) -> f32 {
        self.0
            .measure_styled(text, size, weight, family, None, None)
            .width
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
    renderers: HashMap<WindowId, WindowRenderer>,
    ui: Option<widgets::Ui>,
    /// Last published accessibility nodes per window, to resolve requests back to widgets.
    access: HashMap<WindowId, Vec<platform::access::AccessNode>>,
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
        if let Some(rt) = self.runtime.as_mut() {
            for req in rt.take_window_requests() {
                match req {
                    WindowRequest::Open(id, spec) => ctx.open_window(
                        platform::shell::WindowKey(id.0),
                        WindowSettings {
                            title: spec.title,
                            size: spec.size,
                        },
                    ),
                    WindowRequest::Close(id) => {
                        self.renderers.remove(&id);
                        ctx.close_window(platform::shell::WindowKey(id.0));
                    }
                }
            }
            for id in rt.take_dirty() {
                ctx.request_redraw_of(platform::shell::WindowKey(id.0));
            }
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
            Ok(r) => {
                self.renderers.insert(WindowId::MAIN, r);
            }
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
        if let Some(r) = self.renderers.get(&WindowId::MAIN) {
            ui.set_metrics(std::rc::Rc::new(MeasurerMetrics(r.text_measurer())));
        }
        ui.set_clipboard(std::rc::Rc::new(OsClipboard(std::cell::RefCell::new(
            platform::default_clipboard(),
        ))));
        let app = build(&ui);
        cx::register_window_ui(WindowId::MAIN, &ui);
        self.ui = Some(ui);
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
        let win = WindowId(ctx.window_key().0);
        let outcome = match event {
            PlatformEvent::Accessibility { window, request } => {
                let w = WindowId(window.0);
                let nodes = self.access.get(&w).map(Vec::as_slice).unwrap_or(&[]);
                let Some(node) = nodes.iter().find(|n| n.id == request.target) else {
                    return;
                };
                let ui = cx::window_ui(w).or_else(|| self.ui.clone());
                let o = apply_access(rt, ui.as_ref(), w, node, &request.action);
                self.pump(ctx, o);
                return;
            }
            PlatformEvent::WindowClosed => {
                self.renderers.remove(&win);
                self.access.remove(&win);
                cx::forget_window_ui(win);
                rt.window_closed(win)
            }
            PlatformEvent::Input(Event::Resized(s)) => {
                let (w, h) = ctx.physical_size();
                if let Some(r) = self.renderers.get_mut(&win) {
                    r.resize(w, h);
                }
                rt.handle_window_event(win, &Event::Resized(s))
            }
            PlatformEvent::Input(e) => rt.handle_window_event(win, &e),
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
            PlatformEvent::MenuAction(id) => {
                // A menu item or shortcut: replay it as the key press it stands for, so the
                // focused widget and the page see it exactly as they would a typed one.
                let Some(sc) = platform::shortcut::Shortcut::parse(&id) else {
                    return;
                };
                let key = match sc.key.to_ascii_lowercase().as_str() {
                    "enter" => Key::Enter,
                    "tab" => Key::Tab,
                    "backspace" => Key::Backspace,
                    "escape" | "esc" => Key::Escape,
                    _ if sc.key.chars().count() == 1 => Key::Character(sc.key.clone()),
                    _ => Key::Named(sc.key.clone()),
                };
                rt.handle_window_event(win, &Event::KeyPressed(key, sc.mods))
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
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        let win = WindowId(ctx.window_key().0);
        if let std::collections::hash_map::Entry::Vacant(slot) = self.renderers.entry(win) {
            // A window opened by `Command::open_window`: build its surface on first paint.
            let (w, h) = ctx.physical_size();
            match WindowRenderer::new(ctx.window().clone(), w, h) {
                Ok(r) => {
                    slot.insert(r);
                }
                Err(e) => {
                    log::error!("renderer init failed for {win:?}: {e}");
                    return;
                }
            }
        }
        if let Some(r) = self.renderers.get_mut(&win) {
            r.render(&rt.draw_window(win), ctx.scale_factor() as f32);
        }
        if let Some(items) = keys::menu_changed() {
            // Labelled shortcuts go in the menu bar (and, where the OS has no native one, are
            // still matched by the shell). Fails quietly where there is no menu bar.
            let mut edit = platform::menu::Menu::new("Edit");
            let mut actions = platform::menu::Menu::new("Actions");
            for (keys, label) in &items {
                let target = if label == "Undo" || label == "Redo" {
                    &mut edit
                } else {
                    &mut actions
                };
                *target = std::mem::take(target).action(keys, label, Some(keys));
            }
            let menus: Vec<_> = [actions, edit]
                .into_iter()
                .filter(|m| !m.items.is_empty())
                .collect();
            if let Err(e) = ctx.set_menu(&menus) {
                log::debug!("no native menu bar: {e:?}");
            }
        }
        if let Some(ui) = cx::window_ui(win).or_else(|| self.ui.clone()) {
            let nodes = access_nodes(&ui.accessibility());
            ctx.set_accessibility(&platform::access::AccessTree {
                title: ctx.window().title(),
                nodes: nodes.clone(),
            });
            self.access.insert(win, nodes);
        }
    }
}

/// Map the widgets' accessibility description onto the platform's tree.
///
/// Focusable widgets keep their focus id; the rest get a hash of role, label
/// and how many identical ones came before, which is stable while the layout is.
fn access_nodes(nodes: &[widgets::A11yNode]) -> Vec<platform::access::AccessNode> {
    use platform::access::{AccessId, AccessNode, AccessRole};
    use std::hash::{Hash, Hasher};
    use widgets::Role;
    let mut seen: HashMap<(u8, &str), u64> = HashMap::new();
    nodes
        .iter()
        .map(|n| {
            let role = match n.role {
                Role::Label => AccessRole::Label,
                Role::Button => AccessRole::Button,
                Role::Checkbox => AccessRole::Checkbox,
                Role::Switch => AccessRole::Switch,
                Role::Radio => AccessRole::Radio,
                Role::Slider => AccessRole::Slider,
                Role::SpinButton => AccessRole::SpinButton,
                Role::TextInput => AccessRole::TextInput,
                Role::TextEditor => AccessRole::TextEditor,
                Role::ComboBox => AccessRole::ComboBox,
                Role::DatePicker => AccessRole::DatePicker,
                Role::Tab => AccessRole::Tab,
                Role::ProgressBar => AccessRole::ProgressBar,
            };
            let id = n.id.unwrap_or_else(|| {
                let count = seen.entry((role as u8, n.label.as_str())).or_insert(0);
                *count += 1;
                let mut h = std::collections::hash_map::DefaultHasher::new();
                (role as u8, &n.label, *count).hash(&mut h);
                // Keep hashed ids clear of small sequential focus ids.
                h.finish() | (1 << 63)
            });
            AccessNode {
                id: AccessId(id),
                role,
                label: n.label.clone(),
                value: n.value.clone(),
                checked: n.checked,
                focused: n.focused,
                enabled: n.enabled,
                bounds: n.bounds,
                range: None,
            }
        })
        .collect()
}

/// Carry out a screen-reader request by replaying the input a user would give.
fn apply_access<A: Application>(
    rt: &mut Runtime<A>,
    ui: Option<&widgets::Ui>,
    win: WindowId,
    node: &platform::access::AccessNode,
    action: &platform::access::AccessAction,
) -> Outcome {
    use platform::access::AccessAction as Act;
    let b = node.bounds;
    let centre = Point::new(
        b.origin.x + b.size.width / 2.0,
        b.origin.y + b.size.height / 2.0,
    );
    let focus = |ui: Option<&widgets::Ui>| {
        if let Some(ui) = ui {
            // Hashed ids (bit 63 set) belong to non-focusable widgets.
            if node.id.0 >> 63 == 0 {
                ui.set_focus(Some(node.id.0));
            }
        }
    };
    let key = |rt: &mut Runtime<A>, name: &str| {
        let k = Key::Named(name.to_string());
        rt.handle_window_event(win, &Event::KeyPressed(k.clone(), Modifiers::default()));
        rt.handle_window_event(win, &Event::KeyReleased(k, Modifiers::default()))
    };
    match action {
        Act::Press => {
            rt.handle_window_event(win, &Event::CursorMoved(centre));
            rt.handle_window_event(win, &Event::MousePressed(MouseButton::Left));
            rt.handle_window_event(win, &Event::MouseReleased(MouseButton::Left))
        }
        Act::Focus => {
            focus(ui);
            rt.handle_window_event(win, &Event::RedrawRequested)
        }
        Act::Increment => {
            focus(ui);
            key(rt, "ArrowUp")
        }
        Act::Decrement => {
            focus(ui);
            key(rt, "ArrowDown")
        }
        Act::SetValue(_) => {
            log::debug!("accessibility SetValue is not supported yet");
            Outcome::default()
        }
    }
}
