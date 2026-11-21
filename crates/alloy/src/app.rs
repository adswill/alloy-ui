//! The application trait, commands, subscriptions and the `run` entry points.

use crate::{
    chrome::Chrome,
    cx,
    dsl::Act,
    keys::{self, Hotkey, Meta, Snap},
    view::{Node, View},
};
use alloy_ui_core::{Application, Command, Element, Subscription};
use alloy_ui_platform::WindowSettings;
use alloy_ui_widgets::{Container, Root, Ui};
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

    /// Open an 800x600 window and run until it closes.
    fn run(self, title: &str) {
        self.run_sized(title, 800.0, 600.0);
    }

    /// Open a window of the given logical size and run until it closes.
    ///
    /// `--headless [file.png]` renders one frame to a PNG instead (after
    /// pressing every `--click <text>`, `--rclick <text>`, `--hover <text>`, `--key <name>`, `--chord <keys>` and `--type <text>` in order), for CI and docs.
    fn run_sized(self, title: &str, width: f32, height: f32) {
        let args: Vec<String> = std::env::args().skip(1).collect();
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
                    _ => {}
                }
            }
            h.settle();
            let out = args.get(at + 1).filter(|a| !a.starts_with("--"));
            let path = out
                .cloned()
                .unwrap_or_else(|| format!("{}.png", title.replace(' ', "_")));
            h.save_png(&path);
            println!("wrote {path}");
            return;
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
            let page: Node<A::Msg> = self.app.view_window(id.0).into();
            let bg = cx::palette().background;
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
        self.app.subscriptions().into_iter().map(|s| s.0).collect()
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
            .collect()
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
        ticks.chain(crate::resource::pump_sub()).collect()
    }
}
