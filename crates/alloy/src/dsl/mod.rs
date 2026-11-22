//! Widget helpers: one short function per widget, builder methods for options.

use crate::{
    bind::{Place, Val},
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

    /// Run the mutation on `state`. Apps that implement [`App`](crate::App) by hand with
    /// `Msg = Act<State>` call this from `update`.
    pub fn apply(&self, state: &mut S) {
        (self.0)(state)
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
    italic: bool,
    weight: Option<u16>,
}

/// Body text. Roles: `.heading()`, `.caption()`, ...
pub fn text(s: impl Into<String>) -> Txt {
    Txt {
        s: s.into(),
        role: w::TextRole::Body,
        dim: false,
        color: None,
        wrap: true,
        width: Length::Shrink,
        italic: false,
        weight: None,
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
    /// True italic (Alloy Sans Italic).
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    /// Font weight, 100 to 900 (400 is regular, 600 semibold, 700 bold). Combines with
    /// [`color`](Self::color) and the roles: `text("Online").weight(600).color(green)`.
    pub fn weight(mut self, w: u16) -> Self {
        self.weight = Some(w.clamp(100, 900));
        self
    }
    /// Bold (weight 700).
    pub fn bold(self) -> Self {
        self.weight(700)
    }
    /// Semibold (weight 600).
    pub fn semibold(self) -> Self {
        self.weight(600)
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
    /// Accent colour when `yes`.
    pub fn accent_if(self, yes: bool) -> Self {
        if yes {
            self.accent()
        } else {
            self
        }
    }
    /// Dim when `yes`.
    pub fn dim_if(self, yes: bool) -> Self {
        if yes {
            self.dim()
        } else {
            self
        }
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
        if self.italic {
            t = t.italic();
        }
        if let Some(wt) = self.weight {
            t = t.weight(wt);
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
    off: bool,
    sel: bool,
    tint: Option<Color>,
    hover: Option<Color>,
    esc: Option<M>,
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
        off: false,
        sel: false,
        tint: None,
        hover: None,
        esc: None,
        id: auto_id(1),
    }
}

impl<M> Btn<M> {
    /// Message sent on Escape while a [`confirm`](Btn::confirm) question is showing.
    pub fn on_escape(mut self, m: M) -> Self {
        self.esc = Some(m);
        self
    }
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
    /// Fill with your own colour. It lightens on hover and darkens when pressed, and the label
    /// picks white or near-black for contrast: `button("Play").tint(green)`.
    pub fn tint(mut self, c: Color) -> Self {
        self.tint = Some(c);
        self
    }
    /// The colour while hovered, for a [`tint`](Self::tint)ed button.
    pub fn hover(mut self, c: Color) -> Self {
        self.hover = Some(c);
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
    /// A sidebar row: left-aligned, no outline, highlighted when `current`.
    pub fn nav(mut self, current: bool) -> Self {
        self.variant = Variant::Nav;
        self.sel = current;
        self
    }
    /// Explicit focus id.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
    /// Disable the button unless `yes`, wherever in the chain this is called.
    pub fn enabled(mut self, yes: bool) -> Self {
        self.off = !yes;
        self
    }
}
impl<S: 'static> Btn<Act<S>> {
    /// Closure style: run `f` on the state when pressed.
    pub fn act(self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.on(Act::new(f))
    }
}
impl<S: 'static> Btn<Act<S>> {
    /// Ask first: the button turns into "prompt [label] [Cancel]" and only the second click
    /// runs the action. Put it last, after `.act(..)`.
    ///
    /// While the question shows, Escape cancels it, like a modal. `.on_escape(..)` adds a
    /// handler that also runs then.
    #[track_caller]
    pub fn confirm(mut self, prompt: &str) -> Node<Act<S>> {
        let asking = crate::local(false);
        let Some(go) = self.on.take() else {
            return self.into();
        };
        if !asking.get() {
            self.on = Some(Act::new(move |_| asking.set(true)));
            return self.into();
        }
        let sure = button(self.label).primary().act(move |s: &mut S| {
            asking.set(false);
            (go.0)(s)
        });
        let back = button("Cancel").ghost().act(move |_| asking.set(false));
        let extra = self.esc.take();
        crate::keys::register(crate::keys::Hotkey::new(
            "esc",
            Act::new(move |s: &mut S| {
                asking.set(false);
                if let Some(e) = &extra {
                    (e.0)(s);
                }
            }),
        ));
        crate::row([Node::from(caption(prompt)), sure.into(), back.into()])
            .align_center()
            .gap(8.0)
            .into()
    }
}
impl<M: Clone + 'static> Btn<M> {
    fn build(self) -> Node<M> {
        let mut b = w::Button::new(&cx::ui(), self.label)
            .variant(self.variant)
            .selected(self.sel);
        if let Some(c) = self.tint {
            b = b.tint(c);
        }
        if let Some(c) = self.hover {
            b = b.hover_tint(c);
        }
        if let Some(m) = self.on.filter(|_| !self.off) {
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
            pub fn local(self, cell: &impl $crate::Cell<$ty>) -> Self {
                let cell = *cell;
                self.on(move |v| Act::new(move |_| cell.write(v.clone())))
            }
        }
    };
}

mod choices;
mod values;

pub use choices::*;
pub use values::*;

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

/// [`scroll`] that stays pinned to the bottom, like a chat log: it starts at the newest content,
/// follows it as it grows while the view is at the bottom, and stops following when the user
/// scrolls up (until they scroll back to the end).
#[track_caller]
pub fn scroll_bottom<M: 'static>(child: impl Into<Node<M>>) -> Node<M> {
    let id = auto_id(11);
    Node(
        w::Scrollable::new(&cx::ui(), id, child.into().0)
            .width(Length::Fill(1))
            .height(Length::Fill(1))
            .stick_bottom()
            .into_element(),
    )
}

/// [`scroll`] that also scrolls by dragging the content, like a touch screen: it follows the
/// pointer, stretches past the ends with resistance and keeps coasting when let go. Buttons and
/// other controls inside still take their own presses.
#[track_caller]
pub fn scroll_touch<M: 'static>(child: impl Into<Node<M>>) -> Node<M> {
    let id = auto_id(11);
    Node(
        w::Scrollable::new(&cx::ui(), id, child.into().0)
            .width(Length::Fill(1))
            .height(Length::Fill(1))
            .touch_drag()
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
