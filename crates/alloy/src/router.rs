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
    blocks::{local, local_lazy, Local},
    dsl::{button, caption, Act},
    view::{row, Node},
    Shortcuts,
};

struct Stack<R> {
    /// Visited routes, oldest first; the last one is on screen.
    past: Vec<R>,
    /// Routes that `back` stepped over, nearest first, for `forward`.
    future: Vec<R>,
    /// Writes `past` to disk after every change (see [`router_saved`]).
    save: Option<Box<dyn Fn(&[R]) + Send>>,
}

impl<R> Stack<R> {
    fn changed(&self) {
        if let Some(save) = &self.save {
            save(&self.past);
        }
    }
}

/// The most screens a saved trail keeps; older ones are dropped.
const SAVED_DEPTH: usize = 50;

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
        save: None,
    }))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Saved<R> {
    trail: Vec<R>,
}

/// Like [`router`], but the screen stack survives a restart: it is written to
/// `<app data>/<name>/router.json` after every change and read back at launch, so the app
/// reopens where the person left it. Routes must be serialisable. A file that is missing,
/// corrupt or from an older version of the enum is ignored and the stack starts at `home`.
#[track_caller]
pub fn router_saved<R>(name: &str, home: R) -> Router<R>
where
    R: serde::Serialize + serde::de::DeserializeOwned + Send + 'static,
{
    router_saved_at(
        crate::persist::path(name).with_file_name("router.json"),
        home,
    )
}

/// [`router_saved`] with an explicit file, for apps that keep their data elsewhere and for tests.
#[track_caller]
pub fn router_saved_at<R>(path: impl Into<std::path::PathBuf>, home: R) -> Router<R>
where
    R: serde::Serialize + serde::de::DeserializeOwned + Send + 'static,
{
    let path: std::path::PathBuf = path.into();
    Router(local_lazy(move || {
        let trail = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Saved<R>>(&b).ok())
            .map(|s| s.trail)
            .filter(|t| !t.is_empty());
        Stack {
            past: trail.unwrap_or_else(|| vec![home]),
            future: Vec::new(),
            save: Some(Box::new(move |past: &[R]| {
                let from = past.len().saturating_sub(SAVED_DEPTH);
                let json =
                    serde_json::to_vec_pretty(&serde_json::json!({ "trail": &past[from..] }));
                if let (Ok(json), Some(dir)) = (json, path.parent()) {
                    let _ = std::fs::create_dir_all(dir);
                    let tmp = path.with_extension("tmp");
                    if std::fs::write(&tmp, json).is_ok() {
                        let _ = std::fs::rename(&tmp, &path);
                    }
                }
            })),
        }
    }))
}

/// The pieces of a deep link's path: `link_path("myapp://invite/42?x=1")` is `["invite", "42"]`.
/// The query and fragment are dropped; a link with no `://` is read as a bare path.
pub fn link_path(url: &str) -> Vec<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    rest.split('/')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

type LinkHandler = Box<dyn Fn(&str) -> bool>;

thread_local! {
    /// One handler per router that asked for deep links, keyed by the router's slot.
    static LINKS: std::cell::RefCell<std::collections::HashMap<u64, LinkHandler>> =
        std::cell::RefCell::new(Default::default());
    /// Links that arrived before any router could take them (at launch, before the first view).
    static PENDING: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Hand an opened deep link to the routers that asked for them ([`Router::links`]). Returns
/// true if one took it; otherwise the link waits for the next router that registers.
pub(crate) fn deliver_link(url: &str) -> bool {
    let taken = LINKS.with(|l| l.borrow().values().any(|h| h(url)));
    if !taken {
        PENDING.with(|p| {
            let mut p = p.borrow_mut();
            if p.len() < 8 {
                p.push(url.to_string());
            }
        });
    }
    taken
}

impl<R: Clone + Send + 'static> Router<R> {
    /// Go to `route`; the current screen stays reachable with [`back`](Self::back).
    /// Clears the forward history, like a browser.
    pub fn push(self, route: R) {
        self.0.with(|s| {
            s.past.push(route);
            s.future.clear();
            s.changed();
        });
    }
    /// Swap the current screen for `route` without adding a history step (a login that
    /// becomes the home screen, a wizard moving on).
    pub fn replace(self, route: R) {
        self.0.with(|s| {
            *s.past.last_mut().expect("never empty") = route;
            s.changed();
        });
    }
    /// Return to the previous screen. Does nothing at the first one.
    pub fn back(self) {
        self.0.with(|s| {
            if s.past.len() > 1 {
                let r = s.past.pop().expect("len > 1");
                s.future.insert(0, r);
                s.changed();
            }
        });
    }
    /// Undo a [`back`](Self::back).
    pub fn forward(self) {
        self.0.with(|s| {
            if !s.future.is_empty() {
                let r = s.future.remove(0);
                s.past.push(r);
                s.changed();
            }
        });
    }
    /// Drop all history and show `route` as the only screen.
    pub fn reset(self, route: R) {
        self.0.with(|s| {
            s.past = vec![route];
            s.future.clear();
            s.changed();
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

    /// Open screens from deep links (`myapp://note/3`): `parse` turns the link into a route
    /// (`None` ignores it) and the router pushes it, unless it is already on screen. Works for
    /// links that arrive while the app runs and for the one that launched it. Call it where you
    /// make the router, and register the scheme with `url_schemes` (or `.url_scheme(..)` on a
    /// sketch). Use [`link_path`] to split the URL.
    ///
    /// ```ignore
    /// let nav = router(Route::List).links(|url| match link_path(url).as_slice() {
    ///     [a, b] if a == "note" => b.parse().ok().map(Route::Note),
    ///     _ => None,
    /// });
    /// ```
    pub fn links(self, parse: impl Fn(&str) -> Option<R> + 'static) -> Self
    where
        R: PartialEq,
    {
        let me = self;
        let take = move |url: &str| match parse(url) {
            Some(route) => {
                if me.current() != route {
                    me.push(route);
                }
                true
            }
            None => false,
        };
        // Links that came in before this router existed.
        let waiting = PENDING.with(|p| std::mem::take(&mut *p.borrow_mut()));
        for url in waiting {
            if !take(&url) {
                PENDING.with(|p| p.borrow_mut().push(url));
            }
        }
        LINKS.with(|l| l.borrow_mut().insert(self.0.id(), Box::new(take)));
        self
    }

    /// Build the screen for the current route. Also binds `alt+←` and `alt+→` (back and
    /// forward) while it is shown, so every app gets browser-style keyboard navigation.
    pub fn show<S: 'static>(self, screen: impl FnOnce(R) -> Node<Act<S>>) -> Node<Act<S>> {
        let page = screen(self.current());
        page.shortcut("alt+ArrowLeft", move |_: &mut S| self.back())
            .shortcut("alt+ArrowRight", move |_: &mut S| self.forward())
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
                    .act(move |_: &mut S| {
                        self.0.with(|s| {
                            s.past.truncate(i + 1);
                            s.changed();
                        })
                    })
                    .into()
            });
        }
        row(items).align_center().gap(6.0).into()
    }
}
