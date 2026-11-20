//! Widget helpers: one short function per widget, builder methods for options.

use crate::{
    cx::{self, auto_id},
    view::Node,
};
use alloy_ui_core::{style::Length, Color, Element};
use alloy_ui_widgets as w;
use std::sync::Arc;

pub use w::Variant;

/// Closure-style message: a function from the state to nothing. Apps written
/// with [`sketch`](crate::sketch) use this as their message type, so there is
/// no `Msg` enum to write.
pub struct Act<S>(pub(crate) Arc<dyn Fn(&mut S) + Send + Sync>);

impl<S> Clone for Act<S> {
    fn clone(&self) -> Self {
        Act(self.0.clone())
    }
}

impl<S> Act<S> {
    /// Wrap a state mutation.
    pub fn new(f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        Act(Arc::new(f))
    }
}

fn len(px: f32) -> Length {
    Length::Fixed(px)
}

// ---------------------------------------------------------------- text

/// Text under construction. See [`text`].
pub struct Txt {
    s: String,
    role: w::TextRole,
    dim: bool,
    color: Option<Color>,
    wrap: bool,
    width: Length,
}

/// Body text. Roles: [`.heading()`](Txt::heading), [`.caption()`](Txt::caption), ...
pub fn text(s: impl Into<String>) -> Txt {
    Txt {
        s: s.into(),
        role: w::TextRole::Body,
        dim: false,
        color: None,
        wrap: true,
        width: Length::Shrink,
    }
}
/// A section heading.
pub fn heading(s: impl Into<String>) -> Txt {
    text(s).heading()
}
/// Small secondary text.
pub fn caption(s: impl Into<String>) -> Txt {
    text(s).caption()
}

impl Txt {
    /// Page title size.
    pub fn display(mut self) -> Self {
        self.role = w::TextRole::Display;
        self
    }
    /// Heading size.
    pub fn heading(mut self) -> Self {
        self.role = w::TextRole::Heading;
        self
    }
    /// Sub-heading size.
    pub fn sub(mut self) -> Self {
        self.role = w::TextRole::Subheading;
        self
    }
    /// Control-label style.
    pub fn label(mut self) -> Self {
        self.role = w::TextRole::Label;
        self
    }
    /// Small caption size.
    pub fn caption(mut self) -> Self {
        self.role = w::TextRole::Caption;
        self
    }
    /// Secondary (dimmed) colour.
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }
    /// Explicit colour.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// Danger colour.
    pub fn danger(self) -> Self {
        let c = cx::palette().danger;
        self.color(c)
    }
    /// Success colour.
    pub fn success(self) -> Self {
        let c = cx::palette().success;
        self.color(c)
    }
    /// Danger colour when `yes`.
    pub fn danger_if(self, yes: bool) -> Self {
        if yes {
            self.danger()
        } else {
            self
        }
    }
    /// Accent colour.
    pub fn accent(self) -> Self {
        let c = cx::palette().accent;
        self.color(c)
    }
    /// Never wrap onto a second line.
    pub fn no_wrap(mut self) -> Self {
        self.wrap = false;
        self
    }
    /// Fixed width.
    pub fn w(mut self, px: f32) -> Self {
        self.width = len(px);
        self
    }
    /// Take the remaining width (pushes later siblings to the end of a row).
    pub fn fill_w(mut self) -> Self {
        self.width = Length::Fill(1);
        self
    }
    fn build<M: 'static>(self) -> Node<M> {
        let mut t = w::Text::new(&cx::ui(), self.s)
            .role(self.role)
            .width(self.width);
        if self.dim {
            t = t.secondary();
        }
        if let Some(c) = self.color {
            t = t.color(c);
        }
        if !self.wrap {
            t = t.no_wrap();
        }
        Node(t.into_element())
    }
}
impl<M: 'static> From<Txt> for Node<M> {
    fn from(t: Txt) -> Self {
        t.build()
    }
}

// -------------------------------------------------------------- button

/// A button under construction. See [`button`].
pub struct Btn<M> {
    label: String,
    on: Option<M>,
    variant: Variant,
    wide: bool,
    id: u64,
}

/// A button. Without `.on(..)` it is disabled.
#[track_caller]
pub fn button<M>(label: impl Into<String>) -> Btn<M> {
    Btn {
        label: label.into(),
        on: None,
        variant: Variant::Secondary,
        wide: false,
        id: auto_id(1),
    }
}

impl<M> Btn<M> {
    /// Message sent when pressed.
    pub fn on(mut self, m: M) -> Self {
        self.on = Some(m);
        self
    }
    /// The one main action of a view.
    pub fn primary(mut self) -> Self {
        self.variant = Variant::Primary;
        self
    }
    /// Low emphasis: no fill until hovered.
    pub fn ghost(mut self) -> Self {
        self.variant = Variant::Ghost;
        self
    }
    /// Stretch to the available width.
    pub fn fill_w(mut self) -> Self {
        self.wide = true;
        self
    }
    /// Secondary when `yes`, ghost otherwise: a hand-made segmented control.
    pub fn active(mut self, yes: bool) -> Self {
        self.variant = if yes {
            Variant::Secondary
        } else {
            Variant::Ghost
        };
        self
    }
    /// Explicit focus id.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
}
impl<S: 'static> Btn<Act<S>> {
    /// Closure style: run `f` on the state when pressed.
    pub fn act(self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.on(Act::new(f))
    }
}
impl<M: Clone + 'static> Btn<M> {
    fn build(self) -> Node<M> {
        let mut b = w::Button::new(&cx::ui(), self.label).variant(self.variant);
        if let Some(m) = self.on {
            b = b.on_press(m);
        }
        if self.wide {
            b = b.width(Length::Fill(1));
        }
        Node(b.id(self.id).into_element())
    }
}
impl<M: Clone + 'static> From<Btn<M>> for Node<M> {
    fn from(b: Btn<M>) -> Self {
        b.build()
    }
}

// ---------------------------------------------------- value widgets

/// A lens into the state: which field a widget edits.
type Lens<S, T> = Arc<dyn for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync>;

fn set_via<S: 'static, T: Send + Sync + Clone + 'static>(
    lens: impl for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync + 'static,
) -> impl Fn(T) -> Act<S> {
    let lens: Lens<S, T> = Arc::new(lens);
    move |v| {
        let lens = lens.clone();
        Act::new(move |s| *lens(s) = v.clone())
    }
}

fn set_with<S: 'static, T: Send + Sync + Clone + 'static>(
    f: impl Fn(&mut S, T) + Send + Sync + 'static,
) -> impl Fn(T) -> Act<S> {
    let f = Arc::new(f);
    move |v| {
        let f = f.clone();
        Act::new(move |s| f(s, v.clone()))
    }
}

macro_rules! value_widget {
    ($name:ident, $fnname:literal, $ty:ty, $doc:literal) => {
        impl<M: 'static> $name<M> {
            #[doc = $doc]
            pub fn on(mut self, f: impl Fn($ty) -> M + 'static) -> Self {
                self.on = Some(Box::new(f));
                self
            }
        }
        impl<S: 'static> $name<Act<S>> {
            /// Closure style: `.act(|state, value| ..)`.
            pub fn act(self, f: impl Fn(&mut S, $ty) + Send + Sync + 'static) -> Self {
                self.on(set_with(f))
            }
            /// Closure style: write the value straight into a field, `.bind(|s| &mut s.field)`.
            pub fn bind(
                self,
                lens: impl for<'a> Fn(&'a mut S) -> &'a mut $ty + Send + Sync + 'static,
            ) -> Self {
                self.on(set_via(lens))
            }
            /// Write the value into a [`Local`](crate::Local) view-state cell.
            pub fn local(self, cell: &$crate::Local<$ty>) -> Self {
                let cell = cell.clone();
                self.on(move |v| {
                    let cell = cell.clone();
                    Act::new(move |_| cell.set(v.clone()))
                })
            }
        }
    };
}

/// A single-line text field. See [`input`].
pub struct Input<M> {
    value: String,
    hint: String,
    on: Option<Box<dyn Fn(String) -> M>>,
    enter: Option<Box<dyn Fn() -> M>>,
    id: u64,
    wide: bool,
}

/// A single-line text field showing `value`.
#[track_caller]
pub fn input<M>(value: impl Into<String>) -> Input<M> {
    Input {
        value: value.into(),
        hint: String::new(),
        on: None,
        enter: None,
        id: auto_id(2),
        wide: false,
    }
}
impl<M> Input<M> {
    /// Placeholder shown while empty.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
    /// Stretch to the available width.
    pub fn fill_w(mut self) -> Self {
        self.wide = true;
        self
    }
    /// Message sent when Enter is pressed.
    pub fn enter(mut self, m: M) -> Self
    where
        M: Clone + 'static,
    {
        self.enter = Some(Box::new(move || m.clone()));
        self
    }
    /// Explicit id.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
}
impl<S: 'static> Input<Act<S>> {
    /// Closure style: run `f` on the state when Enter is pressed.
    pub fn submit(self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.enter(Act::new(f))
    }
}
value_widget!(
    Input,
    "input",
    String,
    "Called with the new text on every edit."
);
impl<M: 'static> From<Input<M>> for Node<M> {
    fn from(i: Input<M>) -> Self {
        let mut t = w::TextInput::new(&cx::ui(), i.id, i.value).placeholder(i.hint);
        if let Some(f) = i.on {
            t = t.on_input(f);
        }
        if let Some(f) = i.enter {
            t = t.on_submit(f);
        }
        if i.wide {
            t = t.width(Length::Fill(1));
        }
        Node(t.into_element())
    }
}

/// A multi-line text area. See [`editor`].
pub struct Editor<M> {
    value: String,
    hint: String,
    on: Option<Box<dyn Fn(String) -> M>>,
    id: u64,
}
/// A multi-line text area that fills the space it is given.
#[track_caller]
pub fn editor<M>(value: impl Into<String>) -> Editor<M> {
    Editor {
        value: value.into(),
        hint: String::new(),
        on: None,
        id: auto_id(3),
    }
}
impl<M> Editor<M> {
    /// Placeholder shown while empty.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
}
value_widget!(
    Editor,
    "editor",
    String,
    "Called with the whole text on every edit."
);
impl<M: 'static> From<Editor<M>> for Node<M> {
    fn from(e: Editor<M>) -> Self {
        let mut t = w::TextEditor::new(&cx::ui(), e.id, e.value)
            .placeholder(e.hint)
            .width(Length::Fill(1))
            .height(Length::Fill(1));
        if let Some(f) = e.on {
            t = t.on_input(f);
        }
        Node(t.into_element())
    }
}

macro_rules! switch {
    ($name:ident, $fnname:ident, $widget:ident, $salt:literal, $doc:literal) => {
        #[doc = $doc]
        pub struct $name<M> {
            label: String,
            value: bool,
            on: Option<Box<dyn Fn(bool) -> M>>,
            id: u64,
        }
        #[doc = $doc]
        #[track_caller]
        pub fn $fnname<M>(label: impl Into<String>, value: bool) -> $name<M> {
            $name {
                label: label.into(),
                value,
                on: None,
                id: auto_id($salt),
            }
        }
        value_widget!($name, "", bool, "Called with the requested new value.");
        impl<M: 'static> From<$name<M>> for Node<M> {
            fn from(c: $name<M>) -> Self {
                let mut t = w::$widget::new(&cx::ui(), c.label, c.value).id(c.id);
                if let Some(f) = c.on {
                    t = t.on_toggle(f);
                }
                Node(t.into_element())
            }
        }
    };
}
switch!(Check, check, Checkbox, 4, "A labelled checkbox.");
switch!(Switch, toggle, Toggle, 5, "A labelled on/off switch.");

/// A radio option. See [`radio`].
pub struct Radio<M> {
    label: String,
    on: Option<M>,
    selected: bool,
    id: u64,
}
/// A radio option, `selected` when it is the current choice.
#[track_caller]
pub fn radio<M>(label: impl Into<String>, selected: bool) -> Radio<M> {
    Radio {
        label: label.into(),
        on: None,
        selected,
        id: auto_id(6),
    }
}
impl<M> Radio<M> {
    /// Message sent when chosen.
    pub fn on(mut self, m: M) -> Self {
        self.on = Some(m);
        self
    }
}
impl<M: Clone + 'static> From<Radio<M>> for Node<M> {
    fn from(r: Radio<M>) -> Self {
        let mut t = w::Radio::new(&cx::ui(), r.label, r.selected).id(r.id);
        if let Some(m) = r.on {
            t = t.on_select(m);
        }
        Node(t.into_element())
    }
}

/// A horizontal slider. See [`slider`].
pub struct Slide<M> {
    range: (f32, f32),
    value: f32,
    step: Option<f32>,
    on: Option<Box<dyn Fn(f32) -> M>>,
    id: u64,
    wide: bool,
}
/// A slider over `range` showing `value`.
#[track_caller]
pub fn slider<M>(range: std::ops::RangeInclusive<f32>, value: f32) -> Slide<M> {
    Slide {
        range: (*range.start(), *range.end()),
        value,
        step: None,
        on: None,
        id: auto_id(7),
        wide: true,
    }
}
impl<M> Slide<M> {
    /// Snap to multiples of `s`.
    pub fn step(mut self, s: f32) -> Self {
        self.step = Some(s);
        self
    }
}
value_widget!(
    Slide,
    "slider",
    f32,
    "Called with the new value while dragging."
);
impl<M: 'static> From<Slide<M>> for Node<M> {
    fn from(s: Slide<M>) -> Self {
        let mut t = w::Slider::new(&cx::ui(), s.range.0, s.range.1, s.value).id(s.id);
        if let Some(st) = s.step {
            t = t.step(st);
        }
        if let Some(f) = s.on {
            t = t.on_change(f);
        }
        if s.wide {
            t = t.width(Length::Fill(1));
        }
        Node(t.into_element())
    }
}

/// A progress bar over `range`.
pub fn progress<M: 'static>(range: std::ops::RangeInclusive<f32>, value: f32) -> Node<M> {
    Node(
        w::ProgressBar::new(&cx::ui(), *range.start(), *range.end(), value)
            .width(Length::Fill(1))
            .into_element(),
    )
}

/// A segmented control over fixed choices. See [`choose`] and [`tabs`].
pub struct Seg<M> {
    labels: Vec<String>,
    selected: usize,
    on: Option<Box<dyn Fn(usize) -> M>>,
    id: u64,
}
/// A segmented control selecting an index.
#[track_caller]
pub fn tabs<M, L: Into<String>>(labels: impl IntoIterator<Item = L>, selected: usize) -> Seg<M> {
    Seg {
        labels: labels.into_iter().map(Into::into).collect(),
        selected,
        on: None,
        id: auto_id(8),
    }
}
value_widget!(Seg, "tabs", usize, "Called with the picked index.");
impl<M: 'static> From<Seg<M>> for Node<M> {
    fn from(s: Seg<M>) -> Self {
        let mut t = w::Segmented::new(&cx::ui(), s.id, s.labels, s.selected);
        if let Some(f) = s.on {
            t = t.on_select(f);
        }
        Node(t.into_element())
    }
}

/// A segmented control over typed values, so no index bookkeeping:
/// `choose([("All", Filter::All), ("Open", Filter::Open)], &self.filter).on(Msg::Filter)`.
pub struct Choose<T, M> {
    opts: Vec<(String, T)>,
    cur: usize,
    on: Option<Box<dyn Fn(T) -> M>>,
    id: u64,
}
/// See [`Choose`]. `current` is matched against the options with `==`.
#[track_caller]
pub fn choose<T: PartialEq, M, L: Into<String>>(
    opts: impl IntoIterator<Item = (L, T)>,
    current: &T,
) -> Choose<T, M> {
    let opts: Vec<(String, T)> = opts.into_iter().map(|(l, v)| (l.into(), v)).collect();
    let cur = opts
        .iter()
        .position(|(_, v)| v == current)
        .unwrap_or(usize::MAX);
    Choose {
        opts,
        cur,
        on: None,
        id: auto_id(9),
    }
}
impl<T, M> Choose<T, M> {
    /// Called with the picked value.
    pub fn on(mut self, f: impl Fn(T) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
}
impl<T: Clone + Send + Sync + 'static, S: 'static> Choose<T, Act<S>> {
    /// Write the picked value into a [`Local`](crate::Local) view-state cell.
    pub fn local(self, cell: &crate::Local<T>) -> Self {
        let cell = cell.clone();
        self.act(move |_, v| cell.set(v))
    }
    /// Closure style: `.act(|state, picked| ..)`.
    pub fn act(self, f: impl Fn(&mut S, T) + Send + Sync + 'static) -> Self {
        self.on(set_with(f))
    }
    /// Closure style: write the picked value into a field, `.bind(|s| &mut s.mode)`.
    pub fn bind(
        self,
        lens: impl for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync + 'static,
    ) -> Self {
        self.on(set_via(lens))
    }
}
impl<T: Clone + 'static, M: 'static> From<Choose<T, M>> for Node<M> {
    fn from(c: Choose<T, M>) -> Self {
        let labels: Vec<String> = c.opts.iter().map(|(l, _)| l.clone()).collect();
        let vals: Vec<T> = c.opts.into_iter().map(|(_, v)| v).collect();
        let mut t = w::Segmented::new(&cx::ui(), c.id, labels, c.cur);
        if let Some(f) = c.on {
            t = t.on_select(move |i| f(vals[i].clone()));
        }
        Node(t.into_element())
    }
}

/// A drop-down list. See [`pick`].
pub struct Pick<M> {
    opts: Vec<String>,
    selected: Option<usize>,
    hint: String,
    on: Option<Box<dyn Fn(usize) -> M>>,
    id: u64,
}
/// A drop-down over `options`.
#[track_caller]
pub fn pick<M, L: Into<String>>(
    options: impl IntoIterator<Item = L>,
    selected: Option<usize>,
) -> Pick<M> {
    Pick {
        opts: options.into_iter().map(Into::into).collect(),
        selected,
        hint: String::new(),
        on: None,
        id: auto_id(10),
    }
}
impl<M> Pick<M> {
    /// Text shown when nothing is selected.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
}
value_widget!(Pick, "pick", usize, "Called with the picked index.");
impl<M: 'static> From<Pick<M>> for Node<M> {
    fn from(p: Pick<M>) -> Self {
        let mut t = w::PickList::new(&cx::ui(), p.id, p.opts, p.selected).placeholder(p.hint);
        if let Some(f) = p.on {
            t = t.on_select(f);
        }
        Node(t.into_element())
    }
}

// ------------------------------------------------------------ layout bits

/// A scrolling area around `child`; fills the space it is given.
#[track_caller]
pub fn scroll<M: 'static>(child: impl Into<Node<M>>) -> Node<M> {
    let id = auto_id(11);
    Node(
        w::Scrollable::new(&cx::ui(), id, child.into().0)
            .width(Length::Fill(1))
            .height(Length::Fill(1))
            .into_element(),
    )
}

/// Flexible empty space inside a row: pushes siblings apart (use [`vspace`] in a column).
pub fn space<M: 'static>() -> Node<M> {
    Node(w::Space::fill_w().into_element())
}

/// Flexible empty space inside a column.
pub fn vspace<M: 'static>() -> Node<M> {
    Node(w::Space::fill_h().into_element())
}

/// Fixed empty space.
pub fn gap<M: 'static>(w_px: f32, h_px: f32) -> Node<M> {
    Node(w::Space::fixed(w_px, h_px).into_element())
}

/// A thin divider line.
pub fn rule<M: 'static>() -> Node<M> {
    Node(w::Rule::horizontal(&cx::ui()).into_element())
}

/// Any raw element (custom widgets) as a view.
pub fn custom<M>(e: Element<M>) -> Node<M> {
    Node(e)
}

// ------------------------------------------------------------ choices

/// A small enum whose variants are the options of a [`options`] / [`nav`]
/// control. Declare one with [`choices!`](crate::choices).
pub trait Choice: Copy + PartialEq + 'static {
    /// Every variant with its label, in display order.
    fn all() -> &'static [(&'static str, Self)];
    /// Just the labels.
    fn labels() -> Vec<&'static str> {
        Self::all().iter().map(|(l, _)| *l).collect()
    }
}

/// Declare an enum usable with [`options`] and [`nav`]; labels are the variant names.
///
/// ```
/// alloy_ui::choices! { enum Filter { All, Open, Done } }
/// ```
#[macro_export]
macro_rules! choices {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($var:ident),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        $vis enum $name { $($var),+ }
        impl ::std::default::Default for $name {
            fn default() -> Self {
                <Self as $crate::Choice>::all()[0].1
            }
        }
        impl $crate::serde::Serialize for $name {
            fn serialize<S: $crate::serde::Serializer>(&self, s: S) -> ::std::result::Result<S::Ok, S::Error> {
                s.serialize_str(<Self as $crate::Choice>::all().iter().find(|(_, v)| v == self).unwrap().0)
            }
        }
        impl<'de> $crate::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::serde::Deserializer<'de>>(d: D) -> ::std::result::Result<Self, D::Error> {
                let t = <String as $crate::serde::Deserialize>::deserialize(d)?;
                <Self as $crate::Choice>::all().iter().find(|(l, _)| *l == t).map(|(_, v)| *v)
                    .ok_or_else(|| <D::Error as $crate::serde::de::Error>::custom("unknown variant"))
            }
        }
        impl $crate::Choice for $name {
            fn all() -> &'static [(&'static str, Self)] {
                &[$((stringify!($var), $name::$var)),+]
            }
        }
    };
}

/// A segmented control over every variant of a [`Choice`] enum.
#[track_caller]
pub fn options<T: Choice, M>(current: &T) -> Choose<T, M> {
    choose(T::all().iter().copied(), current)
}

/// A vertical list of buttons, one per option, the current one highlighted.
pub struct Nav<T, M> {
    opts: Vec<(String, T)>,
    cur: T,
    on: Option<Box<dyn Fn(T) -> M>>,
}

/// A vertical navigation list over every variant of a [`Choice`] enum.
pub fn nav<T: Choice, M>(current: &T) -> Nav<T, M> {
    Nav {
        opts: T::all().iter().map(|(l, v)| (l.to_string(), *v)).collect(),
        cur: *current,
        on: None,
    }
}
impl<T, M> Nav<T, M> {
    pub(crate) fn of(opts: Vec<(String, T)>, cur: T) -> Self {
        Nav {
            opts,
            cur,
            on: None,
        }
    }
    /// Called with the picked value.
    pub fn on(mut self, f: impl Fn(T) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
}
impl<T: Choice + Send + Sync, S: 'static> Nav<T, Act<S>> {
    /// Closure style: write the picked value into a field, `.bind(|s| &mut s.page)`.
    pub fn bind(
        self,
        lens: impl for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync + 'static,
    ) -> Self {
        self.on(set_via(lens))
    }
}
impl<T: PartialEq, M: Clone + 'static> From<Nav<T, M>> for Node<M> {
    fn from(n: Nav<T, M>) -> Self {
        let on = n.on;
        let items: Vec<Node<M>> = n
            .opts
            .into_iter()
            .map(|(label, v)| {
                let mut b = button(label).active(v == n.cur).fill_w();
                if let Some(f) = &on {
                    b = b.on(f(v));
                }
                b.into()
            })
            .collect();
        crate::view::col(items).gap(2.0).pad(12.0).into()
    }
}

/// A captioned group of controls, the building block of settings pages.
pub fn section<M: 'static>(title: &str, body: impl Into<Node<M>>) -> crate::view::Flex<M> {
    crate::view::col([text(title).label().dim().into(), body.into()])
}
