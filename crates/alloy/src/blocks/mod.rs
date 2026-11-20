//! High-level building blocks: view-local state, form fields and the app shell.

use crate::{
    app::App,
    bind::Place,
    cx::{self, auto_id},
    dsl::{button, caption, space, text, Act},
    view::View,
    view::{col, Flex, Node},
};
use alloy_ui_core::style::Length;
use alloy_ui_widgets as w;
use std::sync::Arc;

mod pages;
mod table;
mod wizard;

pub use pages::*;
pub use table::*;
pub use wizard::*;

// ------------------------------------------------------------ local state

/// A value that lives with the widget tree, not in your app state: a search
/// box, a sort order, an open panel. Made by [`local`] or [`store`]. It is a small
/// `Copy` handle, so closures can capture it freely with no `.clone()`.
pub struct Local<T>(u64, std::marker::PhantomData<fn() -> T>);

impl<T> Clone for Local<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Local<T> {}

impl<T: Send + 'static> Local<T> {
    /// Replace the value.
    pub fn set(self, v: T) {
        self.with(|x| *x = v);
    }
    /// Read or change the value in place.
    pub fn with<R>(self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut cx::slot_by_id::<T>(self.0).lock().unwrap())
    }
}
impl<T: Clone + Send + 'static> Local<T> {
    /// A copy of the value.
    pub fn get(self) -> T {
        self.with(|v| v.clone())
    }
}
impl<T: Clone + Send + std::ops::Not<Output = T> + 'static> Local<T> {
    /// Flip a `bool`.
    pub fn toggle(self) {
        self.with(|v| *v = !v.clone());
    }
}

/// View-local state: `let q = local(String::new());` keeps its value between
/// frames (keyed by the call site) with no field in your state and no message.
///
/// Wire a widget to it with `.local(&q)`, or change it in a handler with
/// `.act(move |_| q.set(..))`.
#[track_caller]
pub fn local<T: Send + 'static>(init: T) -> Local<T> {
    let id = auto_id(20);
    cx::slot(id, || init);
    Local(id, std::marker::PhantomData)
}

/// Like [`local`] but saved: with [`Pages::persist`] the value survives restarts, and
/// it is keyed by the page it is built in plus call order, so it needs no name.
/// `init` runs only when there is nothing saved.
pub fn store<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
    init: impl FnOnce() -> T,
) -> Local<T> {
    Local(cx::durable(init), std::marker::PhantomData)
}

/// A [`store`] with a name, so several pages share it: `store_as("sort", || Sort::Recent)`.
pub fn store_as<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
    name: &str,
    init: impl FnOnce() -> T,
) -> Local<T> {
    Local(
        cx::durable_keyed(format!("={name}"), init),
        std::marker::PhantomData,
    )
}

// ---------------------------------------------------------------- forms

/// A labelled form field with an optional validation message under it:
/// `field("Email", input(&s.email).bind(..), (!s.email.contains('@')).then_some("Needs an @"))`.
pub fn field<M: 'static>(label: &str, body: impl Into<Node<M>>, error: Option<&str>) -> Flex<M> {
    let f = col([text(label).label().dim().into(), body.into()]).gap(4.0);
    match error {
        Some(e) => f.push(caption(e).danger()),
        None => f,
    }
}

// ------------------------------------------------------------- app shell

/// A sidebar of pages next to the current page's `body`: `shell(&s.page, |s| &mut s.page, body)`.
/// `page` is a [`choices!`](crate::choices) enum.
pub fn shell<T, S>(
    page: &T,
    lens: impl for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync + 'static,
    body: impl Into<Node<Act<S>>>,
) -> Node<Act<S>>
where
    T: crate::Choice + Send + Sync,
    S: 'static,
{
    use crate::view::View;
    let side: Node<Act<S>> = crate::nav(page).bind(lens).into();
    crate::view::row([
        side.surface().w(170.0).fill_h().into(),
        sidebar_edge(),
        body.into(),
    ])
    .gap(0.0)
    .fill()
    .into()
}

/// The hairline between a sidebar and its page.
fn sidebar_edge<M: 'static>() -> Node<M> {
    use crate::view::View;
    crate::view::col(Vec::<Node<M>>::new())
        .w(1.0)
        .fill_h()
        .bg(cx::palette().border)
        .into()
}

/// A Light / Dark switch for the whole window.
pub fn theme_switch<S: 'static>() -> Node<Act<S>> {
    let dark = store_as("dark", || false);
    crate::choose([("Light", false), ("Dark", true)], dark.get())
        .on(move |d| {
            let dark = dark;
            Act::new(move |_: &mut S| dark.set(d))
        })
        .into()
}
