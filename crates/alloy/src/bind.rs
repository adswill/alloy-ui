//! Two-way bindings: `input(at!(s.name))` reads the field and writes edits back to it.
//!
//! [`at!`](crate::at) names a field of the state inside `view`. The widget shows its current
//! value and edits it directly, so a form needs no handlers at all:
//!
//! ```ignore
//! col![
//!     input(at!(s.name)).hint("Name"),
//!     check("Subscribe", at!(s.news)),
//!     slider(0.0..=10.0, at!(s.volume)),
//!     button("Reset").set(at!(s.volume), 5.0),
//! ]
//! ```
//!
//! Plain values still work (`input(&s.name)` is read-only until `.on(..)`/`.act(..)`).

use crate::dsl::Act;
use std::sync::Arc;

/// How a place in the state is reached for writing.
pub type Lens<S, T> = Arc<dyn for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync>;

/// A field of the state `S`: its value now, and how to write it later. Made by [`at!`](crate::at).
pub struct Place<'a, S, T> {
    pub(crate) cur: &'a T,
    pub(crate) lens: Lens<S, T>,
}

/// Used by [`at!`](crate::at); `_witness` only fixes `S` so closures need no annotations.
#[doc(hidden)]
pub fn place<'a, S, T>(
    _witness: &S,
    cur: &'a T,
    lens: impl for<'b> Fn(&'b mut S) -> &'b mut T + Send + Sync + 'static,
) -> Place<'a, S, T> {
    Place {
        cur,
        lens: Arc::new(lens),
    }
}

/// A field of the state, for two-way binding: `input(at!(s.name))`, `check("On", at!(s.on))`,
/// `button("Pause").toggle(at!(s.paused))`. `s` is the `&State` your view receives;
/// nested fields (`at!(s.form.email)`) and indexing (`at!(s.minutes[i])`) work too.
#[macro_export]
macro_rules! at {
    ($s:ident $($path:tt)+) => {
        $crate::place(&*$s, &$s $($path)+, move |__s| &mut __s $($path)+)
    };
}

type Setter<T, M> = Box<dyn Fn(T) -> M>;

/// A widget's value: a plain value (read-only until `.on(..)`) or a [`Place`] (edits write back).
pub trait Val<T, M> {
    /// The value to show and the handler that stores edits, if the source is writable.
    fn split(self) -> (T, Option<Setter<T, M>>);
}

impl<T, M> Val<T, M> for T {
    fn split(self) -> (T, Option<Setter<T, M>>) {
        (self, None)
    }
}
impl<T: Clone, M> Val<T, M> for &T {
    fn split(self) -> (T, Option<Setter<T, M>>) {
        (self.clone(), None)
    }
}
impl<M> Val<String, M> for &str {
    fn split(self) -> (String, Option<Setter<String, M>>) {
        (self.to_string(), None)
    }
}
impl<'a, S: 'static, T: Clone + Send + Sync + 'static> Val<T, Act<S>> for Place<'a, S, T> {
    fn split(self) -> (T, Option<Setter<T, Act<S>>>) {
        let lens = self.lens;
        let set: Setter<T, Act<S>> = Box::new(move |v| {
            let lens = lens.clone();
            Act::new(move |s| *lens(s) = v.clone())
        });
        (self.cur.clone(), Some(set))
    }
}

impl<'a, S: 'static, T: 'static> Place<'a, S, T> {
    /// A state change that writes `v` here.
    pub(crate) fn writer(&self, v: T) -> Act<S>
    where
        T: Clone + Send + Sync,
    {
        let lens = self.lens.clone();
        Act::new(move |s| *lens(s) = v.clone())
    }
}

impl<S: 'static> crate::dsl::Btn<Act<S>> {
    /// Flip a `bool` field when pressed: `button("Pause").toggle(at!(s.paused))`.
    pub fn toggle(self, p: Place<'_, S, bool>) -> Self {
        let lens = p.lens;
        self.act(move |s| {
            let v = lens(s);
            *v = !*v;
        })
    }
    /// Write `value` into a field when pressed: `button("Done").set(at!(s.step), Step::Done)`.
    pub fn set<T: Clone + Send + Sync + 'static>(self, p: Place<'_, S, T>, value: T) -> Self {
        self.on(p.writer(value))
    }
    /// Add `by` to a number field when pressed (negative `by` subtracts): `.add(at!(s.n), 1)`.
    pub fn add<T>(self, p: Place<'_, S, T>, by: T) -> Self
    where
        T: std::ops::AddAssign + Copy + Send + Sync + 'static,
    {
        let lens = p.lens;
        self.act(move |s| *lens(s) += by)
    }
}
