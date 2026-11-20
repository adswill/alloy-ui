//! The application trait, commands, subscriptions and the `run` entry points.

use crate::{
    cx,
    dsl::Act,
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
    /// Close the window and exit.
    pub fn exit() -> Self {
        Cmd(Command::Exit)
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

    /// Timers and other long-lived message sources for the current state.
    fn subscriptions(&self) -> Vec<Sub<Self::Msg>> {
        Vec::new()
    }

    /// Open an 800x600 window and run until it closes.
    fn run(self, title: &str) {
        self.run_sized(title, 800.0, 600.0);
    }

    /// Open a window of the given logical size and run until it closes.
    ///
    /// `--headless [file.png]` renders one frame to a PNG instead (after
    /// pressing every `--click <text>` target in order), for CI and docs.
    fn run_sized(self, title: &str, width: f32, height: f32) {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Some(at) = args.iter().position(|a| a == "--headless") {
            let mut h = crate::testing::Harness::new(self, width, height);
            for pair in args.windows(2).filter(|p| p[0] == "--click") {
                h.click(&pair[1]);
            }
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
        let result = crate::run(settings, move |ui| Shell {
            app: self,
            ui: ui.clone(),
        });
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
}

impl<A: App> Application for Shell<A> {
    type Message = A::Msg;

    fn update(&mut self, m: A::Msg) -> Command<A::Msg> {
        let (app, ui) = (&mut self.app, self.ui.clone());
        cx::enter(&ui, || app.update_cmd(m).0)
    }

    fn view(&self) -> Element<A::Msg> {
        cx::enter(&self.ui, || {
            let page: Node<A::Msg> = self.app.view().into();
            let bg = cx::palette().background;
            let page = Container::new(page.0)
                .background(bg)
                .width(alloy_ui_core::style::Length::Fill(1))
                .height(alloy_ui_core::style::Length::Fill(1))
                .into_element();
            Root::new(&self.ui, page).into_element()
        })
    }

    fn subscription(&self) -> Vec<Subscription<A::Msg>> {
        self.app.subscriptions().into_iter().map(|s| s.0).collect()
    }
}

type Saver<S> = Box<dyn Fn(&S)>;

/// An app with no message type: state plus a view whose handlers mutate it
/// directly (`button("+").act(|n| *n += 1)`). Made by [`sketch`].
pub struct Sketch<S: 'static, V> {
    state: S,
    view: V,
    tick: Option<(Duration, Act<S>)>,
    save: Option<Saver<S>>,
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

    /// Run `f` on the state every `period`, for as long as the app runs.
    pub fn tick(mut self, period: Duration, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.tick = Some((period, Act::new(f)));
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
        tick: None,
        save: None,
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

    fn subscriptions(&self) -> Vec<Sub<Act<S>>> {
        self.tick
            .iter()
            .map(|(period, act)| {
                let act = act.clone();
                every(*period, move || act.clone())
            })
            .collect()
    }
}
