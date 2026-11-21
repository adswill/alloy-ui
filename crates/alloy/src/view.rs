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
    border: Option<(Color, f32)>,
    radius: f32,
    w: Length,
    h: Length,
    align: Align,
}

impl<M: 'static> Boxed<M> {
    fn new(child: Element<M>) -> Self {
        Self {
            child,
            pad: Padding::ZERO,
            bg: None,
            border: None,
            radius: 0.0,
            w: Length::Shrink,
            h: Length::Shrink,
            align: Align::Start,
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
    /// The theme's panel colour as background.
    pub fn surface(self) -> Self {
        self.bg(cx::palette().surface)
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
        self.surface().outline().round(r).pad(16.0)
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
        if let Some(bg) = b.bg {
            c = c.background(bg);
        }
        if let Some((col, width)) = b.border {
            c = c.border(col, width);
        }
        Node(c.into_element())
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
