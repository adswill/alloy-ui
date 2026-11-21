//! Second-wave motion and surface effects, one call each. Like every other
//! helper they key themselves from the call site, honour reduced motion, and
//! cost nothing once settled (loading indicators loop only while shown).
//!
//! ```ignore
//! card.lift().ripple()                      // hover lift + press ripple
//! panel.frosted().grain(0.03)               // frosted glass with a hint of grain
//! glide(sidebar)                            // size and position changes animate
//! stagger(rows)                             // list entrance, one after another
//! route(s.page, page_view)                  // fade + slide between pages
//! skeleton(240.0, 14.0)                     // shimmering placeholder
//! counter(s.visitors).prefix("~")           // odometer number
//! spinner()                                 // working indicator
//! ```

use crate::{
    cx::{auto_id, ui},
    view::Node,
    Flex,
};
use alloy_ui_core::{style::Align, Point};
use alloy_ui_theme::Easing;
use alloy_ui_widgets::{self as w, Animatable, Keyframes, Timeline, Transition};
use std::hash::{Hash, Hasher};

pub use w::RouteKind;

fn hash_of(v: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn mix(key: u64, i: u64) -> u64 {
    key ^ (i + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Let `child`'s size and position changes animate: neighbours reflow
/// smoothly and the child glides to its new place instead of jumping.
#[track_caller]
pub fn glide<M: 'static>(child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(70);
    w::Glide::new(&ui(), key, child.into().into_element())
        .into_element()
        .into()
}

/// Like [`glide`] for size only; use it inside scrolling areas.
#[track_caller]
pub fn grow<M: 'static>(child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(71);
    w::Glide::new(&ui(), key, child.into().into_element())
        .size_only()
        .into_element()
        .into()
}

/// A shared element: give the same `id` to a widget on each of two pages (or
/// two layouts) and it travels from one place and size to the other.
#[track_caller]
pub fn shared<M: 'static>(id: impl Hash, child: impl Into<Node<M>>) -> Node<M> {
    let key = hash_of(&("shared", id));
    w::Glide::new(&ui(), key, child.into().into_element())
        .into_element()
        .into()
}

/// Fade and rise `child` in when it appears.
#[track_caller]
pub fn enter<M: 'static>(child: impl Into<Node<M>>) -> Node<M> {
    enter_after(0, child)
}

/// [`enter`] after a delay in milliseconds.
#[track_caller]
pub fn enter_after<M: 'static>(delay_ms: u64, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(72);
    w::Enter::new(&ui(), key, delay_ms, child.into().into_element())
        .into_element()
        .into()
}

/// A column whose children enter one after another, 45 ms apart (the first
/// eight; the rest follow together). Replays whenever it reappears.
#[track_caller]
pub fn stagger<M: 'static, V: Into<Node<M>>>(items: impl IntoIterator<Item = V>) -> Flex<M> {
    let base = auto_id(73);
    let u = ui();
    let els = items
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            w::Enter::new(
                &u,
                mix(base, i as u64),
                (i.min(8) as u64) * 45,
                v.into().into_element(),
            )
            .into_element()
        })
        .collect();
    Flex::new(false, els)
}

/// Show `child` as page number `page`; when `page` changes the old page
/// leaves and the new one arrives with a short fade and slide in the
/// direction of travel (larger numbers move forward).
#[track_caller]
pub fn route<M: 'static>(page: usize, child: impl Into<Node<M>>) -> Node<M> {
    route_with(page, RouteKind::SlideFade, child)
}

/// [`route`] with a chosen [`RouteKind`]: `Fade`, `SlideFade`, `Slide` or `Scale`.
#[track_caller]
pub fn route_with<M: 'static>(page: usize, kind: RouteKind, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(74);
    w::Route::new(&ui(), key, page as u64, child.into().into_element())
        .kind(kind)
        .into_element()
        .into()
}

/// A shared element for [`route`] and `Router::show_with` transitions: give the same `id` to a
/// widget on two pages and, when the page changes, it flies from where it was to where it now is,
/// scaling and cross-fading between its two looks. The pages fade and slide around it. Outside a
/// route it is just its child.
///
/// A hero is drawn above its page, so keep it to elements that sit on top anyway (cards, avatars,
/// cover images, titles).
pub fn hero<M: 'static>(id: impl Hash, child: impl Into<Node<M>>) -> Node<M> {
    let key = hash_of(&("hero", id));
    w::Hero::new(&ui(), key, child.into().into_element())
        .into_element()
        .into()
}

/// A bottom sheet over the area it is placed in (use inside [`stack`]): drag or flick the handle
/// and it follows the pointer, resists past the ends, and settles on one of `snaps` (visible
/// heights as fractions of the area, e.g. `[0.12, 0.5, 0.9]`) with the speed of the release.
#[track_caller]
pub fn sheet<M: 'static>(snaps: &[f32], content: impl Into<Node<M>>) -> Node<M> {
    let id = auto_id(75);
    w::Sheet::new(&ui(), id, snaps, content.into().into_element())
        .scrim()
        .into_element()
        .into()
}

/// [`route`] with an explicit page number (larger moves forward).
#[track_caller]
pub(crate) fn route_page<M: 'static>(page: u64, kind: RouteKind, child: Node<M>) -> Node<M> {
    let key = auto_id(74);
    w::Route::new(&ui(), key, page, child.into_element())
        .kind(kind)
        .into_element()
        .into()
}

/// Single-line text whose weight animates (400 to 700) when `weight` changes between rebuilds:
/// a hovered tab label that firms up, a selected item that becomes bold.
///
/// Alloy Sans has four static weights, so the in-between weights are drawn by fading the
/// heavier face over the lighter one; see `WeightText` for why.
#[track_caller]
pub fn weighted<M: 'static>(content: impl Into<String>, weight: u16) -> Node<M> {
    let key = auto_id(76);
    w::WeightText::new(&ui(), key, content, weight)
        .into_element()
        .into()
}

/// A shimmering placeholder block `width` x `height`.
pub fn skeleton<M: 'static>(width: f32, height: f32) -> Node<M> {
    w::Skeleton::new(&ui(), height)
        .width(crate::view::Length::Fixed(width))
        .into_element()
        .into()
}

/// A shimmering placeholder that fills the available width.
pub fn skeleton_bar<M: 'static>(height: f32) -> Node<M> {
    w::Skeleton::new(&ui(), height).into_element().into()
}

/// A shimmering circle, for avatars.
pub fn skeleton_circle<M: 'static>(diameter: f32) -> Node<M> {
    w::Skeleton::new(&ui(), diameter)
        .width(crate::view::Length::Fixed(diameter))
        .radius(diameter / 2.0)
        .into_element()
        .into()
}

/// `lines` placeholder text lines in a column; the last is shorter.
pub fn skeleton_text<M: 'static>(lines: usize) -> Flex<M> {
    let u = ui();
    let items = (0..lines.max(1))
        .map(|i| {
            let last = i + 1 == lines.max(1) && lines > 1;
            let mut s = w::Skeleton::new(&u, 11.0).radius(4.0);
            if last {
                s = s.width(crate::view::Length::Fixed(140.0));
            }
            s.into_element()
        })
        .collect();
    Flex::new(false, items)
}

/// A turning ring for "working on it".
pub fn spinner<M: 'static>() -> Node<M> {
    w::Spinner::new(&ui(), w::SpinnerKind::Ring)
        .into_element()
        .into()
}

/// A ring of a given diameter.
pub fn spinner_sized<M: 'static>(size: f32) -> Node<M> {
    w::Spinner::new(&ui(), w::SpinnerKind::Ring)
        .size(size)
        .into_element()
        .into()
}

/// Three dots handing a light along, for chat typing and short waits.
pub fn dots<M: 'static>() -> Node<M> {
    w::Spinner::new(&ui(), w::SpinnerKind::Dots)
        .into_element()
        .into()
}

/// An indeterminate bar of the given length, a segment sliding across a track.
pub fn loading_bar<M: 'static>(length: f32) -> Node<M> {
    w::Spinner::new(&ui(), w::SpinnerKind::Bar)
        .size(length)
        .into_element()
        .into()
}

/// A determinate ring showing `fraction` (0 to 1) with its percentage; the
/// arc eases to each new value.
#[track_caller]
pub fn progress_ring<M: 'static>(fraction: f32) -> Node<M> {
    let key = auto_id(75);
    w::ProgressRing::new(&ui(), key, fraction)
        .into_element()
        .into()
}

/// A number whose digits roll like an odometer to each new value.
#[track_caller]
pub fn counter(value: impl Into<f64>) -> Rolling {
    Rolling {
        key: auto_id(76),
        value: value.into(),
        decimals: 0,
        group: true,
        prefix: String::new(),
        suffix: String::new(),
        size: None,
        weight: 600,
    }
}

/// Builder returned by [`counter`]; converts into a view.
pub struct Rolling {
    key: u64,
    value: f64,
    decimals: u32,
    group: bool,
    prefix: String,
    suffix: String,
    size: Option<f32>,
    weight: u16,
}

impl Rolling {
    /// Digits after the decimal point.
    pub fn decimals(mut self, d: u32) -> Self {
        self.decimals = d;
        self
    }
    /// Text before the number.
    pub fn prefix(mut self, s: impl Into<String>) -> Self {
        self.prefix = s.into();
        self
    }
    /// Text after the number.
    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.suffix = s.into();
        self
    }
    /// Turn thousands separators on or off.
    pub fn grouped(mut self, on: bool) -> Self {
        self.group = on;
        self
    }
    /// Font size in logical pixels.
    pub fn size(mut self, s: f32) -> Self {
        self.size = Some(s);
        self
    }
    /// Heading-sized.
    pub fn large(self) -> Self {
        let s = crate::theme().typography.heading.size;
        self.size(s)
    }
    /// Font weight.
    pub fn weight(mut self, w: u16) -> Self {
        self.weight = w;
        self
    }
}

impl<M: 'static> From<Rolling> for Node<M> {
    fn from(r: Rolling) -> Self {
        let mut c = w::Counter::new(&ui(), r.key, r.value)
            .decimals(r.decimals)
            .grouped(r.group)
            .prefix(r.prefix)
            .suffix(r.suffix)
            .weight(r.weight);
        if let Some(s) = r.size {
            c = c.size(s);
        }
        c.into_element().into()
    }
}

/// A choreographed sequence: starts when first seen and again whenever
/// `trigger` changes. Ask it for each part's progress:
///
/// ```ignore
/// let t = timeline(s.step, 900);
/// let card = t.eased(0, 400, Easing::QUART_OUT);
/// let badge = t.eased(250, 700, Easing::QUART_OUT);
/// ```
#[track_caller]
pub fn timeline(trigger: impl Hash, total_ms: u32) -> Timeline {
    let key = auto_id(77);
    ui().timeline(key, hash_of(&trigger), total_ms)
}

/// A value passing through `frames` over `ms`, replayed whenever `trigger` changes.
#[track_caller]
pub fn keyframes<T: Animatable>(trigger: impl Hash, ms: u32, frames: &Keyframes<T>) -> T {
    let key = auto_id(78);
    ui().keyframes(key, hash_of(&trigger), ms, frames)
}

/// [`animated`](crate::animated) with your own duration and easing curve.
#[track_caller]
pub fn animated_with<T: Animatable>(target: T, ms: u32, easing: Easing) -> T {
    let key = auto_id(79);
    ui().animate(key, target, Transition::eased(ms, easing))
}

/// Where a layer sits in a [`stack`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    /// Top-left corner (the default).
    #[default]
    TopLeft,
    /// Top edge, centred.
    Top,
    /// Top-right corner.
    TopRight,
    /// Left edge, centred.
    Left,
    /// Dead centre.
    Center,
    /// Right edge, centred.
    Right,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom edge, centred.
    Bottom,
    /// Bottom-right corner.
    BottomRight,
}

impl Anchor {
    fn parts(self) -> (Align, Align) {
        use Align::{Center as C, End as E, Start as S};
        match self {
            Anchor::TopLeft => (S, S),
            Anchor::Top => (C, S),
            Anchor::TopRight => (E, S),
            Anchor::Left => (S, C),
            Anchor::Center => (C, C),
            Anchor::Right => (E, C),
            Anchor::BottomLeft => (S, E),
            Anchor::Bottom => (C, E),
            Anchor::BottomRight => (E, E),
        }
    }
}

/// One layer of a [`stack`]: a view, where it sits and how far it is nudged.
pub struct Layer<M> {
    node: Node<M>,
    place: w::Place,
}

impl<M: 'static> Layer<M> {
    /// Pin to a corner, edge or the centre.
    pub fn anchor(mut self, a: Anchor) -> Self {
        let (x, y) = a.parts();
        self.place.x = x;
        self.place.y = y;
        self
    }
    /// Nudge by `(dx, dy)` logical pixels after anchoring.
    pub fn offset(mut self, dx: f32, dy: f32) -> Self {
        self.place.offset = Point::new(dx, dy);
        self
    }
}

impl<M: 'static, T: Into<Node<M>>> From<T> for Layer<M> {
    fn from(v: T) -> Self {
        Layer {
            node: v.into(),
            place: w::Place::default(),
        }
    }
}

/// Layer views on top of each other, first at the back. The stack is as big
/// as its largest layer; give the base `.fill()` or a size, and pin the rest
/// with `Layer::anchor`. This is what gives `.frosted()` something to blur.
///
/// ```ignore
/// stack([
///     Layer::from(photo),
///     Layer::from(caption.frosted().pad(12.0)).anchor(Anchor::Bottom).offset(0.0, -16.0),
/// ])
/// ```
pub fn stack<M: 'static>(layers: impl IntoIterator<Item = Layer<M>>) -> Node<M> {
    let layers: Vec<_> = layers
        .into_iter()
        .map(|l| (l.node.into_element(), l.place))
        .collect();
    // A layer that fills (a sheet, a full-size page) makes the stack fill too.
    let fills = |f: fn(&alloy_ui_core::style::Style) -> crate::view::Length| {
        if layers
            .iter()
            .any(|(e, _)| matches!(f(&e.0.style()), crate::view::Length::Fill(_)))
        {
            crate::view::Length::Fill(1)
        } else {
            crate::view::Length::Shrink
        }
    };
    let (wd, ht) = (fills(|s| s.width), fills(|s| s.height));
    w::Stack::new(layers)
        .width(wd)
        .height(ht)
        .into_element()
        .into()
}
