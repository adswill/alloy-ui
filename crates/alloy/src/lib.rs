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
    #[doc = include_str!("../../../docs/guide/12-dev-loop.md")]
    struct G11;
    #[doc = include_str!("../../../docs/guide/13-localization.md")]
    struct G12;
    #[doc = include_str!("../../../docs/guide/14-navigation.md")]
    struct G13;
    #[doc = include_str!("../../../docs/guide/15-testing.md")]
    struct G14;
    #[doc = include_str!("../../../docs/guide/16-coming-from-iced-and-egui.md")]
    struct G15;
    #[doc = include_str!("../../../docs/guide/17-shipping.md")]
    struct G16;
    #[doc = include_str!("../../../docs/guide/18-desktop-integration.md")]
    struct G17;
    #[doc = include_str!("../../../docs/guide/19-system-integration.md")]
    struct G18;
    #[doc = include_str!("../../../docs/guide/20-visual-widgets.md")]
    struct G19;
    #[doc = include_str!("../../../docs/guide/21-mobile.md")]
    struct G20;
}

/// `AGENTS.md` teaches coding agents the API; its snippets are compiled too.
#[cfg(doctest)]
mod agents {
    #[doc = include_str!("../../../AGENTS.md")]
    struct Agents;
}

/// The cookbook in `docs/cookbook` is compiled and run as doc-tests too.
#[cfg(doctest)]
mod cookbook {
    #[doc = include_str!("../../../docs/cookbook/01-state-and-input.md")]
    struct C0;
    #[doc = include_str!("../../../docs/cookbook/02-layout.md")]
    struct C1;
    #[doc = include_str!("../../../docs/cookbook/03-data.md")]
    struct C2;
    #[doc = include_str!("../../../docs/cookbook/04-app-polish.md")]
    struct C3;
    #[doc = include_str!("../../../docs/cookbook/05-look-and-language.md")]
    struct C4;
    #[doc = include_str!("../../../docs/cookbook/06-shell-and-testing.md")]
    struct C5;
}

// Lets the proc macros name this crate as `::alloy_ui` from inside it too.
extern crate self as alloy_ui;

mod app;
mod bind;
mod blocks;
mod chart;
mod chrome;
pub mod clipboard;
mod collections;
mod cx;
mod declare;
mod desktop;
mod dsl;
mod editors;
pub mod fs;
mod fx;
pub mod i18n;
mod icons;
mod inputs;
mod keys;
mod live;
mod mock;
mod modules;
mod motion;
mod palette;
mod persist;
pub mod plugin;
mod pro;
mod records;
mod resource;
mod rich;
mod router;
/// Deliver a deep link the way the shell does at launch; for tests of launch-time links.
#[doc(hidden)]
pub fn router_deliver_for_tests(url: &str) -> bool {
    router::deliver_link(url)
}
pub mod testing;
mod tone;
pub mod valid;
pub mod view;
mod viz;

pub use alloy_ui_macros::{at, state};
pub use alloy_ui_platform::safe_area::Insets;
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
pub use cx::{
    files_hovering, palette, safe_area, set_accent, set_dark, set_look, system, theme, ui, Look,
};
pub use declare::{clock, money};

/// Open the command palette (what `mod+k` does), for a button that advertises it.
/// Does nothing when it is already open.
pub fn open_palette() {
    if !palette::is_open() {
        palette::toggle();
    }
}
pub use desktop::{files, notify, Desktop, DesktopSpec, FileDialog, MenuBuilder, TrayBuilder};
pub use dsl::{
    attempt, button, caption, check, choose, custom, editor, gap, heading, input, list, nav,
    options, pick, progress, radio, radio_list, radio_list_with, radios, rule, scroll, section,
    slider, space, tabs, text, toggle, vspace, Act, Choice, Variant,
};
pub use editors::{
    code_editor, data_grid, rich_editor, Block, BlockKind, CodeView, Grid, GridCell, GridColumn,
    GridRows, Lang, Mark, Marks, RichText, RichView, Run, Selection,
};
pub use fx::{
    animated_with, counter, dots, enter, enter_after, glide, grow, keyframes, loading_bar,
    progress_ring, route, route_with, shared, skeleton, skeleton_bar, skeleton_circle,
    skeleton_text, spinner, spinner_sized, stack, stagger, timeline, Anchor, Layer, Rolling,
    RouteKind,
};
pub use icons::{icon, set_icon_pack, Ico, Icon, IconPack};
pub use inputs::{number, stepper, Count, Stepper};
pub use keys::{quit, redo, toast, undo, Hotkey, Meta, Shortcuts, Snap, Toast};
pub use live::{live_file, Live};
pub use mock::{series, Mock};
pub use modules::*;
pub use motion::{animated, fade, reveal, spring};
pub use pro::{
    context_menu, markdown, menu_item, menu_rule, popover, split, tab_bar, vlist, Markdown, Menu,
    MenuEntry, Popover, Split, TabBar, VList,
};
pub use records::{
    crud, crud_app, records, records_in, table_of, Board, Field, FieldInfo, Kind, Record, Records,
    Stamp, Value,
};
pub use resource::{block_on, resource, resource_for, sleep, Resource};
pub use rich::{
    badge, bar_chart, color_picker, line_chart, sparkline, swatch, tree, Badge, Chart, Picker,
    Series, Spark, Swatch, Tone, Tree, TreeItem,
};
pub use router::{link_path, router, router_saved, router_saved_at, Router};
pub use view::{col, row, Boxed, Flex, Node, View};
pub use viz::{
    calendar, candle, candles, canvas, card, entry, kanban, lane, media, node, node_graph, object,
    pie, pt, rgb, stream, viewport3d, CalAction, CalView, Candle, Candles, Card, Entry, Frame,
    GraphAction, GraphNode, Lane, MediaAction, MediaSource, Mesh, Move, Object, Orbit, Path, Pie,
    Playback, Pointer, Ring, Slice, Stream, TestCard, Wire,
};
pub use {serde, serde_json};

/// One import for everything an app needs: `use alloy_ui::prelude::*;`.
pub mod prelude {
    pub use crate::i18n::language_picker;
    pub use crate::live;
    pub use crate::{
        activity, activity_feed, avatar, avatar_group, breadcrumb, changelog, chat, command_bar,
        composer, empty_state, file, file_browser, folder, gallery_grid, gauge, hit, kpi, kpi_row,
        login, message, moment, moments, notice, notification_center, page_header, plan,
        plan_picker, radial, release, reply, setting, settings_page, tile, tour, tracker, Art,
        Presence,
    };
    pub use crate::{
        animated, animated_with, app, attempt, badge, bar_chart, bars, browse, browse_of, button,
        caption, check, choose, color_picker, context_menu, crud, crud_app, custom, editor, every,
        fade, field, field_after, filter_by, gap, heading, icon, input, line_chart, link_path,
        list, local, markdown, menu_item, menu_rule, nav, number, options, palette, pick, popover,
        progress, radio, radio_list, radio_list_with, radios, records, records_in, resource,
        resource_for, reveal, router, router_saved, router_saved_at, rule, scroll, secs, section,
        series, set_accent, set_dark, set_icon_pack, set_look, shell, sketch, sleep, slider, space,
        sparkline, split, spring, stat, stats, stepper, store, store_as, swatch, tab_bar, table,
        table_of, tabs, tally, text, theme_switch, toggle, tree, vlist, vspace, wizard, Act, App,
        Cell, Choice, ChoiceCell, Cmd, FlagCell, Icon, IconPack, Item, Live, Local, Look, Node,
        Place, Record, Records, Resource, Router, Stamp, TextCell, TreeItem, View, Wizard,
    };
    pub use crate::{
        at, choices, clipboard, clock, col, files, files_hovering, money, notify, quit, redo, row,
        state, system, t, toast, undo, vals, Hotkey, Shortcuts, WindowEffect,
    };
    pub use crate::{
        calendar, candle, candles, canvas, card, counter, dots, enter, enter_after, entry, glide,
        grow, kanban, keyframes, lane, loading_bar, media, node, node_graph, object, pie,
        progress_ring, pt, route, route_with, shared, skeleton, skeleton_bar, skeleton_circle,
        skeleton_text, spinner, spinner_sized, stack, stagger, stream, timeline, viewport3d,
        Anchor, Layer, RouteKind,
    };
    pub use crate::{code_editor, data_grid, rich_editor, GridCell, Lang, RichText, Tone};
    pub use alloy_ui_core::Color;
    pub use alloy_ui_theme::{Density, Easing, Preset};
    pub use alloy_ui_widgets::{Keyframes, Timeline};
    pub use std::time::Duration;
}

pub use alloy_ui_core::*;
pub use alloy_ui_ipc as ipc;
pub use alloy_ui_layout as layout;
pub use alloy_ui_platform as platform;
pub use alloy_ui_render as render;
pub use alloy_ui_theme as theme;
pub use alloy_ui_widgets as widgets;
pub use plugin::ext;

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
    // Load the graphics runtime and the fonts while the platform opens the window.
    alloy_ui_render::prewarm();
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
            keyed: HashMap::new(),
            proxy,
            start: Instant::now(),
            tick_due: Arc::new(AtomicBool::new(false)),
            tick_pending: false,
            subs: HashMap::new(),
            sub_rx,
            sub_tx,
            tray_applied: None,
            hotkeys_applied: Vec::new(),
            effect: WindowEffect::None,
            system_accent: false,
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
    /// Handles of keyed tasks, so a newer one (or `Cmd::cancel`) can stop the older.
    keyed: HashMap<u64, alloy_ui_ipc::TaskHandle>,
    proxy: alloy_ui_platform::shell::ShellProxy,
    start: Instant,
    tick_due: Arc<AtomicBool>,
    tick_pending: bool,
    subs: HashMap<u64, Arc<AtomicBool>>,
    sub_rx: mpsc::Receiver<A::Message>,
    sub_tx: mpsc::Sender<A::Message>,
    tray_applied: Option<platform::tray::TraySettings>,
    hotkeys_applied: Vec<String>,
    /// The window effect applied to the main window, and what the OS made of it.
    effect: WindowEffect,
    system_accent: bool,
}

impl<A: Application + 'static> Host<A> {
    /// Hand the menu bar, tray and global hotkeys to the OS. Labelled shortcuts that no
    /// menu of the app lists go in an "Actions" menu (and Undo/Redo in "Edit"), so every
    /// shortcut is discoverable; failures are logged because the shortcuts and the palette
    /// still work without a native menu.
    fn apply_desktop(
        &mut self,
        ctx: &mut ShellContext,
        items: &[(String, String)],
        desktop: &DesktopSpec,
    ) {
        fn listed(items: &[platform::menu::MenuItem], keys: &str, label: &str) -> bool {
            items.iter().any(|i| match i {
                platform::menu::MenuItem::Action {
                    shortcut, label: l, ..
                } => l == label && shortcut.as_deref().unwrap_or("") == keys,
                platform::menu::MenuItem::Submenu(m) => listed(&m.items, keys, label),
                _ => false,
            })
        }
        let mut edit = platform::menu::Menu::new("Edit");
        let mut actions = platform::menu::Menu::new("Actions");
        for (keys, label) in items {
            if desktop.menus.iter().any(|m| listed(&m.items, keys, label)) {
                continue;
            }
            let target = if label == "Undo" || label == "Redo" {
                &mut edit
            } else {
                &mut actions
            };
            *target = std::mem::take(target).action(keys, label, Some(keys));
        }
        let mut menus: Vec<_> = desktop.menus.clone();
        menus.extend([actions, edit].into_iter().filter(|m| !m.items.is_empty()));
        if cfg!(target_os = "macos") && !menus.is_empty() {
            // The first menu of a Mac app is the application menu, named after the app.
            menus.insert(
                0,
                platform::menu::Menu::new(ctx.window().title()).action(
                    "alloy:quit",
                    "Quit",
                    Some("mod+q"),
                ),
            );
        }
        if let Err(e) = ctx.set_menu(&menus) {
            log::debug!("no native menu bar: {e:?}");
        }
        if desktop.tray != self.tray_applied {
            match &desktop.tray {
                Some(t) => {
                    if let Err(e) = ctx.set_tray(t) {
                        log::warn!("{e}");
                    }
                }
                None => ctx.clear_tray(),
            }
            self.tray_applied = desktop.tray.clone();
        }
        if desktop.hotkeys != self.hotkeys_applied {
            ctx.unregister_global_hotkeys();
            for keys in &desktop.hotkeys {
                if let Err(e) = ctx.register_global_hotkey(keys) {
                    log::warn!("{e}");
                }
            }
            self.hotkeys_applied = desktop.hotkeys.clone();
        }
    }

    /// Make the host-level wishes of the app (window effect, accent) true.
    /// Cheap when nothing changed, so it runs after every outcome.
    fn apply_config(&mut self, ctx: &mut ShellContext) {
        let Some(rt) = self.runtime.as_ref() else {
            return;
        };
        let cfg = rt.app().host_config();
        if cfg.system_accent != self.system_accent {
            self.system_accent = cfg.system_accent;
            if let (true, Some([r, g, b])) = (cfg.system_accent, cx::system().accent) {
                cx::apply_accent_everywhere(Color::hex(
                    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
                ));
            }
        }
        if cfg.effect == self.effect {
            return;
        }
        self.effect = cfg.effect;
        self.refresh_effect(ctx);
    }

    /// (Re)apply `self.effect` to the main window and pick the matching page opacity.
    fn refresh_effect(&mut self, ctx: &mut ShellContext) {
        let effect = self.effect;
        let outcome = ctx.set_effect(effect);
        let see_through =
            effect != WindowEffect::None && outcome != platform::EffectOutcome::Unsupported;
        let ok = match self.renderers.get_mut(&WindowId::MAIN) {
            Some(r) => {
                let ok = r.set_transparent(see_through);
                r.set_background(if see_through && ok {
                    Color::hex(0).with_alpha(0.0)
                } else {
                    Color::hex(0xFFFFFF)
                });
                ok
            }
            None => false,
        };
        let alpha = match (see_through && ok, outcome) {
            (false, _) => 1.0,
            (true, platform::EffectOutcome::Applied) => effect.page_alpha(),
            (true, _) => effect.fallback_alpha(),
        };
        if !ok && effect != WindowEffect::None {
            // The surface cannot be composited: undo the OS side too.
            ctx.set_effect(WindowEffect::None);
        }
        cx::set_page_alpha(alpha);
        if let Some(rt) = self.runtime.as_mut() {
            let o = rt.dispatch_all(Vec::new());
            if o.redraw {
                ctx.request_redraw();
            }
        }
    }

    fn pump(&mut self, ctx: &mut ShellContext, outcome: Outcome) {
        if outcome.exit || keys::take_quit() {
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
            for req in rt.take_tasks() {
                match req {
                    alloy_ui_core::TaskRequest::Spawn(f) => {
                        self.tasks.spawn(f);
                    }
                    alloy_ui_core::TaskRequest::Keyed(k, f) => {
                        if let Some(old) = self.keyed.insert(k, self.tasks.spawn(f)) {
                            old.cancel();
                        }
                    }
                    alloy_ui_core::TaskRequest::Cancel(k) => {
                        if let Some(old) = self.keyed.remove(&k) {
                            old.cancel();
                        }
                    }
                }
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
        cx::set_safe_area(ctx.safe_area());
        let (w, h) = ctx.physical_size();
        let mut t = Instant::now();
        let made = WindowRenderer::new(ctx.window().clone(), w, h);
        alloy_ui_render::perf_mark("renderer-total", &mut t);
        match made {
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
        // Phones and tablets get the roomy spacing preset: bigger touch targets.
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let theme = theme.density(theme::Density::Spacious);
        let ui = widgets::Ui::new(theme);
        if let Some(r) = self.renderers.get(&WindowId::MAIN) {
            ui.set_metrics(std::rc::Rc::new(MeasurerMetrics(r.text_measurer())));
        }
        ui.set_clipboard(std::rc::Rc::new(OsClipboard(std::cell::RefCell::new(
            platform::default_clipboard(),
        ))));
        alloy_ui_render::perf_mark("ui-setup", &mut t);
        let app = build(&ui);
        alloy_ui_render::perf_mark("app-build", &mut t);
        cx::register_window_ui(WindowId::MAIN, &ui);
        self.ui = Some(ui);
        let mut rt = Runtime::new(app, size, layouter);
        for req in rt.take_tasks() {
            match req {
                alloy_ui_core::TaskRequest::Spawn(f) | alloy_ui_core::TaskRequest::Keyed(_, f) => {
                    self.tasks.spawn(f);
                }
                alloy_ui_core::TaskRequest::Cancel(_) => {}
            }
        }
        self.runtime = Some(rt);
        self.sync_subscriptions();
        self.apply_config(ctx);
        alloy_ui_render::perf_mark("runtime", &mut t);
        ctx.request_redraw();
    }

    fn event(&mut self, ctx: &mut ShellContext, event: PlatformEvent) {
        if let PlatformEvent::SettingsChanged(s) = event {
            cx::set_system(s);
            // An explicit ALLOY_REDUCE_MOTION (0 or 1) beats the OS.
            let forced = std::env::var("ALLOY_REDUCE_MOTION").ok().map(|v| v != "0");
            cx::set_reduced_motion_everywhere(forced.unwrap_or(s.reduced_motion));
            self.system_accent = false; // re-read the app's wish below
                                        // Mica and friends follow light/dark: tell the OS again.
            if self.effect != WindowEffect::None {
                self.refresh_effect(ctx);
            }
            self.apply_config(ctx);
            if let Some(rt) = self.runtime.as_mut() {
                let o = rt.dispatch_all(Vec::new());
                self.pump(ctx, o);
            }
            return;
        }
        let Some(rt) = self.runtime.as_mut() else {
            return;
        };
        let win = WindowId(ctx.window_key().0);
        let tick_ms = self.start.elapsed().as_millis() as u64;
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
            // Desktop commands are queued by name and run by the page's shortcut layer on the
            // tick that follows, so they reach the app exactly as a typed shortcut would.
            PlatformEvent::TrayClicked => {
                keys::invoke("tray:click".into());
                rt.handle_event(&Event::Tick(tick_ms))
            }
            PlatformEvent::GlobalHotkey(keys) => {
                keys::invoke(format!("global:{keys}"));
                rt.handle_event(&Event::Tick(tick_ms))
            }
            PlatformEvent::MenuAction(id) if id == "alloy:quit" => {
                ctx.exit();
                return;
            }
            PlatformEvent::MenuAction(id)
                if id.starts_with("cmd:")
                    || id.starts_with("tray:")
                    || id.starts_with("global:") =>
            {
                keys::invoke(id);
                rt.handle_event(&Event::Tick(tick_ms))
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
            PlatformEvent::SafeAreaChanged(i) => {
                // Rebuild the view (a resize does that) so `safe_area()` readers update.
                cx::set_safe_area(i);
                rt.handle_window_event(win, &Event::Resized(ctx.logical_size()))
            }
            PlatformEvent::Suspended => {
                // Android destroyed the native window: the surfaces are dead.
                self.renderers.clear();
                return;
            }
            PlatformEvent::Resumed => {
                // `redraw` rebuilds each window's renderer on its first paint.
                ctx.request_redraw();
                return;
            }
            PlatformEvent::ScaleFactorChanged(_) => Outcome {
                redraw: true,
                ..Outcome::default()
            },
            PlatformEvent::FileHovered(_) | PlatformEvent::FileHoverCancelled => {
                let over = matches!(event, PlatformEvent::FileHovered(_));
                // winit reports one `FileHovered` per file: act on the first only.
                if !cx::set_files_hovering(over) {
                    return;
                }
                let msg = rt.app().host_event(HostEvent::FilesHovering(over));
                rt.dispatch_all(msg.into_iter().collect())
            }
            PlatformEvent::FilesDropped(files) => {
                cx::set_files_hovering(false);
                let msgs = [
                    HostEvent::FilesHovering(false),
                    HostEvent::FilesDropped(files),
                ]
                .into_iter()
                .filter_map(|e| rt.app().host_event(e))
                .collect();
                rt.dispatch_all(msgs)
            }
            PlatformEvent::Open(args) => {
                let opened = platform::deeplink::classify(&args);
                // A second launch asked for this window: bring it forward.
                ctx.focus_window();
                let msgs = opened
                    .urls
                    .into_iter()
                    .map(HostEvent::OpenUrl)
                    .chain((!opened.files.is_empty()).then_some(HostEvent::OpenFiles(opened.files)))
                    .filter_map(|e| rt.app().host_event(e))
                    .collect();
                rt.dispatch_all(msgs)
            }
            _ => return,
        };
        self.pump(ctx, outcome);
        self.apply_config(ctx);
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
            let mut t = Instant::now();
            let list = rt.draw_window(win);
            alloy_ui_render::perf_mark("first-draw-list", &mut t);
            r.render(&list, ctx.scale_factor() as f32);
            alloy_ui_render::perf_mark("render+present", &mut t);
        }
        if let Some((items, desktop)) = keys::shell_changed() {
            self.apply_desktop(ctx, &items, &desktop);
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

/// Entry points for a phone or tablet build, from one function that runs the app.
///
/// Put this in the `lib` of a crate with `crate-type = ["cdylib", "staticlib", "rlib"]`:
///
/// ```ignore
/// alloy_ui::mobile_entry!(run);
/// fn run() { counter().run("Counter") }
/// ```
///
/// On Android it defines `android_main` (the `NativeActivity` entry point) and
/// hands the activity to Alloy first. On iOS it defines `alloy_ios_main`, which
/// the generated `main.m` calls. Elsewhere it expands to nothing, so the same
/// crate still builds for the desktop.
#[macro_export]
macro_rules! mobile_entry {
    ($run:path) => {
        #[cfg(target_os = "android")]
        #[allow(missing_docs)]
        #[no_mangle]
        pub fn android_main(app: $crate::platform::android::AndroidApp) {
            $crate::platform::android::init(app);
            $run();
        }
        #[cfg(target_os = "ios")]
        #[allow(missing_docs)]
        #[no_mangle]
        pub extern "C" fn alloy_ios_main() {
            $run();
        }
    };
}
