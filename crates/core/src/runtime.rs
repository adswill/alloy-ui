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
    /// Ask the runtime to close the window and exit.
    Exit,
}

impl<M> Command<M> {
    /// Run one future in the background.
    pub fn perform(fut: impl Future<Output = M> + Send + 'static) -> Self {
        Command::Perform(vec![Box::pin(fut)])
    }
}

/// A long-lived source of messages (timers, channels, OS events).
pub struct Subscription<M> {
    /// Stable identity so the runtime can diff subscriptions across updates.
    pub id: u64,
    /// Builds the message stream; real implementation lands in the runtime workstream.
    pub _marker: std::marker::PhantomData<M>,
}

/// The Elm-style application contract.
pub trait Application: Sized {
    /// Messages produced by widgets, commands and subscriptions.
    type Message: Send + 'static;

    /// Handle a message and return follow-up effects.
    fn update(&mut self, message: Self::Message) -> Command<Self::Message>;
    /// Describe the UI for the current state.
    fn view(&self) -> Element<Self::Message>;
    /// Background event sources for the current state.
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
}

/// Computes absolute layout for a view tree at a window size.
///
/// Injected so `core` does not depend on the layout engine.
pub type Layouter<M> = fn(&Element<M>, crate::geometry::Size) -> crate::widget::Layout;

/// Drives an [`Application`]: owns state, the current view tree and layout,
/// routes events to widgets, runs `update`, and decides when to redraw.
///
/// The runtime is platform-free. The shell feeds it [`Event`](crate::Event)s,
/// spawns the futures in [`Runtime::take_futures`], and presents the frame
/// when [`Outcome::redraw`] is set. Redraw is skipped when the new draw list
/// equals the previous one, which keeps idle CPU near zero.
pub struct Runtime<A: Application> {
    app: A,
    view: Element<A::Message>,
    layout: crate::widget::Layout,
    layouter: Layouter<A::Message>,
    size: crate::geometry::Size,
    cursor: crate::geometry::Point,
    last_frame: crate::draw::DrawList,
    futures: Vec<BoxFuture<A::Message>>,
}

impl<A: Application> Runtime<A> {
    /// Create a runtime and build the first view at `size`.
    pub fn new(app: A, size: crate::geometry::Size, layouter: Layouter<A::Message>) -> Self {
        let view = app.view();
        let layout = layouter(&view, size);
        let mut rt = Self {
            app,
            view,
            layout,
            layouter,
            size,
            cursor: Default::default(),
            last_frame: Default::default(),
            futures: Vec::new(),
        };
        rt.last_frame = rt.draw();
        rt
    }

    /// The application state.
    pub fn app(&self) -> &A {
        &self.app
    }

    /// Draw the current tree into a fresh list.
    pub fn draw(&self) -> crate::draw::DrawList {
        let mut list = crate::draw::DrawList::default();
        self.view.0.draw(&self.layout, &mut list);
        list
    }

    /// Futures requested by `update` since the last call; the shell spawns them.
    pub fn take_futures(&mut self) -> Vec<BoxFuture<A::Message>> {
        std::mem::take(&mut self.futures)
    }

    /// Deliver a message (from a widget, task or subscription).
    pub fn dispatch(&mut self, message: A::Message) -> Outcome {
        self.dispatch_all(vec![message])
    }

    /// Deliver several messages, rebuilding the view once.
    pub fn dispatch_all(&mut self, messages: Vec<A::Message>) -> Outcome {
        let mut exit = false;
        for m in messages {
            match self.app.update(m) {
                Command::None => {}
                Command::Perform(mut f) => self.futures.append(&mut f),
                Command::Exit => exit = true,
            }
        }
        self.rebuild(exit)
    }

    /// Route a platform event to the widget tree.
    pub fn handle_event(&mut self, event: &crate::event::Event) -> Outcome {
        match event {
            crate::event::Event::CursorMoved(p) => self.cursor = *p,
            crate::event::Event::Resized(s) => {
                self.size = *s;
                return self.rebuild(false);
            }
            _ => {}
        }
        let mut out = Vec::new();
        self.view
            .0
            .on_event(event, &self.layout, self.cursor, &mut out);
        if out.is_empty() {
            // Widget-local state (hover, caret) may still have changed.
            return self.refresh(false);
        }
        self.dispatch_all(out)
    }

    fn rebuild(&mut self, exit: bool) -> Outcome {
        self.view = self.app.view();
        self.refresh(exit)
    }

    fn refresh(&mut self, exit: bool) -> Outcome {
        self.layout = (self.layouter)(&self.view, self.size);
        let frame = self.draw();
        let redraw = frame != self.last_frame;
        self.last_frame = frame;
        Outcome { redraw, exit }
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
}
