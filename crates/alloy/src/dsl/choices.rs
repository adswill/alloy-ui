//! Enum-backed choices, navigation, sections, lists.

use super::*;

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

#[doc(hidden)]
#[macro_export]
macro_rules! __choice_label {
    ($v:ident) => {
        stringify!($v)
    };
    ($v:ident, $l:literal) => {
        $l
    };
}

/// Declare an enum usable with [`options`] and [`nav`]; labels are the variant names unless a
/// variant has one: `enum Mode { Focus, Short = "Short break" }`.
///
/// ```
/// alloy_ui::choices! { enum Filter { All, Open, Done } }
/// ```
#[macro_export]
macro_rules! choices {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($var:ident $(= $label:literal)?),+ $(,)? }) => {
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
                &[$(($crate::__choice_label!($var $(, $label)?), $name::$var)),+]
            }
        }
    };
}

/// A segmented control over every variant of a [`Choice`] enum.
#[track_caller]
pub fn options<T: Choice, M>(current: impl Val<T, M>) -> Choose<T, M> {
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
                let mut b = button(label).nav(v == n.cur).fill_w();
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

/// A scrolling list of rich rows with one selected. Made by [`list`].
pub struct ListView<M> {
    rows: Vec<Node<M>>,
    selected: Option<usize>,
    on: Option<Box<dyn Fn(usize) -> M>>,
    id: u64,
}

/// A selectable, keyboard-navigable list whose rows are any views:
/// `list(notes.iter().map(|n| col![text(&n.title), caption(&n.preview)]), Some(cur)).act(|s, i| s.cur = i)`.
#[track_caller]
pub fn list<M, V: Into<Node<M>>>(
    rows: impl IntoIterator<Item = V>,
    selected: Option<usize>,
) -> ListView<M> {
    ListView {
        rows: rows.into_iter().map(Into::into).collect(),
        selected,
        on: None,
        id: auto_id(40),
    }
}
value_widget!(
    ListView,
    "list",
    usize,
    "Called with the row's position when one is picked."
);
impl<M: 'static> From<ListView<M>> for Node<M> {
    fn from(l: ListView<M>) -> Self {
        let on = l.on;
        let pick = move |i| match &on {
            Some(f) => f(i),
            None => unreachable!("list needs .on(..) or .act(..)"),
        };
        Node(
            w::List::new(
                &cx::ui(),
                l.id,
                l.rows.into_iter().map(|r| r.0).collect(),
                l.selected,
                pick,
            )
            .into_element(),
        )
    }
}

/// Show what a fallible step produced: the view of its value, or `message: error` in red.
///
/// ```ignore
/// attempt(fs::list(&s.dir), "Cannot read folder", |files| table(&files).col("Name", Entry::label))
/// ```
pub fn attempt<T, E: std::fmt::Display, M: 'static, V: Into<Node<M>>>(
    result: Result<T, E>,
    message: &str,
    view: impl FnOnce(T) -> V,
) -> Node<M> {
    match result {
        Ok(v) => view(v).into(),
        Err(e) => text(format!("{message}: {e}")).danger().into(),
    }
}
