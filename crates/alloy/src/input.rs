//! Hover, focus and keyboard for views: `hovered()`, `.on_hover(..)`, focusable `.tap(..)`,
//! arrow-key navigation over a grid, and the "is a text field focused" query.

use crate::{
    blocks::{local_lazy, Local},
    cx,
    dsl::Act,
    view::{Node, View},
};
use alloy_ui_core::{style::Style, Element, Event, Layout, Point, Rectangle, Size, Widget};
use alloy_ui_widgets as w;
use std::sync::{Arc, Mutex};

/// True while keyboard focus is in a text field (an `input`, `editor`, `number`, code or rich
/// editor). Plain keys then belong to that field: app shortcuts without a Ctrl, Cmd or Alt
/// modifier do not fire and arrow-key navigation stands down. Call it from a handler or `view`:
/// `if !text_focused() { s.next() }`.
pub fn text_focused() -> bool {
    cx::ui().text_focused()
}

/// Whether the pointer is over some part of the view, readable while building the view.
///
/// Make one with [`hovered`] (or [`hovered_for`]), attach it with
/// [`Interact::watch_hover`], and read it with [`get`](Self::get):
///
/// ```
/// use alloy_ui::prelude::*;
/// let app = sketch((), |_: &()| {
///     let over = hovered();
///     text(if over.get() { "Play now" } else { "Play" })
///         .pad(12.0)
///         .watch_hover(over)
/// });
/// # let _ = app;
/// ```
#[derive(Clone, Copy)]
pub struct Hover(Local<bool>);

impl Hover {
    /// True while the pointer is over the watched view.
    pub fn get(self) -> bool {
        self.0.get()
    }
}

/// A hover flag identified by the place it is written, for [`Interact::watch_hover`].
#[track_caller]
pub fn hovered() -> Hover {
    Hover(local_lazy(|| false))
}

/// Like [`hovered`], identified by `key` (an item id) rather than by position: rows that
/// reorder or recycle keep their own hover state.
pub fn hovered_for(key: u64) -> Hover {
    Hover(cx::keyed(key, || local_lazy(|| false)))
}

/// Tells the app when the pointer enters or leaves the wrapped view.
struct HoverWatch<M> {
    child: Vec<Element<M>>,
    state: Arc<Mutex<bool>>,
    make: Box<dyn Fn(bool) -> M>,
}

impl<M: 'static> Widget<M> for HoverWatch<M> {
    fn style(&self) -> Style {
        self.child[0].0.style()
    }
    fn measure(&self, available: Size) -> Size {
        self.child[0].0.measure(available)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        w::arrange_widget(self, bounds)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let consumed = self.child[0]
            .0
            .on_event(event, &layout.children[0], cursor, out);
        let over = match event {
            Event::CursorMoved(p) => Some(layout.bounds.contains(*p)),
            Event::CursorLeft => Some(false),
            Event::Scrolled { .. } | Event::Tick(_) => Some(layout.bounds.contains(cursor)),
            _ => None,
        };
        if let Some(now) = over {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if *s != now {
                *s = now;
                out.push((self.make)(now));
            }
        }
        consumed
    }
    fn animating(&self) -> bool {
        self.child[0].0.animating()
    }
    fn draw(&self, layout: &Layout, out: &mut alloy_ui_core::DrawList) {
        self.child[0].0.draw(&layout.children[0], out);
    }
}

fn watch<M: 'static>(
    child: Element<M>,
    state: Local<bool>,
    make: impl Fn(bool) -> M + 'static,
) -> Node<M> {
    Node(Element::new(HoverWatch {
        child: vec![child],
        state: cx::slot_by_id::<bool>(state.id()),
        make: Box::new(make),
    }))
}

/// Pointer, tap and focus helpers for apps whose messages are closures ([`sketch`](crate::sketch)).
pub trait Interact<S: 'static>: View<Act<S>> + Sized {
    /// Make the view clickable and keyboard-focusable: a click, or Enter or Space while it has
    /// focus, runs `f`. It joins the Tab order (Shift+Tab goes back), shows a focus ring and a
    /// quiet hover tint. Its focus identity is its place in the view; inside a list that can
    /// reorder use [`tap_for`](Self::tap_for).
    #[track_caller]
    fn tap(self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Node<Act<S>> {
        let id = cx::auto_id(200);
        tap_node(self.into().0, id, Act::new(f))
    }
    /// Like [`tap`](Self::tap) with the focus identity taken from `key`, so each item of a
    /// list keeps its own focus when the list reorders or rows are recycled.
    fn tap_for(self, key: u64, f: impl Fn(&mut S) + Send + Sync + 'static) -> Node<Act<S>> {
        let id = cx::keyed(key, || cx::auto_id(201));
        tap_node(self.into().0, id, Act::new(f))
    }
    /// Run `f(state, hovered)` when the pointer enters (`true`) or leaves (`false`) the view:
    /// pause a carousel while it is hovered, show an overlay on game art.
    #[track_caller]
    fn on_hover(self, f: impl Fn(&mut S, bool) + Send + Sync + 'static) -> Node<Act<S>> {
        let state = local_lazy(|| false);
        let f = Arc::new(f);
        watch(self.into().0, state, move |h| {
            let f = f.clone();
            Act::new(move |s: &mut S| f(s, h))
        })
    }
    /// Keep `over` up to date with whether the pointer is on the view, so `view` can read
    /// `over.get()` (see [`Hover`]).
    fn watch_hover(self, over: Hover) -> Node<Act<S>> {
        watch(self.into().0, over.0, |_| Act::new(|_: &mut S| {}))
    }
}
impl<S: 'static, V: View<Act<S>>> Interact<S> for V {}

fn tap_node<M: Clone + 'static>(child: Element<M>, id: u64, msg: M) -> Node<M> {
    Node(
        w::Tap::new(&cx::ui(), child)
            .on_press(msg)
            .focusable(id)
            .into_element(),
    )
}

/// Pointer, tap and focus helpers for apps with their own message type
/// ([`App`](crate::App)).
pub trait InteractMsg<M: Clone + 'static>: View<M> + Sized {
    /// Make the view clickable and keyboard-focusable, sending `msg`. See [`Interact::tap`].
    #[track_caller]
    fn on_tap(self, msg: M) -> Node<M> {
        let id = cx::auto_id(202);
        tap_node(self.into().0, id, msg)
    }
    /// Send `f(hovered)` when the pointer enters or leaves the view.
    #[track_caller]
    fn on_hover_msg(self, f: impl Fn(bool) -> M + 'static) -> Node<M> {
        watch(self.into().0, local_lazy(|| false), f)
    }
    /// Let the arrow keys move focus between the focusable widgets inside (taps, buttons,
    /// checkboxes), in two dimensions: wrap a grid of `.tap(..)` cells in it. Arrow keys a
    /// focused widget handles itself, and every key while a text field has focus, are left
    /// alone.
    fn arrow_nav(self) -> Node<M> {
        Node(w::ArrowNav::new(&cx::ui(), self.into().0).into_element())
    }
}
impl<M: Clone + 'static, V: View<M>> InteractMsg<M> for V {}
