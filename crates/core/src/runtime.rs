//! Application trait, side-effect commands and event subscriptions.

use crate::widget::Element;
use std::{future::Future, pin::Pin};

/// A boxed future that resolves to a message.
pub type BoxFuture<M> = Pin<Box<dyn Future<Output = M> + Send + 'static>>;

/// A side effect requested by `update`.
pub enum Command<M> {
    /// Do nothing.
    None,
    /// Run futures in the background; each result becomes a message.
    Perform(Vec<BoxFuture<M>>),
    /// Ask the runtime to close every window and exit.
    Exit,
    /// Open a window; its content comes from [`Application::view_window`].
    OpenWindow(WindowId, WindowSpec),
    /// Close a window. Closing the last window exits.
    CloseWindow(WindowId),
}

/// Identifies a window. [`WindowId::MAIN`] is the one every app starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

impl WindowId {
    /// The first window, opened at startup.
    pub const MAIN: WindowId = WindowId(0);
}

/// Title and size for a new window.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowSpec {
    /// Window title.
    pub title: String,
    /// Initial logical size.
    pub size: crate::geometry::Size,
}

/// A window change the shell must carry out; see [`Runtime::take_window_requests`].
#[derive(Debug, Clone, PartialEq)]
pub enum WindowRequest {
    /// Create this window.
    Open(WindowId, WindowSpec),
    /// Close this window.
    Close(WindowId),
}

impl<M> Command<M> {
    /// Open a window titled `title`.
    pub fn open_window(
        id: WindowId,
        title: impl Into<String>,
        size: crate::geometry::Size,
    ) -> Self {
        Command::OpenWindow(
            id,
            WindowSpec {
                title: title.into(),
                size,
            },
        )
    }

    /// Run one future in the background.
    pub fn perform(fut: impl Future<Output = M> + Send + 'static) -> Self {
        Command::Perform(vec![Box::pin(fut)])
    }
}

/// A long-lived source of messages. Returned from [`Application::subscription`];
/// the shell starts and stops sources by diffing their `id`s after every update.
pub struct Subscription<M> {
    /// Stable identity: the same id across updates keeps the source running.
    pub id: u64,
    /// What produces the messages.
    pub kind: SubscriptionKind<M>,
}

/// The kinds of message source the shell can run.
pub enum SubscriptionKind<M> {
    /// Fire `make()` every `interval_ms` milliseconds.
    Every {
        /// Period in milliseconds (clamped to at least 1).
        interval_ms: u64,
        /// Builds each message.
        make: std::sync::Arc<dyn Fn() -> M + Send + Sync>,
    },
}

impl<M> Subscription<M> {
    /// A timer producing `make()` every `interval`.
    pub fn every(
        id: u64,
        interval: std::time::Duration,
        make: impl Fn() -> M + Send + Sync + 'static,
    ) -> Self {
        Subscription {
            id,
            kind: SubscriptionKind::Every {
                interval_ms: (interval.as_millis() as u64).max(1),
                make: std::sync::Arc::new(make),
            },
        }
    }
}

/// The Elm-style application contract.
pub trait Application: Sized {
    /// Messages produced by widgets, commands and subscriptions.
    type Message: Send + 'static;

    /// Handle a message and return follow-up effects.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message>;
    /// Describe the UI for the current state.
    fn view(&self) -> Element<Self::Message>;
    /// Describe the UI of window `id`. Defaults to [`Application::view`], so
    /// single-window apps need not implement it.
    fn view_window(&self, _id: WindowId) -> Element<Self::Message> {
        self.view()
    }
    /// Background event sources for the current state (e.g. timers).
    fn subscription(&self) -> Vec<Subscription<Self::Message>> {
        Vec::new()
    }
}

/// What the shell must do after the runtime handled input.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    /// The frame changed and must be presented.
    pub redraw: bool,
    /// The application asked to exit.
    pub exit: bool,
    /// Some widget is animating: the shell should deliver [`Event::Tick`](crate::Event::Tick) again in about 16 ms.
    pub tick: bool,
}

fn tree_animating<M>(e: &Element<M>) -> bool {
    e.0.animating() || e.0.children().iter().any(tree_animating)
}

/// Computes absolute layout for a view tree at a window size.
///
/// Injected so `core` does not depend on the layout engine.
pub type Layouter<M> = fn(&Element<M>, crate::geometry::Size) -> crate::widget::Layout;

struct Win<M> {
    view: Element<M>,
    layout: crate::widget::Layout,
    size: crate::geometry::Size,
    cursor: crate::geometry::Point,
    last_frame: crate::draw::DrawList,
}

/// Drives an [`Application`]: owns state, every window's view tree and layout,
/// routes events to widgets, runs `update`, and decides when to redraw.
///
/// The runtime is platform-free. The shell feeds it [`Event`](crate::Event)s,
/// spawns the futures in [`Runtime::take_futures`], carries out
/// [`Runtime::take_window_requests`], and presents the windows in
/// [`Runtime::take_dirty`]. A window is redrawn only when its draw list
/// changed, which keeps idle CPU near zero. Methods without a window argument
/// act on [`WindowId::MAIN`].
pub struct Runtime<A: Application> {
    app: A,
    windows: std::collections::BTreeMap<WindowId, Win<A::Message>>,
    layouter: Layouter<A::Message>,
    futures: Vec<BoxFuture<A::Message>>,
    requests: Vec<WindowRequest>,
    dirty: std::collections::BTreeSet<WindowId>,
    inspect: bool,
}

impl<A: Application> Runtime<A> {
    /// Create a runtime and build the main window's first view at `size`.
    pub fn new(app: A, size: crate::geometry::Size, layouter: Layouter<A::Message>) -> Self {
        let mut rt = Self {
            app,
            windows: Default::default(),
            layouter,
            futures: Vec::new(),
            requests: Vec::new(),
            dirty: Default::default(),
            inspect: false,
        };
        rt.add_window(WindowId::MAIN, size);
        rt
    }

    fn add_window(&mut self, id: WindowId, size: crate::geometry::Size) {
        let view = self.app.view_window(id);
        let layout = (self.layouter)(&view, size);
        let mut win = Win {
            view,
            layout,
            size,
            cursor: Default::default(),
            last_frame: Default::default(),
        };
        win.last_frame = Self::draw_win(&win, self.inspect);
        self.windows.insert(id, win);
        self.dirty.insert(id);
    }

    fn draw_win(win: &Win<A::Message>, inspect: bool) -> crate::draw::DrawList {
        let mut list = crate::draw::DrawList::default();
        win.view.0.draw(&win.layout, &mut list);
        if inspect {
            crate::inspector::draw(&win.view, &win.layout, win.cursor, win.size, &mut list);
        }
        list
    }

    /// The application state.
    pub fn app(&self) -> &A {
        &self.app
    }

    /// Show or hide the layout inspector overlay (F12 does this when
    /// [`inspector::available`](crate::inspector::available)).
    pub fn set_inspector(&mut self, on: bool) {
        self.inspect = on;
        for id in self.window_ids() {
            self.refresh(id);
        }
    }

    /// Whether the inspector overlay is showing.
    pub fn inspecting(&self) -> bool {
        self.inspect
    }

    /// Ids of the open windows.
    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.keys().copied().collect()
    }

    /// Draw the main window's tree into a fresh list.
    pub fn draw(&self) -> crate::draw::DrawList {
        self.draw_window(WindowId::MAIN)
    }

    /// Draw window `id`'s tree into a fresh list (empty if it does not exist).
    pub fn draw_window(&self, id: WindowId) -> crate::draw::DrawList {
        self.windows
            .get(&id)
            .map(|w| Self::draw_win(w, self.inspect))
            .unwrap_or_default()
    }

    /// Current subscriptions; the shell diffs them by id.
    pub fn subscriptions(&self) -> Vec<Subscription<A::Message>> {
        self.app.subscription()
    }

    /// Futures requested by `update` since the last call; the shell spawns them.
    pub fn take_futures(&mut self) -> Vec<BoxFuture<A::Message>> {
        std::mem::take(&mut self.futures)
    }

    /// Window opens and closes requested by `update` since the last call.
    pub fn take_window_requests(&mut self) -> Vec<WindowRequest> {
        std::mem::take(&mut self.requests)
    }

    /// Windows whose frame changed since the last call; the shell redraws them.
    pub fn take_dirty(&mut self) -> Vec<WindowId> {
        std::mem::take(&mut self.dirty).into_iter().collect()
    }

    /// The shell closed window `id` (e.g. the user clicked its close button).
    /// Returns an outcome with `exit` set when it was the last window.
    pub fn window_closed(&mut self, id: WindowId) -> Outcome {
        self.windows.remove(&id);
        self.dirty.remove(&id);
        Outcome {
            exit: self.windows.is_empty(),
            ..Outcome::default()
        }
    }

    /// Deliver a message (from a widget, task or subscription).
    pub fn dispatch(&mut self, message: A::Message) -> Outcome {
        self.dispatch_all(vec![message])
    }

    /// Deliver several messages, rebuilding every window's view once.
    pub fn dispatch_all(&mut self, messages: Vec<A::Message>) -> Outcome {
        let mut exit = false;
        for m in messages {
            match self.app.update(m) {
                Command::None => {}
                Command::Perform(mut f) => self.futures.append(&mut f),
                Command::Exit => exit = true,
                Command::OpenWindow(id, spec) => {
                    if !self.windows.contains_key(&id) {
                        self.add_window(id, spec.size);
                        self.requests.push(WindowRequest::Open(id, spec));
                    }
                }
                Command::CloseWindow(id) => {
                    if self.windows.remove(&id).is_some() {
                        self.dirty.remove(&id);
                        self.requests.push(WindowRequest::Close(id));
                        exit |= self.windows.is_empty();
                    }
                }
            }
        }
        let ids: Vec<WindowId> = self.windows.keys().copied().collect();
        let mut out = Outcome {
            exit,
            ..Outcome::default()
        };
        for id in ids {
            if let Some(win) = self.windows.get_mut(&id) {
                win.view = self.app.view_window(id);
            }
            let o = self.refresh(id);
            out.redraw |= o.redraw;
            out.tick |= o.tick;
        }
        out
    }

    /// Route a platform event to the main window's widget tree.
    pub fn handle_event(&mut self, event: &crate::event::Event) -> Outcome {
        self.handle_window_event(WindowId::MAIN, event)
    }

    /// Route a platform event to window `id`'s widget tree.
    pub fn handle_window_event(&mut self, id: WindowId, event: &crate::event::Event) -> Outcome {
        if let crate::event::Event::KeyPressed(crate::event::Key::Named(k), _) = event {
            if k == "F12" && crate::inspector::available() {
                self.set_inspector(!self.inspect);
                return Outcome {
                    redraw: true,
                    ..Outcome::default()
                };
            }
        }
        let Some(win) = self.windows.get_mut(&id) else {
            return Outcome::default();
        };
        match event {
            crate::event::Event::CursorMoved(p) => win.cursor = *p,
            crate::event::Event::Resized(s) => {
                win.size = *s;
                win.view = self.app.view_window(id);
                return self.refresh(id);
            }
            _ => {}
        }
        let mut out = Vec::new();
        win.view
            .0
            .on_event(event, &win.layout, win.cursor, &mut out);
        if out.is_empty() {
            // Widget-local state (hover, caret) may still have changed.
            return self.refresh(id);
        }
        self.dispatch_all(out)
    }

    fn refresh(&mut self, id: WindowId) -> Outcome {
        let Some(win) = self.windows.get_mut(&id) else {
            return Outcome::default();
        };
        win.layout = (self.layouter)(&win.view, win.size);
        let frame = Self::draw_win(win, self.inspect);
        let redraw = frame != win.last_frame;
        win.last_frame = frame;
        if redraw {
            self.dirty.insert(id);
        }
        Outcome {
            redraw,
            exit: false,
            tick: tree_animating(&win.view),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        draw::{DrawCommand, DrawList},
        event::{Event, MouseButton},
        geometry::{Color, Point, Rectangle, Size},
        widget::{Layout, Widget},
    };

    struct Btn;
    impl Widget<i32> for Btn {
        fn measure(&self, _: Size) -> Size {
            Size::new(40.0, 20.0)
        }
        fn on_event(&mut self, e: &Event, l: &Layout, c: Point, out: &mut Vec<i32>) -> bool {
            if matches!(e, Event::MousePressed(MouseButton::Left)) && l.bounds.contains(c) {
                out.push(1);
                return true;
            }
            false
        }
        fn draw(&self, l: &Layout, out: &mut DrawList) {
            out.push(DrawCommand::Quad {
                bounds: l.bounds,
                fill: Color::hex(0x336655),
                radius: 4.0,
                border: None,
                shadow: None,
            });
        }
    }

    struct Counter(i32);
    impl Application for Counter {
        type Message = i32;
        fn update(&mut self, m: i32) -> Command<i32> {
            self.0 += m;
            if self.0 >= 3 {
                Command::Exit
            } else {
                Command::None
            }
        }
        fn view(&self) -> Element<i32> {
            Element::new(Btn)
        }
    }

    fn lay(_: &Element<i32>, s: Size) -> Layout {
        Layout {
            bounds: Rectangle::new(Point::new(0.0, 0.0), Size::new(40.0, 20.0).min(s)),
            children: vec![],
        }
    }

    impl Size {
        fn min(self, o: Size) -> Size {
            Size::new(self.width.min(o.width), self.height.min(o.height))
        }
    }

    #[test]
    fn click_updates_state_and_exits() {
        let mut rt = Runtime::new(Counter(0), Size::new(100.0, 100.0), lay);
        rt.handle_event(&Event::CursorMoved(Point::new(5.0, 5.0)));
        let o = rt.handle_event(&Event::MousePressed(MouseButton::Left));
        assert_eq!(rt.app().0, 1);
        assert!(!o.exit);
        assert!(!o.redraw, "identical frame must not redraw");
        rt.handle_event(&Event::MousePressed(MouseButton::Left));
        let o = rt.handle_event(&Event::MousePressed(MouseButton::Left));
        assert!(o.exit);
    }

    #[test]
    fn click_outside_does_nothing() {
        let mut rt = Runtime::new(Counter(0), Size::new(100.0, 100.0), lay);
        rt.handle_event(&Event::CursorMoved(Point::new(90.0, 90.0)));
        rt.handle_event(&Event::MousePressed(MouseButton::Left));
        assert_eq!(rt.app().0, 0);
    }

    #[test]
    fn resize_changes_frame() {
        let mut rt = Runtime::new(Counter(0), Size::new(100.0, 100.0), lay);
        let o = rt.handle_event(&Event::Resized(Size::new(10.0, 10.0)));
        assert!(o.redraw);
    }

    struct Two(Vec<WindowId>);
    impl Application for Two {
        type Message = i32;
        fn update(&mut self, m: i32) -> Command<i32> {
            match m {
                1 => Command::open_window(WindowId(7), "Second", Size::new(50.0, 50.0)),
                2 => Command::CloseWindow(WindowId(7)),
                _ => Command::CloseWindow(WindowId::MAIN),
            }
        }
        fn view(&self) -> Element<i32> {
            Element::new(Btn)
        }
        fn view_window(&self, _: WindowId) -> Element<i32> {
            let _ = &self.0;
            Element::new(Btn)
        }
    }

    #[test]
    fn open_and_close_windows() {
        let mut rt = Runtime::new(Two(vec![]), Size::new(100.0, 100.0), lay);
        assert_eq!(rt.window_ids(), vec![WindowId::MAIN]);
        rt.dispatch(1);
        assert_eq!(rt.window_ids(), vec![WindowId::MAIN, WindowId(7)]);
        let reqs = rt.take_window_requests();
        assert!(matches!(reqs[0], WindowRequest::Open(WindowId(7), _)));
        assert!(rt.take_dirty().contains(&WindowId(7)));
        assert!(!rt.dispatch(2).exit);
        assert_eq!(rt.window_ids(), vec![WindowId::MAIN]);
        assert_eq!(
            rt.take_window_requests(),
            vec![WindowRequest::Close(WindowId(7))]
        );
        assert!(rt.dispatch(3).exit, "closing the last window exits");
    }

    #[test]
    fn user_closing_last_window_exits() {
        let mut rt = Runtime::new(Two(vec![]), Size::new(100.0, 100.0), lay);
        rt.dispatch(1);
        assert!(!rt.window_closed(WindowId(7)).exit);
        assert!(rt.window_closed(WindowId::MAIN).exit);
    }
}
