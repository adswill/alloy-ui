//! Animation in views, one call each. All of them key themselves from the
//! call site (like every other helper here), honour reduced motion, and cost
//! nothing once settled.
//!
//! ```ignore
//! reveal(s.show_help, text("Press ⌘K for the palette").dim())   // fade + slide + collapse
//! let w = animated(if s.wide { 320.0 } else { 160.0 });         // eased number
//! let k = spring(if hot { 1.1 } else { 1.0 });                  // no-overshoot spring
//! ```

use crate::{
    cx::{self, auto_id},
    ui,
    view::{Flex, Node},
};
use alloy_ui_widgets::{self as w, Animatable, Spring, Transition};
use std::hash::{Hash, Hasher};

/// Show `child` while `show` is true: it fades and drifts up 6 px on enter,
/// the reverse on exit, and the space it occupies grows and shrinks with it.
/// Hidden content is not drawn and gets no input.
#[track_caller]
pub fn reveal<M: 'static>(show: bool, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(60);
    w::Reveal::new(ui(), key, show, child.into().into_element())
        .slide(6.0)
        .collapse(true)
        .into_element()
        .into()
}

/// Like [`reveal`] but only the opacity changes; layout never moves.
#[track_caller]
pub fn fade<M: 'static>(show: bool, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(61);
    w::Reveal::new(ui(), key, show, child.into().into_element())
        .into_element()
        .into()
}

/// `target` eased over the theme's normal duration: call it every view with
/// the value you want and use what it returns. Works for `f32`, `Point` and
/// `Color`.
#[track_caller]
pub fn animated<T: Animatable>(target: T) -> T {
    let key = auto_id(62);
    let ms = ui().theme().motion.normal_ms;
    ui().animate(key, target, Transition::ms(ms))
}

/// `target` on a critically damped spring (no overshoot). Interrupting it
/// mid-flight keeps its velocity, so it never snaps.
#[track_caller]
pub fn spring<T: Animatable>(target: T) -> T {
    let key = auto_id(63);
    ui().animate(key, target, Transition::Spring(Spring::SNAPPY))
}

/// Like [`reveal`], but also plays the enter transition the first time it is
/// built while `show` is already true (cached or just-opened content eases in
/// instead of popping).
#[track_caller]
pub fn reveal_appear<M: 'static>(show: bool, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(64);
    w::Reveal::new(ui(), key, show, child.into().into_element())
        .slide(6.0)
        .collapse(true)
        .appear(true)
        .into_element()
        .into()
}

/// Like [`fade`], but also fades in the first time it is built while `show`
/// is true. Layout never moves.
#[track_caller]
pub fn fade_appear<M: 'static>(show: bool, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(65);
    w::Reveal::new(ui(), key, show, child.into().into_element())
        .appear(true)
        .into_element()
        .into()
}

/// Run `f` with every animation, hover, `local` and other call-site-keyed
/// state inside it also keyed by `id`. Wrap each row of a list that can
/// reorder or recycle (`keyed(item.id, || row(item))`) so its fade, reveal
/// and hover state follows the item instead of its position in the list.
pub fn keyed<R>(id: impl Hash, f: impl FnOnce() -> R) -> R {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    cx::keyed(h.finish(), f)
}

struct Entry<T> {
    key: u64,
    item: T,
    /// Time (ms) it left the list, if it has.
    gone: Option<u64>,
    /// Joined after the first build, so it eases in.
    fresh: bool,
}

/// A column of `items` where rows that disappear stay mounted and play their
/// exit (fade, slide, collapse) before they are removed, and rows that join
/// later ease in. `key` gives an item's identity, `row` builds it. The first
/// build shows everything at once. Under reduced motion removed rows vanish
/// immediately. Items must be `Clone + Send` because a leaving row is rebuilt
/// from its last value.
///
/// ```ignore
/// presence_list(&s.todos, |t| t.id, |t| text(&t.title))
/// ```
#[track_caller]
pub fn presence_list<M, T, K, N>(
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> K,
    row: impl Fn(&T) -> N,
) -> Flex<M>
where
    M: 'static,
    T: Clone + Send + 'static,
    K: Hash,
    N: Into<Node<M>>,
{
    let hash = |k: &K| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        k.hash(&mut h);
        h.finish()
    };
    let u = ui();
    let now = u.now();
    let linger = u.theme().motion.normal_ms as u64 + 60;
    let reduced = u.reduced_motion();
    // `None` until the first build: that build must not animate anything in.
    let mem = crate::local(None::<Vec<Entry<T>>>);
    let prev = mem.with(|m| m.take());
    let first = prev.is_none();
    let prev = prev.unwrap_or_default();

    let mut out: Vec<Entry<T>> = items
        .into_iter()
        .map(|item| {
            let k = hash(&key(&item));
            let old = prev.iter().find(|p| p.key == k);
            Entry {
                key: k,
                fresh: old.map_or(!first, |p| p.fresh),
                item,
                gone: None,
            }
        })
        .collect();
    // Ghosts: rows that were there and are not now, kept in their old place.
    for (pi, p) in prev.iter().enumerate() {
        if out.iter().any(|e| e.key == p.key) {
            continue;
        }
        let gone = p.gone.unwrap_or(now);
        if reduced || now.saturating_sub(gone) > linger {
            continue;
        }
        let at = prev[..pi]
            .iter()
            .rev()
            .find_map(|q| out.iter().position(|e| e.key == q.key))
            .map_or(0, |i| i + 1);
        out.insert(
            at,
            Entry {
                key: p.key,
                item: p.item.clone(),
                gone: Some(gone),
                fresh: p.fresh,
            },
        );
    }
    let els: Vec<_> = out
        .iter()
        .map(|e| {
            let show = e.gone.is_none();
            let fresh = e.fresh;
            cx::keyed(e.key, || {
                let n: Node<M> = row(&e.item).into();
                let k = auto_id(66);
                w::Reveal::new(ui(), k, show, n.into_element())
                    .slide(6.0)
                    .collapse(true)
                    .appear(fresh)
                    .into_element()
            })
        })
        .collect();
    mem.set(Some(out));
    Flex::new(false, els)
}

/// A registered bitmap (`Renderer::register_image` id) at a fixed size. See [`image`].
pub struct Pic {
    id: u64,
    w: f32,
    h: f32,
    alpha: f32,
}

/// The renderer image `id` at `w` by `h` logical pixels. `.opacity(a)` draws it
/// translucent; [`fade`], [`reveal`] and [`enter`](crate::enter) around it fade
/// the picture like any other content.
pub fn image(id: u64, w: f32, h: f32) -> Pic {
    Pic {
        id,
        w,
        h,
        alpha: 1.0,
    }
}

impl Pic {
    /// Draw at `alpha` opacity, 0 to 1.
    pub fn opacity(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }
}

impl<M: 'static> From<Pic> for Node<M> {
    fn from(p: Pic) -> Self {
        w::Image::new(p.id, alloy_ui_core::Size::new(p.w, p.h))
            .opacity(p.alpha)
            .into_element()
            .into()
    }
}
