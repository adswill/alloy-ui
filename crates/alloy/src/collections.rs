#![allow(clippy::type_complexity)]
//! Data-shaped blocks: cells and lenses, a searchable list with a detail pane, stat cards,
//! filter chips.

use crate::{
    blocks::{local, store, Local},
    cx,
    dsl::{button, caption, heading, input, list, text, Act, Check, Choice, Editor, Input, Switch},
    view::{col, row, Flex, Node, View},
};
use std::{cmp::Ordering, marker::PhantomData};

// ------------------------------------------------------------------ cells

/// Anything a widget can read and write: a [`Local`], or a [`Prop`] of one row of a list.
/// `.local(&cell)` on a widget writes into it, and the shortcuts below ([`TextCell`],
/// [`FlagCell`]) build the widget from it: `query.input()`, `note.field(|n| &mut n.pinned).check("Pinned")`.
pub trait Cell<V>: Copy + Send + Sync + 'static {
    /// The current value.
    fn read(self) -> V;
    /// Replace the value.
    fn write(self, v: V);
}

impl<T: Clone + Send + 'static> Cell<T> for Local<T> {
    fn read(self) -> T {
        self.get()
    }
    fn write(self, v: T) {
        self.set(v)
    }
}

/// Widgets for a text cell.
pub trait TextCell: Cell<String> {
    /// A single-line field that edits the cell.
    #[track_caller]
    fn input<S: 'static>(self) -> Input<Act<S>> {
        input(self.read()).local(&self)
    }
    /// A multi-line editor that edits the cell, with a cursor of its own per list row.
    #[track_caller]
    fn editor<S: 'static>(self) -> Editor<Act<S>> {
        crate::editor(self.read()).local(&self)
    }
}
impl<C: Cell<String>> TextCell for C {}

/// Widgets for a `bool` cell.
pub trait FlagCell: Cell<bool> {
    /// A checkbox that edits the cell.
    #[track_caller]
    fn check<S: 'static>(self, label: &str) -> Check<Act<S>> {
        crate::check(label, self.read()).local(&self)
    }
    /// An on/off switch that edits the cell.
    #[track_caller]
    fn switch<S: 'static>(self, label: &str) -> Switch<Act<S>> {
        crate::toggle(label, self.read()).local(&self)
    }
}
impl<C: Cell<bool>> FlagCell for C {}

impl<T: Send + 'static> Local<Vec<T>> {
    /// Append a row.
    pub fn push(self, v: T) {
        self.with(|x| x.push(v));
    }
    /// How many rows there are.
    pub fn len(self) -> usize {
        self.with(|x| x.len())
    }
    /// Whether there are no rows.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    /// A handle to row `at`.
    pub fn item(self, at: usize) -> Item<T> {
        Item { list: self, at }
    }
}

/// One row of a `Local<Vec<T>>`: a `Copy` handle to read it, change it or remove it.
pub struct Item<T> {
    list: Local<Vec<T>>,
    at: usize,
}
impl<T> Clone for Item<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Item<T> {}

impl<T: Send + 'static> Item<T> {
    /// Position in the list.
    pub fn index(self) -> usize {
        self.at
    }
    /// Read or change the row in place (does nothing if it is gone).
    pub fn with<R>(self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.list.with(|v| v.get_mut(self.at).map(f))
    }
    /// Delete the row.
    pub fn remove(self) {
        self.list.with(|v| {
            if self.at < v.len() {
                v.remove(self.at);
            }
        });
    }
    /// One field of the row as a [`Cell`]: `row.field(|n| &mut n.title)`.
    pub fn field<V>(self, lens: fn(&mut T) -> &mut V) -> Prop<T, V> {
        Prop {
            item: self,
            lens,
            _v: PhantomData,
        }
    }
}
impl<T: Clone + Send + 'static> Item<T> {
    /// A copy of the row.
    pub fn get(self) -> Option<T> {
        self.with(|t| t.clone())
    }
}

/// One field of one row, made by [`Item::field`].
pub struct Prop<T, V> {
    item: Item<T>,
    lens: fn(&mut T) -> &mut V,
    _v: PhantomData<fn() -> V>,
}
impl<T, V> Clone for Prop<T, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, V> Copy for Prop<T, V> {}

impl<T: Send + 'static, V: Clone + Default + Send + 'static> Cell<V> for Prop<T, V> {
    fn read(self) -> V {
        self.item
            .with(|t| (self.lens)(t).clone())
            .unwrap_or_default()
    }
    fn write(self, v: V) {
        self.item.with(|t| *(self.lens)(t) = v);
    }
}

// ---------------------------------------------------------- list + detail

type RowFn<T> = Box<dyn Fn(&T) -> (String, String)>;
type DetailFn<T, S> = Box<dyn Fn(Item<T>) -> Node<Act<S>>>;
type FootFn<T, S> = Box<dyn Fn(&T) -> Node<Act<S>>>;
type Order<T> = (String, Box<dyn Fn(&T, &T) -> Ordering>);

/// A searchable list next to a detail pane. Made by [`browse`].
pub struct Browse<T, S> {
    items: Local<Vec<T>>,
    row: Option<RowFn<T>>,
    search: Option<Box<dyn Fn(&T) -> String>>,
    orders: Vec<Order<T>>,
    pin: Option<Box<dyn Fn(&T) -> bool>>,
    new: Option<Box<dyn Fn() -> T + Send + Sync>>,
    detail: Option<DetailFn<T, S>>,
    form: Option<fn(Item<T>, Option<&FootFn<T, S>>) -> Node<Act<S>>>,
    foot: Option<FootFn<T, S>>,
    empty: String,
}

/// A list-detail view over the rows of `items`: a search box, the rows (title and a dimmer
/// line each), a "New" button, and a pane for the selected row.
///
/// ```ignore
/// browse(&notes)
///     .row(|n| (n.title(), n.preview()))
///     .new(Note::default)
///     .detail(|n| n.field(|n| &mut n.body).editor())
/// ```
///
/// The selection survives restarts when the app persists its [`store`]s.
pub fn browse<T: Send + 'static, S: 'static>(items: &Local<Vec<T>>) -> Browse<T, S> {
    Browse {
        items: *items,
        row: None,
        search: None,
        orders: Vec::new(),
        pin: None,
        new: None,
        detail: None,
        form: None,
        foot: None,
        empty: "Nothing selected".into(),
    }
}

impl<T: Send + 'static, S: 'static> Browse<T, S> {
    /// A row's `(title, subtitle)`. Required.
    pub fn row(mut self, f: impl Fn(&T) -> (String, String) + 'static) -> Self {
        self.row = Some(Box::new(f));
        self
    }
    /// Rows are the text itself: the first non-blank line is the title ("Untitled" if none), the
    /// next one the dimmer line, and the search looks in all of it. For notes, mails, snippets.
    pub fn doc(self, text: impl Fn(&T) -> &str + Clone + 'static) -> Self {
        self.doc_of(move |x| text(x).to_string())
    }
    fn doc_of(mut self, text: impl Fn(&T) -> String + Clone + 'static) -> Self {
        let t = text.clone();
        self.row = Some(Box::new(move |x| {
            let body = t(x);
            let mut lines = body.lines().map(str::trim).filter(|l| !l.is_empty());
            let title = lines.next().unwrap_or("Untitled").to_string();
            let more = lines.next().unwrap_or("No more text");
            (title, more.chars().take(48).collect())
        }));
        self.search = Some(Box::new(text));
        self
    }
    /// Rows for which this is true show a ★ and stay above the others, whatever the order.
    pub fn pin(mut self, f: impl Fn(&T) -> bool + 'static) -> Self {
        self.pin = Some(Box::new(f));
        self
    }
    /// The text the search box looks in (default: title and subtitle).
    pub fn search(mut self, f: impl Fn(&T) -> String + 'static) -> Self {
        self.search = Some(Box::new(f));
        self
    }
    /// How rows are ordered on screen (default: as stored). Read your sort choice with a [`store`].
    pub fn order(mut self, f: impl Fn(&T, &T) -> Ordering + 'static) -> Self {
        self.orders = vec![(String::new(), Box::new(f))];
        self
    }
    /// Offer an ordering under `label`. With two or more, a switch above the list picks one
    /// (the first is the default) and the choice is saved with the app's stores.
    pub fn sort_by(mut self, label: &str, f: impl Fn(&T, &T) -> Ordering + 'static) -> Self {
        self.orders.push((label.into(), Box::new(f)));
        self
    }
    /// A line under the detail form of [`browse_of`], left of the Delete button.
    pub fn foot<N: Into<Node<Act<S>>>>(mut self, f: impl Fn(&T) -> N + 'static) -> Self {
        self.foot = Some(Box::new(move |t| f(t).into()));
        self
    }
    /// Show a "New" button that appends `make()` and selects it.
    pub fn new(mut self, make: impl Fn() -> T + Send + Sync + 'static) -> Self {
        self.new = Some(Box::new(make));
        self
    }
    /// The pane for the selected row. It runs once per frame and gets its own widget
    /// state per row, so each row keeps its own cursor and scroll position.
    pub fn detail<N: Into<Node<Act<S>>>>(mut self, f: impl Fn(Item<T>) -> N + 'static) -> Self {
        self.detail = Some(Box::new(move |i| f(i).into()));
        self
    }
    /// Text shown when there is nothing to select.
    pub fn empty(mut self, t: &str) -> Self {
        self.empty = t.into();
        self
    }
}

impl<T: Send + 'static, S: 'static> From<Browse<T, S>> for Node<Act<S>> {
    fn from(b: Browse<T, S>) -> Self {
        let (cur, query) = (store(|| 0usize), local(String::new()));
        let by = (b.orders.len() > 1).then(|| store(|| 0usize));
        let items = b.items;
        let row_of = b.row.expect("browse needs .row(|item| (title, subtitle))");
        let q = query.get().to_lowercase();
        // (index, title, subtitle), filtered by the query, in display order.
        let shown: Vec<(usize, String, String)> = items.with(|all| {
            let mut v: Vec<usize> = (0..all.len())
                .filter(|&i| {
                    let (t, s) = row_of(&all[i]);
                    match &b.search {
                        Some(f) => f(&all[i]),
                        None => format!("{t} {s}"),
                    }
                    .to_lowercase()
                    .contains(&q)
                })
                .collect();
            let pinned = |i: usize| b.pin.as_ref().is_some_and(|p| p(&all[i]));
            v.sort_by(|&a, &c| {
                pinned(c).cmp(&pinned(a)).then_with(|| {
                    let at = by
                        .map_or(0, |l| l.get())
                        .min(b.orders.len().saturating_sub(1));
                    b.orders
                        .get(at)
                        .map_or(Ordering::Equal, |o| (o.1)(&all[a], &all[c]))
                })
            });
            v.into_iter()
                .map(|i| {
                    let (t, s) = row_of(&all[i]);
                    (i, if pinned(i) { format!("★ {t}") } else { t }, s)
                })
                .collect()
        });
        let len = items.len();
        let sel = cur.get().min(len.saturating_sub(1));
        let at = shown.iter().position(|r| r.0 == sel);
        let rows: Vec<Flex<Act<S>>> = shown
            .iter()
            .map(|(_, t, s)| {
                col([Node::from(text(t.clone())), caption(s.clone()).dim().into()]).gap(2.0)
            })
            .collect();
        let idx: Vec<usize> = shown.iter().map(|r| r.0).collect();
        let mut top: Vec<Node<Act<S>>> = vec![input(query.get())
            .hint("Search")
            .local(&query)
            .fill_w()
            .into()];
        if let Some(make) = b.new {
            top.push(
                button("New")
                    .act(move |_: &mut S| {
                        items.push(make());
                        cur.set(items.len() - 1);
                        query.set(String::new());
                    })
                    .into(),
            );
        }
        let top: Node<Act<S>> = row(top).gap(8.0).into();
        let top: Node<Act<S>> = match by {
            Some(l) => col([
                top,
                crate::choose(
                    b.orders.iter().enumerate().map(|(i, o)| (o.0.clone(), i)),
                    &l.get(),
                )
                .local(&l)
                .into(),
            ])
            .gap(8.0)
            .into(),
            None => top,
        };
        let rows: Node<Act<S>> = list(rows, at)
            .act(move |_: &mut S, p| cur.set(idx[p]))
            .into();
        let side = col([top, rows]).gap(12.0).pad(12.0).w(300.0).fill_h();
        let pane: Node<Act<S>> = match (&b.detail, len) {
            (Some(f), 1..) => cx::keyed(sel as u64 + 1, || f(items.item(sel))),
            (None, 1..) if b.form.is_some() => cx::keyed(sel as u64 + 1, || {
                (b.form.unwrap())(items.item(sel), b.foot.as_ref())
            }),
            _ => text(b.empty).dim().into(),
        };
        row([side.into(), pane]).fill().into()
    }
}

/// [`browse`] over a [`Record`](crate::Record), with everything derived: the first text field is
/// the title and the text searched, a `Pinned` checkbox field pins, a form of the fields
/// (and Delete) is the detail pane, "New" appends a default row, and a `Stamp` field adds
/// Recent / A–Z ordering. Override any of it with the usual methods.
///
/// ```ignore
/// browse_of(&notes).foot(|n| caption(format!("{} words", n.body.split_whitespace().count())))
/// ```
pub fn browse_of<T: crate::Record, S: 'static>(items: &Local<Vec<T>>) -> Browse<T, S> {
    use crate::records::{control, Kind, Value};
    let fields = T::fields();
    let text_at = fields
        .iter()
        .position(|f| matches!(f.kind, Kind::Text | Kind::Long))
        .expect("browse_of needs a record with a text field");
    let doc = move |t: &T| match t.get(text_at) {
        Value::Text(s) => s,
        _ => String::new(),
    };
    let mut b = browse(items).doc_of(doc).new(T::default);
    if let Some(p) = fields
        .iter()
        .position(|f| f.label == "Pinned" && matches!(f.kind, Kind::Bool))
    {
        b = b.pin(move |t| matches!(t.get(p), Value::Bool(true)));
    }
    if T::default().stamp().is_some() {
        b = b
            .sort_by("Recent", |a, c| c.stamp().cmp(&a.stamp()))
            .sort_by("A–Z", move |a, c| {
                doc(a).to_lowercase().cmp(&doc(c).to_lowercase())
            });
    }
    b.form = Some(|it, foot| {
        let Some(rec) = it.get() else {
            return text("").into();
        };
        let mut items: Vec<Node<Act<S>>> = Vec::new();
        for (i, f) in T::fields().iter().enumerate() {
            let put = move |v: Value| {
                Act::new(move |_: &mut S| {
                    it.with(|r| {
                        r.set(i, v.clone());
                        r.touch();
                    });
                })
            };
            items.push(control(f, rec.get(i), put));
        }
        let end = foot.map_or_else(|| text("").into(), |f| f(&rec));
        let del = button("Delete")
            .ghost()
            .act(move |_: &mut S| it.remove())
            .confirm("Delete this item?");
        items.push(
            row([end, crate::space(), del.into()])
                .align_center()
                .gap(12.0)
                .into(),
        );
        col(items).gap(12.0).pad(16.0).fill().into()
    });
    b
}

// ---------------------------------------------------------------- stats

/// One number under a label, on a card.
pub fn stat<M: 'static>(label: impl Into<String>, value: impl ToString) -> crate::view::Boxed<M> {
    col([
        Node::from(caption(label.into())),
        heading(value.to_string()).into(),
    ])
    .card()
    .pad(16.0)
    .fill_w()
}

/// A row of [`stat`] cards.
pub fn stats<M: 'static, L: Into<String>, V: ToString>(
    items: impl IntoIterator<Item = (L, V)>,
) -> Flex<M> {
    row(items.into_iter().map(|(l, v)| stat(l, v))).gap(12.0)
}

/// One [`stat`] card per variant of an enum, counting the rows that have it:
/// `tally(&services, |s| s.status)`.
pub fn tally<T, C: Choice, M: 'static>(rows: &[T], by: impl Fn(&T) -> C) -> Flex<M> {
    stats(
        C::all()
            .iter()
            .map(|&(l, c)| (l, rows.iter().filter(|r| by(r) == c).count())),
    )
}

/// "All" plus one chip per variant, bound to a `Local<Option<T>>` (`None` is All).
/// Narrow rows with [`Local::keeps`](crate::Local::keeps).
#[track_caller]
pub fn filter_by<T: Choice + Send + Sync, S: 'static>(
    cell: &Local<Option<T>>,
) -> crate::dsl::Choose<Option<T>, Act<S>> {
    crate::choose(
        std::iter::once(("All", None)).chain(T::all().iter().map(|&(l, v)| (l, Some(v)))),
        &cell.get(),
    )
    .local(cell)
}

impl<T: PartialEq + Clone + Send + 'static> Local<Option<T>> {
    /// Whether a row with this value passes the filter (everything does while it is `None`).
    pub fn keeps(self, v: &T) -> bool {
        self.with(|f| f.as_ref().map_or(true, |x| x == v))
    }
}

impl<T: Choice + Send + Sync> Local<Option<T>> {
    /// The rows whose `by(row)` passes the filter, cloned: `filter.apply(&all, |s| s.status)`.
    pub fn apply<R: Clone>(self, rows: &[R], by: impl Fn(&R) -> T) -> Vec<R> {
        rows.iter()
            .filter(|r| self.with(|f| f.map_or(true, |x| x == by(r))))
            .cloned()
            .collect()
    }
}

/// Widgets for a cell holding a [`Choice`] enum.
pub trait ChoiceCell<T: Choice>: Cell<T> {
    /// A segmented control over every variant that edits the cell.
    #[track_caller]
    fn options<S: 'static>(self) -> crate::dsl::Choose<T, Act<S>>
    where
        T: Send + Sync,
    {
        crate::options(&self.read()).local(&self)
    }
}
impl<T: Choice, C: Cell<T>> ChoiceCell<T> for C {}
