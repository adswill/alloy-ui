//! The application trait, commands, subscriptions and the `run` entry points.

use crate::{
    chrome::Chrome,
    cx,
    dsl::Act,
    keys::{self, Hotkey, Meta, Snap},
    view::{Node, View},
};
use alloy_ui_core::{
    Application, Command, Element, HostConfig, HostEvent, Subscription, WindowEffect,
};
use alloy_ui_platform::WindowSettings;
use alloy_ui_widgets::{Container, Root, Ui};
use std::path::PathBuf;
use std::{future::Future, hash::Hasher, time::Duration};

/// A side effect returned from `update`. Most messages need none: implement `App::update` and return nothing.
pub struct Cmd<M>(pub(crate) Command<M>);

impl<M> Cmd<M> {
    /// No effect.
    pub fn none() -> Self {
        Cmd(Command::None)
    }
    /// Run a future in the background; its output becomes a message.
    pub fn perform(fut: impl Future<Output = M> + Send + 'static) -> Self {
        Cmd(Command::perform(fut))
    }
    /// Like [`perform`](Self::perform), but under a `key`: starting another future with the
    /// same key cancels the one still running, so only the latest result arrives. Use it for
    /// search-as-you-type and refresh buttons. The key is anything hashable; a `&str` works.
    ///
    /// ```ignore
    /// Msg::Query(q) => Cmd::perform_keyed("search", async move { Msg::Found(search(q).await) }),
    /// ```
    pub fn perform_keyed(
        key: impl std::hash::Hash,
        fut: impl Future<Output = M> + Send + 'static,
    ) -> Self {
        Cmd(Command::perform_keyed(key_hash(key), fut))
    }
    /// Cancel the future started with [`perform_keyed`](Self::perform_keyed) under `key`.
    /// Its message never arrives, and the future is dropped at its next wake-up.
    pub fn cancel(key: impl std::hash::Hash) -> Self {
        Cmd(Command::Cancel(key_hash(key)))
    }
    /// Close every window and exit.
    pub fn exit() -> Self {
        Cmd(Command::Exit)
    }
    /// Open another window. `id` (not 0, which is the first window) names it in
    /// [`App::view_window`] and [`Cmd::close_window`].
    pub fn open_window(id: u64, title: &str, width: f32, height: f32) -> Self {
        Cmd(Command::open_window(
            alloy_ui_core::WindowId(id),
            title,
            alloy_ui_core::Size::new(width, height),
        ))
    }
    /// Close a window opened with [`Cmd::open_window`]. Closing the last one exits.
    pub fn close_window(id: u64) -> Self {
        Cmd(Command::CloseWindow(alloy_ui_core::WindowId(id)))
    }
}

pub(crate) fn key_hash(key: impl std::hash::Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    h.finish()
}

/// A message source that lives as long as the app asks for it.
pub struct Sub<M>(Subscription<M>);

/// Send `msg()` every `period`. Stops when `subscriptions` no longer returns it.
#[track_caller]
pub fn every<M>(period: Duration, msg: impl Fn() -> M + Send + Sync + 'static) -> Sub<M> {
    let loc = std::panic::Location::caller();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write(loc.file().as_bytes());
    h.write_u32(loc.line());
    h.write_u32(loc.column());
    h.write_u128(period.as_nanos());
    Sub(Subscription::every(h.finish(), period, msg))
}

/// A subscription registered from inside a view: type-erased, replayed on every sync.
struct ViewSub<M> {
    id: u64,
    interval: Duration,
    make: std::sync::Arc<dyn Fn() -> M + Send + Sync>,
}

thread_local! {
    /// Subscriptions registered by the view build in progress.
    static VIEW_SUBS_PENDING: std::cell::RefCell<Vec<Box<dyn std::any::Any>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// The subscriptions of each window's last view build.
    static VIEW_SUBS: std::cell::RefCell<std::collections::HashMap<u64, Vec<Box<dyn std::any::Any>>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Ask for a timer from inside a view: it runs while the view that asks is on screen and stops
/// the first time a rebuild no longer asks. `App` apps return the message from `subscribe`.
///
/// ```ignore
/// fn view(&self) -> impl View<Msg> {
///     if self.page == Page::Live { subscribe(every(secs(5.0), || Msg::Refresh)); }
///     /* ... */
/// }
/// ```
pub fn subscribe<M: Send + 'static>(sub: Sub<M>) {
    let id = sub.0.id;
    let alloy_ui_core::SubscriptionKind::Every { interval_ms, make } = sub.0.kind;
    let v = ViewSub {
        id,
        interval: Duration::from_millis(interval_ms),
        make,
    };
    VIEW_SUBS_PENDING.with(|p| p.borrow_mut().push(Box::new(v)));
}

/// Run `f` on the state every `period` while the view that calls this is shown (`sketch` apps):
/// `if s.live { tick_while_shown(secs(5.0), |s: &mut Feed| s.refresh()); }`. The timer starts
/// when a build first calls it and stops when a build stops.
#[track_caller]
pub fn tick_while_shown<S: 'static>(period: Duration, f: impl Fn(&mut S) + Send + Sync + 'static) {
    let act = Act::new(f);
    subscribe(every(period, move || act.clone()));
}

fn view_subs_begin() {
    VIEW_SUBS_PENDING.with(|p| p.borrow_mut().clear());
}

fn view_subs_commit(window: u64) {
    let found = VIEW_SUBS_PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
    VIEW_SUBS.with(|m| {
        if found.is_empty() {
            m.borrow_mut().remove(&window);
        } else {
            m.borrow_mut().insert(window, found);
        }
    });
}

/// The subscriptions views of message type `M` asked for, one per id.
fn view_subs<M: Send + 'static>() -> Vec<Subscription<M>> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    VIEW_SUBS.with(|m| {
        for v in m.borrow().values().flatten() {
            if let Some(v) = v.downcast_ref::<ViewSub<M>>() {
                if !seen.contains(&v.id) {
                    seen.push(v.id);
                    let make = v.make.clone();
                    out.push(Subscription::every(v.id, v.interval, move || make()));
                }
            }
        }
    });
    out
}

/// `n` seconds as a [`Duration`].
pub fn secs(n: f32) -> Duration {
    Duration::from_secs_f32(n)
}

/// An application: state, `update` and `view`.
///
/// ```no_run
/// use alloy_ui::prelude::*;
///
/// #[derive(Default)]
/// struct Counter(i64);
///
/// #[derive(Clone)]
/// enum Msg { Inc, Dec }
///
/// impl App for Counter {
///     type Msg = Msg;
///     fn update(&mut self, m: Msg) {
///         match m { Msg::Inc => self.0 += 1, Msg::Dec => self.0 -= 1 }
///     }
///     fn view(&self) -> impl View<Msg> {
///         row![button("-").on(Msg::Dec), self.0.to_string(), button("+").on(Msg::Inc)]
///             .align_center().pad(24.0)
///     }
/// }
///
/// fn main() { Counter::default().run("Counter") }
/// ```
pub trait App: Sized + 'static {
    /// Messages produced by widgets, commands and subscriptions.
    type Msg: Clone + Send + 'static;

    /// Apply a message to the state.
    fn update(&mut self, _msg: Self::Msg) {}

    /// Like [`update`](Self::update) but may start background work. Implement
    /// this one instead of `update` when a message needs a [`Cmd`].
    fn update_cmd(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        self.update(msg);
        Cmd::none()
    }

    /// Describe the UI for the current state.
    fn view(&self) -> impl View<Self::Msg>;

    /// Describe the UI of window `id` (0 is the first window). Defaults to
    /// [`view`](Self::view), so single-window apps ignore it.
    fn view_window(&self, _id: u64) -> impl View<Self::Msg> {
        self.view()
    }

    /// Timers and other long-lived message sources for the current state.
    fn subscriptions(&self) -> Vec<Sub<Self::Msg>> {
        Vec::new()
    }

    /// Keyboard shortcuts for the whole window: `vec![Hotkey::new("mod+s", Msg::Save)]`.
    /// A key press goes to the focused widget first, so a text field keeps its own `mod+z`.
    fn shortcuts(&self) -> Vec<Hotkey<Self::Msg>> {
        Vec::new()
    }

    /// The menu bar, tray icon and global hotkeys. [`Sketch`] and [`Pages`](crate::Pages) fill
    /// this from `.menu`, `.tray` and `.global_hotkey`; an [`App`] written by hand returns
    /// `DesktopSpec::default()` (the default) or builds one with a [`Desktop`](crate::Desktop).
    fn desktop(&self) -> crate::DesktopSpec {
        crate::DesktopSpec::default()
    }

    /// The smallest size, in logical pixels, the user can resize the window to. `None` (the
    /// default) means no limit. [`Sketch::min_window_size`] sets it; the OS enforces it in a
    /// real window and [`Harness::resize`](crate::Harness::resize) clamps to it.
    fn min_window_size(&self) -> Option<(f32, f32)> {
        None
    }

    /// Where toasts sit: pixels kept free below the stack, and how many show at once.
    /// Defaults to `(24.0, 4)`; [`Sketch::toast_inset`] and [`Sketch::toast_max`] set it.
    fn toast_layout(&self) -> (f32, usize) {
        (24.0, 4)
    }

    /// The density of the whole window (spacing, paddings, default row heights), or `None`
    /// (the default) to leave it to the theme or [`Look`](crate::Look). [`Sketch::density`]
    /// sets it. It wins over the density of a `Look`.
    fn density(&self) -> Option<alloy_ui_theme::Density> {
        None
    }

    /// A copy of the state for undo, or `None` (the default) for an app without it.
    /// `Some(Snap::new(self.clone()))` is usually all it takes, together with
    /// [`restore`](Self::restore) and [`meta`](Self::meta).
    fn snapshot(&self) -> Option<Snap> {
        None
    }

    /// Put back a state captured by [`snapshot`](Self::snapshot).
    fn restore(&mut self, _snap: Snap) {}

    /// The message that calls [`undo`](crate::undo) or [`redo`](crate::redo), so `mod+z` and a
    /// toast's Undo button can reach the app. `None` for an app without undo.
    fn meta(&self, _what: Meta) -> Option<Self::Msg> {
        None
    }

    /// Files dropped on the window from another app (a file manager, the desktop).
    /// While they hover, [`files_hovering`](crate::files_hovering) is true.
    fn on_drop(&self, _files: Vec<PathBuf>) -> Option<Self::Msg> {
        None
    }

    /// Files started (`true`) or stopped (`false`) hovering over the window.
    /// Rarely needed: [`files_hovering`](crate::files_hovering) reads the same fact in `view`.
    fn on_drag_over(&self, _over: bool) -> Option<Self::Msg> {
        None
    }

    /// The user asked to close the window (close button, Alt+F4, Cmd+W). Return `None` to let
    /// it close. Return a message to keep the window open and handle it instead, usually to ask
    /// about unsaved work; close for real afterwards with `Cmd::exit()`.
    fn on_close(&self) -> Option<Self::Msg> {
        None
    }

    /// The window gained (`true`) or lost (`false`) keyboard focus: pause polling, refresh data
    /// on return, dim a live view.
    fn on_focus(&self, _focused: bool) -> Option<Self::Msg> {
        None
    }

    /// A deep link was opened: the OS passed a `myapp://...` URL for a scheme the app
    /// registered ([`url_schemes`](Self::url_schemes)), at launch or, for a
    /// [`single_instance`](Self::single_instance) app, from a second launch.
    fn on_open_url(&self, _url: String) -> Option<Self::Msg> {
        None
    }

    /// The OS asked the app to open these files (file association, "Open with", or a second
    /// launch of a [`single_instance`](Self::single_instance) app).
    fn on_open_files(&self, _files: Vec<PathBuf>) -> Option<Self::Msg> {
        None
    }

    /// `Some("com.example.app")` makes the app a single instance: launching it again hands
    /// its arguments to the running copy (which comes to the front and gets
    /// [`on_open_url`](Self::on_open_url) / [`on_open_files`](Self::on_open_files)) and exits.
    fn single_instance(&self) -> Option<String> {
        None
    }

    /// URL schemes this app opens (`vec!["myapp".into()]` for `myapp://...`). Registered for the
    /// current user at start-up on Linux and Windows; on macOS they come from the bundle
    /// (`url-schemes` in `[package.metadata.alloy]`, written by `alloy bundle`).
    fn url_schemes(&self) -> Vec<String> {
        Vec::new()
    }

    /// A see-through look for the window: Mica, acrylic, blur or plain transparency. The OS
    /// decides what it can do; an effect it cannot provide leaves the window opaque.
    fn window_effect(&self) -> WindowEffect {
        WindowEffect::None
    }

    /// `true` follows the user's system accent colour (macOS, Windows, GNOME) instead of the
    /// app's own. Reduced motion is always followed.
    fn system_accent(&self) -> bool {
        false
    }

    /// Open an 800x600 window and run until it closes.
    fn run(self, title: &str) {
        self.run_sized(title, 800.0, 600.0);
    }

    /// Open a window of the given logical size and run until it closes.
    ///
    /// `--headless [file.png]` renders one frame to a PNG instead (after
    /// pressing every `--click <text>`, `--rclick <text>`, `--hover <text>`, `--key <name>`, `--chord <keys>`, `--type <text>`, `--drag <From>To>`, `--drop <file>`, `--open <url>` and `--advance <ms>` in order; transitions are settled unless `--advance` is given), for CI and docs.
    fn run_sized(self, title: &str, width: f32, height: f32) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        // `--web [addr]` or `ALLOY_WEB=addr`: show the app in a browser instead of a window.
        let env = std::env::var("ALLOY_WEB").ok().filter(|v| !v.is_empty());
        if let Some(at) = args.iter().position(|a| a == "--web") {
            let addr = args.get(at + 1).filter(|a| !a.starts_with("--")).cloned();
            let addr = addr.or(env).unwrap_or_else(|| "127.0.0.1:8080".into());
            return crate::web::serve(self, &addr, (width, height)).expect("web server");
        } else if let Some(addr) = env {
            return crate::web::serve(self, &addr, (width, height)).expect("web server");
        }
        if let Some(at) = args.iter().position(|a| a == "--headless") {
            let mut h = crate::testing::Harness::new(self, width, height);
            h.settle();
            // Scripted input, in order: `--click Text`, `--chord mod+k`, `--type text`.
            for pair in args.windows(2) {
                match pair[0].as_str() {
                    "--click" => drop(h.click(&pair[1])),
                    "--chord" => drop(h.chord(&pair[1])),
                    "--type" => drop(h.type_text(&pair[1])),
                    "--rclick" => drop(h.right_click(&pair[1])),
                    "--hover" => drop(h.hover(&pair[1])),
                    "--key" => drop(h.key(&pair[1])),
                    "--advance" => drop(h.advance(16).advance(pair[1].parse().unwrap_or(0))),
                    "--drag-hold" => match pair[1].split_once('>') {
                        Some((from, to)) => drop(h.drag_hold(from.trim(), to.trim())),
                        None => eprintln!("--drag-hold wants `From>To`"),
                    },
                    "--drag" => match pair[1].split_once('>') {
                        Some((from, to)) => drop(h.drag(from.trim(), to.trim())),
                        None => eprintln!("--drag wants `From>To`"),
                    },
                    "--drop" => drop(h.drop_files([&pair[1]])),
                    "--open" => drop(h.open_url(&pair[1])),
                    _ => {}
                }
            }
            h.settle();
            if args.iter().any(|a| a == "--inspect") {
                h.inspect(true);
            }
            // Let transitions finish unless the script asked for a mid-flight frame.
            if !args.iter().any(|a| a == "--advance") {
                h.advance(16).advance(1000);
            }
            let out = args.get(at + 1).filter(|a| !a.starts_with("--"));
            let path = out
                .cloned()
                .unwrap_or_else(|| format!("{}.png", title.replace(' ', "_")));
            h.save_png(&path);
            println!("wrote {path}");
            return;
        }
        // Single instance first: a second launch must not open a window.
        let _instance = match self.single_instance() {
            Some(id) if std::env::var_os("ALLOY_NO_SINGLE_INSTANCE").is_none() => {
                match alloy_ui_platform::instance::single_instance(&id) {
                    alloy_ui_platform::instance::Single::Forwarded => return,
                    alloy_ui_platform::instance::Single::Primary(guard) => Some(guard),
                }
            }
            _ => None,
        };
        let schemes = self.url_schemes();
        if !schemes.is_empty() {
            // Registering can spawn helper tools: keep it off the start-up path.
            let id = self
                .single_instance()
                .unwrap_or_else(|| title.to_lowercase().replace(' ', "-"));
            let _ = std::thread::Builder::new()
                .name("alloy-schemes".into())
                .spawn(move || {
                    for scheme in schemes {
                        if let Err(e) = alloy_ui_platform::deeplink::register_scheme(&id, &scheme) {
                            log::debug!("url scheme {scheme}: {e}");
                        }
                    }
                });
        }
        alloy_ui_platform::request_window_effect(self.window_effect());
        if let Some((w, h)) = self.min_window_size() {
            alloy_ui_platform::request_min_window_size(w, h);
        }
        let settings = WindowSettings {
            title: title.into(),
            size: alloy_ui_core::Size::new(width, height),
        };
        let result = crate::run(settings, move |ui| Shell::new(self, ui.clone()));
        if let Err(e) = result {
            eprintln!("{title}: {}", e.0);
            std::process::exit(1);
        }
    }
}

/// Adapts an [`App`] to the core `Application` contract: ambient context,
/// the page background, the `Root` wrapper.
pub(crate) struct Shell<A: App> {
    pub(crate) app: A,
    pub(crate) ui: Ui,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
}

impl<A: App> Shell<A> {
    pub(crate) fn new(app: A, ui: Ui) -> Self {
        Shell {
            app,
            ui,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl<A: App> Application for Shell<A> {
    type Message = A::Msg;

    fn update(&mut self, m: A::Msg) -> Command<A::Msg> {
        let (app, ui) = (&mut self.app, self.ui.clone());
        let before = app.snapshot();
        let cmd = cx::enter(&ui, || app.update_cmd(m).0);
        match keys::take_meta() {
            Some(Meta::Undo) => step(app, &mut self.undo, &mut self.redo),
            Some(Meta::Redo) => step(app, &mut self.redo, &mut self.undo),
            None => {
                if let (Some(b), Some(a)) = (before, app.snapshot()) {
                    if !b.same(&a) {
                        self.undo.push(b);
                        self.undo.drain(..self.undo.len().saturating_sub(200));
                        self.redo.clear();
                    }
                }
            }
        }
        cmd
    }

    fn view(&self) -> Element<A::Msg> {
        self.view_window(alloy_ui_core::WindowId::MAIN)
    }

    fn view_window(&self, id: alloy_ui_core::WindowId) -> Element<A::Msg> {
        // Every window has its own focus, scroll and caret state.
        let ui = if id == alloy_ui_core::WindowId::MAIN {
            self.ui.clone()
        } else {
            cx::window_ui_or_fork(id, &self.ui)
        };
        cx::enter(&ui, || {
            cx::pin_density(self.app.density());
            view_subs_begin();
            let page: Node<A::Msg> = self.app.view_window(id.0).into();
            view_subs_commit(id.0);
            let bg = cx::palette().background;
            let bg = if cx::page_alpha() < 1.0 {
                bg.with_alpha(cx::page_alpha())
            } else {
                bg
            };
            let page = Container::new(page.0)
                .background(bg)
                .width(alloy_ui_core::style::Length::Fill(1))
                .height(alloy_ui_core::style::Length::Fill(1))
                .into_element();
            let mut hot = self.app.shortcuts();
            hot.extend(keys::take_registered::<A::Msg>());
            let undo = self.app.meta(Meta::Undo);
            for (what, keys, label) in [
                (Meta::Undo, "mod+z", "Undo"),
                (Meta::Redo, "mod+shift+z", "Redo"),
            ] {
                if let Some(m) = self.app.meta(what) {
                    hot.push(Hotkey::new(keys, m).labelled(label));
                }
            }
            keys::publish_desktop(self.app.desktop());
            keys::publish_toast_layout(self.app.toast_layout());
            keys::publish_menu(
                hot.iter()
                    .filter(|h| !h.keys.is_empty())
                    .filter_map(|h| Some((h.keys.clone(), h.label.clone()?)))
                    .collect(),
                undo.is_some(),
            );
            Root::new(&ui, Element::new(Chrome::new(&ui, page, hot, undo))).into_element()
        })
    }

    fn subscription(&self) -> Vec<Subscription<A::Msg>> {
        let mut subs: Vec<_> = self.app.subscriptions().into_iter().map(|s| s.0).collect();
        subs.extend(view_subs::<A::Msg>());
        subs
    }

    fn host_event(&self, event: HostEvent) -> Option<A::Msg> {
        match event {
            HostEvent::FilesDropped(f) => self.app.on_drop(f),
            HostEvent::FilesHovering(b) => self.app.on_drag_over(b),
            HostEvent::OpenUrl(u) => {
                // Routers made with `Router::links` take their deep links first.
                crate::router::deliver_link(&u);
                self.app.on_open_url(u)
            }
            HostEvent::OpenFiles(f) => self.app.on_open_files(f),
            HostEvent::CloseRequested => self.app.on_close(),
            HostEvent::Focus(f) => {
                crate::cx::set_app_focused(f);
                self.app.on_focus(f)
            }
        }
    }

    fn host_config(&self) -> HostConfig {
        HostConfig {
            effect: self.app.window_effect(),
            system_accent: self.app.system_accent(),
        }
    }
}

/// Swap the current state for the latest snapshot on `from`, keeping the old one on `to`.
fn step<A: App>(app: &mut A, from: &mut Vec<Snap>, to: &mut Vec<Snap>) {
    if let Some(prev) = from.pop() {
        to.extend(app.snapshot());
        app.restore(prev);
    }
}

type Saver<S> = Box<dyn Fn(&S)>;

type FilesAct<S> = std::sync::Arc<dyn Fn(&mut S, Vec<PathBuf>) + Send + Sync>;
type CloseAct<S> = std::sync::Arc<dyn Fn(&mut Closing<'_, S>) + Send + Sync>;
type FocusAct<S> = std::sync::Arc<dyn Fn(&mut S, bool) + Send + Sync>;
type UrlAct<S> = std::sync::Arc<dyn Fn(&mut S, String) + Send + Sync>;

/// A request to close the window, handed to [`Sketch::on_close_requested`]. Do nothing and the
/// window closes; call [`cancel_close`](Self::cancel_close) and it stays open.
pub struct Closing<'a, S> {
    state: &'a S,
    cancelled: bool,
    then: Option<Act<S>>,
}

impl<S: 'static> Closing<'_, S> {
    /// The app state at the moment the user asked to close.
    pub fn state(&self) -> &S {
        self.state
    }

    /// Keep the window open ("close to the menu bar": the app keeps running in the tray).
    pub fn cancel_close(&mut self) {
        self.cancelled = true;
    }

    /// Keep the window open but hide it: "close to the menu bar". The app keeps running and a
    /// tray or menu handler brings the window back with [`show_window`](crate::show_window).
    pub fn hide(&mut self) {
        self.cancelled = true;
        crate::keys::hide_window();
    }

    /// Same as [`hide`](Self::hide).
    pub fn hide_window(&mut self) {
        self.hide();
    }

    /// Keep the window open and run `f` on the state, e.g. to open a "Save changes?" prompt
    /// or remember that the window is hidden.
    pub fn cancel_close_then(&mut self, f: impl Fn(&mut S) + Send + Sync + 'static) {
        self.cancelled = true;
        self.then = Some(Act::new(f));
    }

    /// True once [`cancel_close`](Self::cancel_close) was called.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

/// How a [`Sketch`] reacts to the host.
struct Hooks<S> {
    drop: Option<FilesAct<S>>,
    open_files: Option<FilesAct<S>>,
    open_url: Option<UrlAct<S>>,
    instance: Option<String>,
    schemes: Vec<String>,
    effect: WindowEffect,
    system_accent: bool,
    close: Option<CloseAct<S>>,
    focus: Option<FocusAct<S>>,
}

impl<S> Default for Hooks<S> {
    fn default() -> Self {
        Hooks {
            drop: None,
            open_files: None,
            open_url: None,
            instance: None,
            schemes: Vec::new(),
            effect: WindowEffect::None,
            system_accent: false,
            close: None,
            focus: None,
        }
    }
}

type Cond<S> = Box<dyn Fn(&S) -> bool>;

struct Tick<S> {
    period: Duration,
    when: Option<Cond<S>>,
    act: Act<S>,
}

/// An app with no message type: state plus a view whose handlers mutate it
/// directly (`button("+").act(|n| *n += 1)`). Made by [`sketch`].
pub struct Sketch<S: 'static, V> {
    state: S,
    view: V,
    ticks: Vec<Tick<S>>,
    save: Option<Saver<S>>,
    keys: Vec<(String, Option<String>, Act<S>)>,
    undo: Option<Undo<S>>,
    desktop: crate::Desktop<S>,
    min_size: Option<(f32, f32)>,
    hooks: Hooks<S>,
    toasts: (f32, usize),
    density: Option<alloy_ui_theme::Density>,
}

struct Undo<S> {
    snap: fn(&S) -> Snap,
    put: fn(&mut S, Snap),
}

impl<S: 'static, V> Sketch<S, V> {
    /// Keep the state between runs: load it from the user's config directory
    /// now (keeping the initial value if there is none) and save it after
    /// every change. `name` picks the file, usually your app's name.
    pub fn persist(mut self, name: &str) -> Self
    where
        S: serde::Serialize + serde::de::DeserializeOwned,
    {
        let path = crate::persist::path(name);
        if let Some(s) = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            self.state = s;
        }
        self.save = Some(Box::new(move |s| crate::persist::write(&path, s)));
        self
    }

    /// Run `f` on the state when `keys` is pressed (`"mod+s"`: Cmd on macOS, Ctrl elsewhere).
    pub fn shortcut(mut self, keys: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.keys.push((keys.into(), None, Act::new(f)));
        self
    }

    /// Add `label` to the command palette (`mod+k`): `.command("Export CSV", |s| s.export())`.
    /// Every shortcut is listed there too.
    pub fn command(mut self, label: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.keys
            .push((String::new(), Some(label.into()), Act::new(f)));
        self
    }

    /// Like [`shortcut`](Self::shortcut), and listed in the menu bar as `label`.
    pub fn shortcut_as(
        mut self,
        keys: &str,
        label: &str,
        f: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        self.keys
            .push((keys.into(), Some(label.into()), Act::new(f)));
        self
    }

    /// Add a menu to the menu bar: `.menu("File", |m| m.item("Open", "mod+o", |s| s.open()))`.
    /// Native on macOS and Windows, and in the global menu on KDE and other desktops that have
    /// one; elsewhere the shortcuts and the command palette (`mod+k`) still work. Each item is
    /// `item(label, shortcut, action)` with `""` for no shortcut. See [`quit`](crate::quit).
    pub fn menu(
        mut self,
        title: &str,
        build: impl FnOnce(crate::MenuBuilder<S>) -> crate::MenuBuilder<S>,
    ) -> Self {
        self.desktop.menu(title, build);
        self
    }

    /// Set the application menu, the first one in the macOS menu bar (named after the app):
    /// `.app_menu(|m| m.about().item("Settings...", "mod+,", |s| s.settings()).separator()
    /// .hide().hide_others().show_all().separator().quit())`. Standard edit items go in a
    /// regular menu with [`MenuBuilder::cut`](crate::MenuBuilder::cut) and friends. Other OSes
    /// have no such menu; the custom items' shortcuts still work.
    pub fn app_menu(
        mut self,
        build: impl FnOnce(crate::AppMenuBuilder<S>) -> crate::AppMenuBuilder<S>,
    ) -> Self {
        self.desktop.app_menu(build);
        self
    }

    /// Refuse to let the window shrink below `width` x `height` logical pixels (macOS
    /// `setContentMinSize:`, the same on Windows and Linux). [`Harness::resize`](crate::Harness::resize)
    /// clamps to it too, so tests see what users see.
    pub fn min_window_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Some((width, height));
        self
    }

    /// Put an icon in the system tray (menu bar on macOS) with a context menu:
    /// `.tray("My app", |t| t.click(|s| s.show()).item("Quit", |_| quit()))`.
    pub fn tray(
        mut self,
        tooltip: &str,
        build: impl FnOnce(crate::TrayBuilder<S>) -> crate::TrayBuilder<S>,
    ) -> Self {
        self.desktop.tray(tooltip, build);
        self
    }

    /// Run `f` when `keys` is pressed anywhere on the desktop, even while another app has the
    /// focus: `.global_hotkey("ctrl+alt+space", |s| s.toggle())`. Works on Windows, macOS and
    /// X11; on Wayland the desktop offers no way to do it, so it is logged and skipped.
    pub fn global_hotkey(mut self, keys: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.desktop.global_hotkey(keys, f);
        self
    }

    /// Pixels kept free between the toast stack and the window bottom (default 24). Raise it to
    /// clear a status bar.
    pub fn toast_inset(mut self, px: f32) -> Self {
        self.toasts.0 = px.max(0.0);
        self
    }

    /// One switch for how roomy every page is: `Density::Compact` (about 80% spacing) tightens
    /// paddings, gaps and default row heights, `Density::Spacious` relaxes them. Read it back
    /// with [`density()`](crate::density). Wins over a [`Look`](crate::Look)'s density.
    pub fn density(mut self, d: alloy_ui_theme::Density) -> Self {
        self.density = Some(d);
        self
    }

    /// The most toasts shown at once (default 4); the oldest is dropped first. At least 1.
    pub fn toast_max(mut self, n: usize) -> Self {
        self.toasts.1 = n.max(1);
        self
    }

    /// Undo and redo for every change to the state, on `mod+z` and `mod+shift+z` (and the
    /// Undo button of [`toast(..).undo()`](crate::toast)). Keeps a copy of the state per change,
    /// so it needs `Clone + PartialEq`.
    pub fn undoable(mut self) -> Self
    where
        S: Clone + PartialEq,
    {
        self.undo = Some(Undo {
            snap: |s| Snap::new(s.clone()),
            put: |s, snap| {
                if let Some(v) = snap.take::<S>() {
                    *s = v;
                }
            },
        });
        self
    }

    /// Run `f` with the files dropped on the window from another app:
    /// `.on_drop(|s, files| s.import(files))`. Show a hint with [`files_hovering`](crate::files_hovering).
    pub fn on_drop(mut self, f: impl Fn(&mut S, Vec<PathBuf>) + Send + Sync + 'static) -> Self {
        self.hooks.drop = Some(std::sync::Arc::new(f));
        self
    }

    /// Run `f` when the user asks to close the window (close button, Alt+F4, Cmd+W). Call
    /// `cx.cancel_close()` to keep it open, e.g. to live on in the tray:
    /// `.on_close_requested(|cx| cx.cancel_close())`. Doing nothing lets the window close.
    /// Quit for real with [`quit`](crate::quit) from a menu or tray item.
    pub fn on_close_requested(
        mut self,
        f: impl Fn(&mut Closing<'_, S>) + Send + Sync + 'static,
    ) -> Self {
        self.hooks.close = Some(std::sync::Arc::new(f));
        self
    }

    /// Run `f` when the window gains (`true`) or loses (`false`) focus. Views can also read
    /// [`app_focused`](crate::app_focused) directly.
    pub fn on_focus_changed(mut self, f: impl Fn(&mut S, bool) + Send + Sync + 'static) -> Self {
        self.hooks.focus = Some(std::sync::Arc::new(f));
        self
    }

    /// Run `f` with a deep link (`myapp://...`) the OS opened in this app. Needs
    /// [`url_scheme`](Self::url_scheme); pair with [`single_instance`](Self::single_instance)
    /// so a link opened while the app runs reaches the running copy.
    pub fn on_open_url(mut self, f: impl Fn(&mut S, String) + Send + Sync + 'static) -> Self {
        self.hooks.open_url = Some(std::sync::Arc::new(f));
        self
    }

    /// Run `f` with files the OS asked the app to open (file association, "Open with").
    pub fn on_open_files(
        mut self,
        f: impl Fn(&mut S, Vec<PathBuf>) + Send + Sync + 'static,
    ) -> Self {
        self.hooks.open_files = Some(std::sync::Arc::new(f));
        self
    }

    /// Only one copy runs: launching again hands the arguments to it and exits.
    /// `id` is a reverse-DNS name such as `"com.example.notes"`.
    pub fn single_instance(mut self, id: &str) -> Self {
        self.hooks.instance = Some(id.into());
        self
    }

    /// Handle `scheme://...` links (registered for the user at start-up on Linux and Windows;
    /// on macOS list it under `url-schemes` in `[package.metadata.alloy]`).
    pub fn url_scheme(mut self, scheme: &str) -> Self {
        self.hooks.schemes.push(scheme.into());
        self
    }

    /// A see-through window: `.window_effect(WindowEffect::Mica)`.
    pub fn window_effect(mut self, effect: WindowEffect) -> Self {
        self.hooks.effect = effect;
        self
    }

    /// Follow the user's system accent colour.
    pub fn system_accent(mut self) -> Self {
        self.hooks.system_accent = true;
        self
    }

    /// Run `f` on the state every `period`, for as long as the app runs.
    pub fn tick(mut self, period: Duration, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.ticks.push(Tick {
            period,
            when: None,
            act: Act::new(f),
        });
        self
    }

    /// Like [`tick`](Self::tick) but only while `when(state)` holds: no timer runs, and no
    /// CPU is used, while it does not. `.tick_while(|t| t.running, secs(1.0), Timer::tick)`.
    pub fn tick_while(
        mut self,
        when: impl Fn(&S) -> bool + 'static,
        period: Duration,
        f: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        self.ticks.push(Tick {
            period,
            when: Some(Box::new(when)),
            act: Act::new(f),
        });
        self
    }
}

/// Build a whole app from a state value and a view function:
///
/// ```no_run
/// use alloy_ui::prelude::*;
/// fn main() {
///     sketch(0, |n: &i64| col![n.to_string(), button("+").act(|n: &mut i64| *n += 1)]).run("Hi");
/// }
/// ```
pub fn sketch<S, V, R>(state: S, view: V) -> Sketch<S, V>
where
    S: 'static,
    V: Fn(&S) -> R + 'static,
    R: View<Act<S>>,
{
    Sketch {
        state,
        view,
        ticks: Vec::new(),
        save: None,
        keys: Vec::new(),
        undo: None,
        desktop: crate::Desktop::default(),
        min_size: None,
        hooks: Hooks::default(),
        toasts: (24.0, 4),
        density: None,
    }
}

impl<S, V, R> App for Sketch<S, V>
where
    S: 'static,
    V: Fn(&S) -> R + 'static,
    R: View<Act<S>>,
{
    type Msg = Act<S>;

    fn update(&mut self, m: Act<S>) {
        (m.0)(&mut self.state);
        if let Some(save) = &self.save {
            save(&self.state);
        }
    }

    fn view(&self) -> impl View<Act<S>> {
        (self.view)(&self.state)
    }

    fn shortcuts(&self) -> Vec<Hotkey<Act<S>>> {
        self.keys
            .iter()
            .map(|(k, l, a)| {
                let h = Hotkey::new(k, a.clone());
                match l {
                    Some(l) => h.labelled(l),
                    None => h,
                }
            })
            .chain(self.desktop.hotkeys())
            .collect()
    }

    fn desktop(&self) -> crate::DesktopSpec {
        self.desktop.spec()
    }

    fn min_window_size(&self) -> Option<(f32, f32)> {
        self.min_size
    }

    fn toast_layout(&self) -> (f32, usize) {
        self.toasts
    }

    fn density(&self) -> Option<alloy_ui_theme::Density> {
        self.density
    }

    fn snapshot(&self) -> Option<Snap> {
        self.undo.as_ref().map(|u| (u.snap)(&self.state))
    }

    fn restore(&mut self, snap: Snap) {
        if let Some(u) = &self.undo {
            (u.put)(&mut self.state, snap);
            if let Some(save) = &self.save {
                save(&self.state);
            }
        }
    }

    fn meta(&self, what: Meta) -> Option<Act<S>> {
        self.undo.as_ref().map(|_| keys::meta_act(what))
    }

    fn on_drop(&self, files: Vec<PathBuf>) -> Option<Act<S>> {
        let f = self.hooks.drop.clone()?;
        Some(Act::new(move |s| f(s, files.clone())))
    }

    fn on_close(&self) -> Option<Act<S>> {
        let f = self.hooks.close.clone()?;
        let mut req = Closing {
            state: &self.state,
            cancelled: false,
            then: None,
        };
        f(&mut req);
        if !req.cancelled {
            return None;
        }
        Some(req.then.unwrap_or_else(|| Act::new(|_| {})))
    }

    fn on_focus(&self, focused: bool) -> Option<Act<S>> {
        let f = self.hooks.focus.clone()?;
        Some(Act::new(move |s| f(s, focused)))
    }

    fn on_open_files(&self, files: Vec<PathBuf>) -> Option<Act<S>> {
        let f = self.hooks.open_files.clone()?;
        Some(Act::new(move |s| f(s, files.clone())))
    }

    fn on_open_url(&self, url: String) -> Option<Act<S>> {
        let f = self.hooks.open_url.clone()?;
        Some(Act::new(move |s| f(s, url.clone())))
    }

    fn single_instance(&self) -> Option<String> {
        self.hooks.instance.clone()
    }

    fn url_schemes(&self) -> Vec<String> {
        self.hooks.schemes.clone()
    }

    fn window_effect(&self) -> WindowEffect {
        self.hooks.effect
    }

    fn system_accent(&self) -> bool {
        self.hooks.system_accent
    }

    fn subscriptions(&self) -> Vec<Sub<Act<S>>> {
        let state = &self.state;
        let ticks = self
            .ticks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.when.as_ref().map_or(true, |w| w(state)))
            .map(|(i, t)| {
                let act = t.act.clone();
                let id = 0x7_1c4 + i as u64;
                Sub(Subscription::every(id, t.period, move || act.clone()))
            });
        ticks
            .chain(crate::resource::pump_sub())
            .chain(crate::live::poll_sub())
            .collect()
    }
}
