//! Navigation between screens: a typed route stack with back and forward.
//!
//! ```ignore
//! #[derive(Clone)]
//! enum Route { List, Note(usize) }
//!
//! sketch(Notes::default(), |s| {
//!     let nav = router(Route::List);
//!     nav.show(|route| match route {
//!         Route::List => button("Open first").act(move |_| nav.push(Route::Note(0))).into(),
//!         Route::Note(i) => col![nav.back_button(), text(format!("Note {i}"))].into(),
//!     })
//! })
//! ```

use crate::{
    blocks::{local, Local},
    dsl::{button, caption, Act},
    view::{row, Node},
    Shortcuts,
};

struct Stack<R> {
    /// Visited routes, oldest first; the last one is on screen.
    past: Vec<R>,
    /// Routes that `back` stepped over, nearest first, for `forward`.
    future: Vec<R>,
    /// Bumped by `replace` and `reset`, so those count as moving on.
    rev: u64,
}

/// The history of screens for one window. A small `Copy` handle made by [`router`], so any
/// handler can capture it and call [`push`](Self::push) or [`back`](Self::back).
pub struct Router<R: Send + 'static>(Local<Stack<R>>);

impl<R: Send + 'static> Clone for Router<R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<R: Send + 'static> Copy for Router<R> {}

/// Start (or continue) a navigation stack that begins at `home`. Keyed by call site, like
/// [`local`], so call it once per view and pass the handle to whatever needs to navigate.
#[track_caller]
pub fn router<R: Send + 'static>(home: R) -> Router<R> {
    Router(local(Stack {
        past: vec![home],
        future: Vec::new(),
        rev: 0,
    }))
}

impl<R: Clone + Send + 'static> Router<R> {
    /// Go to `route`; the current screen stays reachable with [`back`](Self::back).
    /// Clears the forward history, like a browser.
    pub fn push(self, route: R) {
        self.0.with(|s| {
            s.past.push(route);
            s.future.clear();
        });
    }
    /// Swap the current screen for `route` without adding a history step (a login that
    /// becomes the home screen, a wizard moving on).
    pub fn replace(self, route: R) {
        self.0.with(|s| {
            *s.past.last_mut().expect("never empty") = route;
            s.rev += 1;
        });
    }
    /// Return to the previous screen. Does nothing at the first one.
    pub fn back(self) {
        self.0.with(|s| {
            if s.past.len() > 1 {
                let r = s.past.pop().expect("len > 1");
                s.future.insert(0, r);
            }
        });
    }
    /// Undo a [`back`](Self::back).
    pub fn forward(self) {
        self.0.with(|s| {
            if !s.future.is_empty() {
                let r = s.future.remove(0);
                s.past.push(r);
            }
        });
    }
    /// Drop all history and show `route` as the only screen.
    pub fn reset(self, route: R) {
        self.0.with(|s| {
            s.past = vec![route];
            s.future.clear();
            s.rev += 1;
        });
    }
    /// The screen on top.
    pub fn current(self) -> R {
        self.0
            .with(|s| s.past.last().cloned().expect("never empty"))
    }
    /// Every screen from the first to the current one.
    pub fn trail(self) -> Vec<R> {
        self.0.with(|s| s.past.clone())
    }
    /// True if there is a screen to go back to.
    pub fn can_back(self) -> bool {
        self.0.with(|s| s.past.len() > 1)
    }
    /// True if a [`back`](Self::back) can be undone.
    pub fn can_forward(self) -> bool {
        self.0.with(|s| !s.future.is_empty())
    }
    /// How many screens deep the stack is (1 at home).
    pub fn depth(self) -> usize {
        self.0.with(|s| s.past.len())
    }

    /// Build the screen for the current route. Also binds `alt+←` and `alt+→` (back and
    /// forward) while it is shown, so every app gets browser-style keyboard navigation.
    pub fn show<S: 'static>(self, screen: impl FnOnce(R) -> Node<Act<S>>) -> Node<Act<S>> {
        let page = screen(self.current());
        page.shortcut("alt+ArrowLeft", move |_: &mut S| self.back())
            .shortcut("alt+ArrowRight", move |_: &mut S| self.forward())
    }

    /// [`show`](Self::show) with page transitions: pushing slides forward, going back slides
    /// the other way, and [`hero`](crate::hero) elements shared between the two screens fly from
    /// one to the other. Idle once the transition ends; reduced motion switches instantly.
    #[track_caller]
    pub fn show_with<S: 'static>(
        self,
        kind: crate::RouteKind,
        screen: impl FnOnce(R) -> Node<Act<S>>,
    ) -> Node<Act<S>> {
        let page = self
            .0
            .with(|s| ((s.past.len() as u64) << 20) | (s.rev & 0xF_FFFF));
        crate::fx::route_page(page, kind, self.show(screen))
    }

    /// A "Back" button, disabled at the first screen.
    pub fn back_button<S: 'static>(self) -> Node<Act<S>> {
        let b = button("Back").ghost();
        if self.can_back() {
            b.act(move |_: &mut S| self.back()).into()
        } else {
            b.into()
        }
    }

    /// A row of steps from home to here: `Home › Notes › Note 3`. Every step but the last is a
    /// link back to it. `name` gives each route its label.
    pub fn breadcrumbs<S: 'static>(self, name: impl Fn(&R) -> String) -> Node<Act<S>> {
        let trail = self.trail();
        let last = trail.len() - 1;
        let mut items: Vec<Node<Act<S>>> = Vec::new();
        for (i, r) in trail.iter().enumerate() {
            if i > 0 {
                items.push(caption("›").into());
            }
            let label = name(r);
            items.push(if i == last {
                caption(label).into()
            } else {
                button(label)
                    .ghost()
                    .act(move |_: &mut S| self.0.with(|s| s.past.truncate(i + 1)))
                    .into()
            });
        }
        row(items).align_center().gap(6.0).into()
    }
}
