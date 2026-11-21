//! [`Node`], the one type every helper converts into, and the [`View`] trait, plus layout builders.

use crate::cx;
pub use alloy_ui_core::style::{Length, Padding};
use alloy_ui_core::{style::Align, Color, Element};
use alloy_ui_widgets as w;

/// A concrete view tree node. Every builder converts into a `Node`, and so do `&str`,
/// `String`, `Option<impl Into<View>>` (`None` is empty) and raw [`Element`]s.
pub struct Node<M>(pub(crate) Element<M>);

impl<M> Node<M> {
    /// The underlying element.
    pub fn into_element(self) -> Element<M> {
        self.0
    }
}
impl<M> From<Element<M>> for Node<M> {
    fn from(e: Element<M>) -> Self {
        Node(e)
    }
}
impl<M> From<Node<M>> for Element<M> {
    fn from(v: Node<M>) -> Self {
        v.0
    }
}
impl<M: 'static> From<&str> for Node<M> {
    fn from(s: &str) -> Self {
        crate::text(s).into()
    }
}
impl<M: 'static> From<String> for Node<M> {
    fn from(s: String) -> Self {
        crate::text(s).into()
    }
}
impl<M: 'static, T: Into<Node<M>>> From<Option<T>> for Node<M> {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => v.into(),
            None => Node(w::Space::fixed(0.0, 0.0).into_element()),
        }
    }
}

/// A box around one view: padding, background, border, size. Chained calls on
/// the same view share one box instead of nesting.
pub struct Boxed<M> {
    child: Element<M>,
    pad: Padding,
    bg: Option<Color>,
    hover_bg: Option<Color>,
    grad: Option<(Color, Color, f32)>,
    shadow: Option<alloy_ui_core::Shadow>,
    border: Option<(Color, f32)>,
    radius: f32,
    w: Length,
    h: Length,
    align: Align,
    fx: Vec<w::Layer>,
    fx_key: u64,
    glide: bool,
    flow: Option<f32>,
}

impl<M: 'static> Boxed<M> {
    fn new(child: Element<M>) -> Self {
        Self {
            child,
            pad: Padding::ZERO,
            bg: None,
            hover_bg: None,
            grad: None,
            shadow: None,
            border: None,
            radius: 0.0,
            w: Length::Shrink,
            h: Length::Shrink,
            align: Align::Start,
            fx: Vec::new(),
            fx_key: 0,
            glide: false,
            flow: None,
        }
    }
    /// Padding on all sides.
    pub fn pad(mut self, p: f32) -> Self {
        self.pad = Padding::all(p);
        self
    }
    /// Horizontal and vertical padding.
    pub fn pad_xy(mut self, x: f32, y: f32) -> Self {
        self.pad = Padding::symmetric(x, y);
        self
    }
    /// Padding of `p` on all sides plus the [`safe_area`](crate::safe_area),
    /// so content clears the notch and system bars while a background set
    /// with `bg` still runs under them.
    pub fn pad_safe(mut self, p: f32) -> Self {
        let i = crate::cx::safe_area();
        self.pad = Padding {
            top: p + i.top,
            right: p + i.right,
            bottom: p + i.bottom,
            left: p + i.left,
        };
        self
    }
    /// Fixed width.
    pub fn w(mut self, w: f32) -> Self {
        self.w = Length::Fixed(w);
        self
    }
    /// Fixed height.
    pub fn h(mut self, h: f32) -> Self {
        self.h = Length::Fixed(h);
        self
    }
    /// Fill the available width.
    pub fn fill_w(mut self) -> Self {
        self.w = Length::Fill(1);
        self
    }
    /// Fill the available height.
    pub fn fill_h(mut self) -> Self {
        self.h = Length::Fill(1);
        self
    }
    /// Fill the available space in both directions.
    pub fn fill(self) -> Self {
        self.fill_w().fill_h()
    }
    /// Background colour.
    pub fn bg(mut self, c: Color) -> Self {
        self.bg = Some(c);
        self
    }
    /// Background colour while the pointer is over the box, eased in and out. On a box with no
    /// `bg` the colour fades in over what is behind it: `row![..].pad(10.0).hover_bg(tint)`.
    pub fn hover_bg(mut self, c: Color) -> Self {
        self.hover_bg = Some(c);
        self
    }
    /// A linear gradient background; `angle_deg` 0 runs top to bottom, 90
    /// left to right. Keep the stops close (a tonal shift, not a rainbow).
    pub fn gradient(mut self, from: Color, to: Color, angle_deg: f32) -> Self {
        self.grad = Some((from, to, angle_deg));
        self
    }
    /// Lift off the page: the theme's medium elevation shadow.
    pub fn elevated(mut self) -> Self {
        self.shadow = Some(cx::theme().elevation.medium);
        self
    }
    /// The theme's panel colour as background. While the window has a [`WindowEffect`]
    /// (Mica, Blur, ...) the colour is as see-through as the page, so the material shows
    /// behind sidebars and headers; use [`card`](Self::card) or `bg` for a region that must
    /// stay solid.
    pub fn surface(self) -> Self {
        let a = cx::page_alpha();
        let c = cx::palette().surface;
        self.bg(if a < 1.0 { c.with_alpha(c.a * a) } else { c })
    }
    /// Corner radius.
    pub fn round(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    /// A 1 px border in the theme's divider colour.
    pub fn outline(mut self) -> Self {
        self.border = Some((cx::palette().border, 1.0));
        self
    }
    /// A raised panel: surface, outline, rounded, padded.
    pub fn card(self) -> Self {
        let r = cx::theme().radii.large;
        self.bg(cx::palette().surface).outline().round(r).pad(16.0)
    }
    #[track_caller]
    fn layer(mut self, l: w::Layer) -> Self {
        if self.fx_key == 0 {
            self.fx_key = cx::auto_id(80);
        }
        self.fx.push(l);
        self
    }
    /// Rise 2 px and deepen the shadow while hovered.
    #[track_caller]
    pub fn lift(self) -> Self {
        self.layer(w::Layer::Lift)
    }
    /// Hover depth: a 1.2% swell, a shadow leaning away from the pointer and a
    /// faint highlight following it.
    #[track_caller]
    pub fn tilt(self) -> Self {
        self.layer(w::Layer::Tilt)
    }
    /// A soft ripple spreading from each press.
    #[track_caller]
    pub fn ripple(self) -> Self {
        self.layer(w::Layer::Ripple(None))
    }
    /// A ripple in your own colour.
    #[track_caller]
    pub fn ripple_color(self, c: Color) -> Self {
        self.layer(w::Layer::Ripple(Some(c)))
    }
    /// While pressed, the accent colour rims the inside edge.
    #[track_caller]
    pub fn glow(self) -> Self {
        self.layer(w::Layer::Glow)
    }
    /// A recessed well: a soft shadow inside the top edge.
    #[track_caller]
    pub fn inset(self) -> Self {
        self.layer(w::Layer::Inset)
    }
    /// Fine film grain; `amount` 0.02 to 0.05 is the usable range.
    #[track_caller]
    pub fn grain(self, amount: f32) -> Self {
        self.layer(w::Layer::Grain(amount))
    }
    /// Frosted glass over whatever is behind (18 px blur, theme tint, hairline
    /// edge). Replaces the background; call it after `.card()` or `.bg()`.
    #[track_caller]
    pub fn frosted(self) -> Self {
        self.layer(w::Layer::Frosted(18.0, None))
    }
    /// Frosted glass with your own blur radius in logical pixels.
    #[track_caller]
    pub fn frosted_blur(self, blur: f32) -> Self {
        self.layer(w::Layer::Frosted(blur, None))
    }
    /// Let this box's size and position changes animate.
    #[track_caller]
    pub fn glide(mut self) -> Self {
        if self.fx_key == 0 {
            self.fx_key = cx::auto_id(80);
        }
        self.glide = true;
        self
    }
    /// Turn the gradient slowly, one full turn every `seconds`. Ambient only:
    /// it keeps the window redrawing while visible (and holds still under
    /// reduced motion).
    #[track_caller]
    pub fn flow(mut self, seconds: f32) -> Self {
        if self.fx_key == 0 {
            self.fx_key = cx::auto_id(80);
        }
        self.flow = Some(seconds.max(1.0));
        self
    }
    /// Centre the child horizontally inside the box.
    pub fn center(mut self) -> Self {
        self.align = Align::Center;
        self
    }
}

impl<M: 'static> From<Boxed<M>> for Node<M> {
    fn from(b: Boxed<M>) -> Self {
        let mut c = w::Container::new(b.child)
            .padding(b.pad)
            .radius(b.radius)
            .align_x(b.align)
            .width(b.w)
            .height(b.h);
        if let (Some(bg), false) = (
            b.bg,
            b.fx.iter().any(|l| matches!(l, w::Layer::Frosted(..))),
        ) {
            c = c.background(bg);
        }
        if let Some(h) = b.hover_bg {
            c = c.hover_background(&cx::ui(), h);
        }
        let mut fx = b.fx;
        if let Some((from, to, a)) = b.grad {
            let mut a = a;
            if let Some(secs) = b.flow {
                let u = cx::ui();
                if !u.reduced_motion() {
                    a += 360.0 * ((u.now() as f32 / 1000.0 / secs) % 1.0);
                    fx.push(w::Layer::Keepalive);
                }
            }
            c = c.gradient(from, to, a);
        }
        if let Some(sh) = b.shadow {
            c = c.shadow(sh);
        }
        if let Some((col, width)) = b.border {
            c = c.border(col, width);
        }
        let mut el = c.into_element();
        if b.glide {
            el = w::Glide::new(&cx::ui(), b.fx_key ^ 0x6C, el).into_element();
        }
        if !fx.is_empty() {
            el = w::Fx::new(&cx::ui(), b.fx_key, b.radius, el)
                .layers(fx)
                .into_element();
        }
        Node(el)
    }
}

/// Anything that can be shown: every builder, `&str`, `String` and `Option` of those.
/// Also provides sizing, padding and decoration. Return `impl View<Msg>` from `view`.
pub trait View<M: 'static>: Into<Node<M>> + Sized {
    /// Wrap in a [`Boxed`] to style.
    fn boxed(self) -> Boxed<M> {
        Boxed::new(self.into().0)
    }
    /// Padding on all sides.
    fn pad(self, p: f32) -> Boxed<M> {
        self.boxed().pad(p)
    }
    /// Horizontal and vertical padding.
    fn pad_xy(self, x: f32, y: f32) -> Boxed<M> {
        self.boxed().pad_xy(x, y)
    }
    /// Padding on all sides plus the [`safe_area`](crate::safe_area).
    fn pad_safe(self, p: f32) -> Boxed<M> {
        self.boxed().pad_safe(p)
    }
    /// Fixed width.
    fn w(self, w: f32) -> Boxed<M> {
        self.boxed().w(w)
    }
    /// Fixed height.
    fn h(self, h: f32) -> Boxed<M> {
        self.boxed().h(h)
    }
    /// Fill the available width.
    fn fill_w(self) -> Boxed<M> {
        self.boxed().fill_w()
    }
    /// Fill the available height.
    fn fill_h(self) -> Boxed<M> {
        self.boxed().fill_h()
    }
    /// Fill the available space.
    fn fill(self) -> Boxed<M> {
        self.boxed().fill()
    }
    /// Background colour.
    fn bg(self, c: Color) -> Boxed<M> {
        self.boxed().bg(c)
    }
    /// Hover background (see [`Boxed::hover_bg`]).
    fn hover_bg(self, c: Color) -> Boxed<M> {
        self.boxed().hover_bg(c)
    }
    /// Gradient background (see [`Boxed::gradient`]).
    fn gradient(self, from: Color, to: Color, angle_deg: f32) -> Boxed<M> {
        self.boxed().gradient(from, to, angle_deg)
    }
    /// Hover lift (see [`Boxed::lift`]).
    #[track_caller]
    fn lift(self) -> Boxed<M> {
        self.boxed().lift()
    }
    /// Hover tilt (see [`Boxed::tilt`]).
    #[track_caller]
    fn tilt(self) -> Boxed<M> {
        self.boxed().tilt()
    }
    /// Press ripple (see [`Boxed::ripple`]).
    #[track_caller]
    fn ripple(self) -> Boxed<M> {
        self.boxed().ripple()
    }
    /// Press glow (see [`Boxed::glow`]).
    #[track_caller]
    fn glow(self) -> Boxed<M> {
        self.boxed().glow()
    }
    /// Recessed well (see [`Boxed::inset`]).
    #[track_caller]
    fn inset(self) -> Boxed<M> {
        self.boxed().inset()
    }
    /// Film grain (see [`Boxed::grain`]).
    #[track_caller]
    fn grain(self, amount: f32) -> Boxed<M> {
        self.boxed().grain(amount)
    }
    /// Frosted glass (see [`Boxed::frosted`]).
    #[track_caller]
    fn frosted(self) -> Boxed<M> {
        self.boxed().frosted()
    }
    /// Animate size and position changes (see [`Boxed::glide`]).
    #[track_caller]
    fn glide(self) -> Boxed<M> {
        self.boxed().glide()
    }
    /// Medium elevation shadow.
    fn elevated(self) -> Boxed<M> {
        self.boxed().elevated()
    }
    /// Panel background.
    fn surface(self) -> Boxed<M> {
        self.boxed().surface()
    }
    /// Corner radius.
    fn round(self, r: f32) -> Boxed<M> {
        self.boxed().round(r)
    }
    /// Raised, outlined, rounded, padded panel.
    fn card(self) -> Boxed<M> {
        self.boxed().card()
    }
    /// Centre inside the parent's width.
    fn center(self) -> Boxed<M> {
        self.boxed().fill_w().center()
    }
    /// Show only when `yes`; nothing is drawn (and no space taken) otherwise.
    fn when(self, yes: bool) -> Node<M> {
        if yes {
            self.into()
        } else {
            Node(w::Space::fixed(0.0, 0.0).into_element())
        }
    }
    /// Put `label` in front, in a column as wide as other labelled controls so they line up.
    fn labelled(self, label: &str) -> Node<M> {
        let label: Node<M> = crate::text(label).w(110.0).into();
        crate::row([label, self.into()]).align_center().into()
    }
    /// Let the user pick this up with the pointer and drop it on a [`drop_on`](Self::drop_on)
    /// target that takes the same type: `card(c).drag(c.id)`. A translucent copy follows the
    /// pointer; Escape cancels. Presses on buttons inside still click.
    #[track_caller]
    fn drag<T: 'static>(self, payload: T) -> Node<M> {
        let id = cx::auto_id(70);
        Node(w::Draggable::new(&cx::ui(), id, payload, self.into().0).into_element())
    }
    /// Take a value dropped here: `column.drop_on(|id: u64| Msg::MoveTo(Col::Done, id))`. Lights
    /// up while a matching [`drag`](Self::drag) is in flight; other types are ignored.
    fn drop_on<T: Clone + 'static>(self, f: impl Fn(T) -> M + 'static) -> Node<M> {
        Node(w::DropZone::new(&cx::ui(), self.into().0, f).into_element())
    }
    /// Like [`drop_on`](Self::drop_on) for apps with closure messages: `.drop_act(|s, id: u64| s.move_to(id))`.
    fn drop_act<T: Clone + Send + Sync + 'static, S: 'static>(
        self,
        f: impl Fn(&mut S, T) + Send + Sync + 'static,
    ) -> Node<crate::dsl::Act<S>>
    where
        Self: Into<Node<crate::dsl::Act<S>>>,
    {
        let f = std::sync::Arc::new(f);
        let node: Node<crate::dsl::Act<S>> = self.into();
        Node(
            w::DropZone::new(&cx::ui(), node.0, move |t: T| {
                let f = f.clone();
                crate::dsl::Act::new(move |s: &mut S| f(s, t.clone()))
            })
            .into_element(),
        )
    }
    /// Show `text` in a tooltip on hover.
    fn tip(self, text: impl Into<String>) -> Node<M> {
        Node(w::Tooltip::new(&cx::ui(), self.into().0, text).into_element())
    }
}
impl<M: 'static, T: Into<Node<M>>> View<M> for T {}

#[doc(hidden)]
pub fn node<M: 'static, V: Into<Node<M>>>(v: V) -> Node<M> {
    v.into()
}

/// A row or column under construction.
pub struct Flex<M> {
    horizontal: bool,
    items: Vec<Element<M>>,
    gap: f32,
    pad: Padding,
    align: Align,
    w: Length,
    h: Length,
}

impl<M: 'static> Flex<M> {
    #[doc(hidden)]
    pub fn new(horizontal: bool, items: Vec<Element<M>>) -> Self {
        Self {
            horizontal,
            items,
            gap: cx::theme().spacing.sm,
            pad: Padding::ZERO,
            align: Align::Start,
            w: Length::Shrink,
            h: Length::Shrink,
        }
    }
    /// Space between children (default 8).
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    /// Padding around the children.
    pub fn pad(mut self, p: f32) -> Self {
        self.pad = Padding::all(p);
        self
    }
    /// Horizontal and vertical padding.
    pub fn pad_xy(mut self, x: f32, y: f32) -> Self {
        self.pad = Padding::symmetric(x, y);
        self
    }
    /// Centre children on the cross axis.
    pub fn align_center(mut self) -> Self {
        self.align = Align::Center;
        self
    }
    /// Push children to the cross-axis end.
    pub fn align_end(mut self) -> Self {
        self.align = Align::End;
        self
    }
    /// Fixed width.
    pub fn w(mut self, w: f32) -> Self {
        self.w = Length::Fixed(w);
        self
    }
    /// Fixed height.
    pub fn h(mut self, h: f32) -> Self {
        self.h = Length::Fixed(h);
        self
    }
    /// Fill the available width.
    pub fn fill_w(mut self) -> Self {
        self.w = Length::Fill(1);
        self
    }
    /// Fill the available height.
    pub fn fill_h(mut self) -> Self {
        self.h = Length::Fill(1);
        self
    }
    /// Fill the available space.
    pub fn fill(self) -> Self {
        self.fill_w().fill_h()
    }
    /// The root of a screen: padding and spacing from the theme's page scale.
    pub fn page(mut self) -> Self {
        let sp = cx::theme().spacing;
        self.pad = Padding::all(sp.xxl);
        self.gap = sp.lg;
        self
    }
    /// Append a child.
    pub fn push(mut self, v: impl Into<Node<M>>) -> Self {
        self.items.push(v.into().0);
        self
    }
}

impl<M: 'static> From<Flex<M>> for Node<M> {
    fn from(f: Flex<M>) -> Self {
        let (p, g, mut a, w, h) = (f.pad, f.gap, f.align, f.w, f.h);
        let mut items = f.items;
        // Right-to-left: rows mirror (first child on the right) and a column's "start" edge is
        // the right one, so layouts written start-to-end flip with the locale.
        if crate::i18n::is_rtl() {
            if f.horizontal {
                items.reverse();
            } else {
                a = match a {
                    Align::Start => Align::End,
                    Align::End => Align::Start,
                    other => other,
                };
            }
        }
        Node(if f.horizontal {
            w::Row::new(items)
                .spacing(g)
                .padding(p)
                .align(a)
                .width(w)
                .height(h)
                .into_element()
        } else {
            w::Column::new(items)
                .spacing(g)
                .padding(p)
                .align(a)
                .width(w)
                .height(h)
                .into_element()
        })
    }
}

/// A vertical stack from any iterator of views: `col(items.iter().map(..))`.
pub fn col<M: 'static, V: Into<Node<M>>>(items: impl IntoIterator<Item = V>) -> Flex<M> {
    Flex::new(false, items.into_iter().map(|v| v.into().0).collect())
}

/// A horizontal stack from any iterator of views.
pub fn row<M: 'static, V: Into<Node<M>>>(items: impl IntoIterator<Item = V>) -> Flex<M> {
    Flex::new(true, items.into_iter().map(|v| v.into().0).collect())
}

/// A vertical stack: `col![heading("Hi"), "plain text", button("Go")]`.
#[macro_export]
macro_rules! col {
    ($($v:expr),* $(,)?) => {
        $crate::view::Flex::new(false, ::std::vec![$($crate::view::node($v).into_element()),*])
    };
}

/// A horizontal stack: `row![button("-"), "3", button("+")]`.
#[macro_export]
macro_rules! row {
    ($($v:expr),* $(,)?) => {
        $crate::view::Flex::new(true, ::std::vec![$($crate::view::node($v).into_element()),*])
    };
}
