//! High-level building blocks: view-local state, data tables, form fields.

use crate::{
    cx::{self, auto_id},
    dsl::{button, caption, text, Act},
    view::{col, Flex, Node},
};
use alloy_ui_core::style::Length;
use alloy_ui_widgets as w;
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------ local state

/// A value that lives with the widget tree, not in your app state: a search
/// box, a sort order, an open panel. Made by [`local`]; clones share the value.
pub struct Local<T>(Arc<Mutex<T>>);

impl<T> Clone for Local<T> {
    fn clone(&self) -> Self {
        Local(self.0.clone())
    }
}

impl<T> Local<T> {
    /// Replace the value.
    pub fn set(&self, v: T) {
        *self.0.lock().unwrap() = v;
    }
    /// Read or change the value in place.
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.0.lock().unwrap())
    }
}
impl<T: Clone> Local<T> {
    /// A copy of the value.
    pub fn get(&self) -> T {
        self.0.lock().unwrap().clone()
    }
}
impl<T: Clone + std::ops::Not<Output = T>> Local<T> {
    /// Flip a `bool`.
    pub fn toggle(&self) {
        self.with(|v| *v = !v.clone());
    }
}

/// View-local state: `let q = local(String::new());` keeps its value between
/// frames (keyed by the call site) with no field in your state and no message.
///
/// Wire a widget to it with `.local(&q)`, or change it in a handler with
/// `.act(move |_| q.set(..))`.
#[track_caller]
pub fn local<T: Send + 'static>(init: T) -> Local<T> {
    Local(cx::slot(auto_id(20), || init))
}

// --------------------------------------------------------------- tables

struct GCol<T> {
    title: String,
    get: Box<dyn Fn(&T) -> String>,
    num: bool,
}

type RowFn<S> = Arc<dyn Fn(&mut S, usize) + Send + Sync>;

/// A sortable, filterable table over a slice. See [`table`].
pub struct Table<'a, T, S> {
    rows: &'a [T],
    cols: Vec<GCol<T>>,
    filter: String,
    selected: Option<usize>,
    on_select: Option<RowFn<S>>,
    actions: Vec<(String, RowFn<S>)>,
    sort: Local<Option<(usize, bool)>>,
    id: u64,
}

/// A table over `rows`. Add columns with `.col(title, |row| cell_text)`; the
/// headers sort, `.filter(query)` narrows, `.on_select(|s, i| ..)` and
/// `.action("Delete", |s, i| ..)` get the index into `rows`.
///
/// ```ignore
/// table(&s.users).col("Name", |u| u.name.clone()).filter(&s.query).on_select(|s, i| s.sel = Some(i))
/// ```
#[track_caller]
pub fn table<T, S>(rows: &[T]) -> Table<'_, T, S> {
    Table {
        rows,
        cols: Vec::new(),
        filter: String::new(),
        selected: None,
        on_select: None,
        actions: Vec::new(),
        sort: local(None),
        id: auto_id(21),
    }
}

impl<'a, T, S: 'static> Table<'a, T, S> {
    /// A text column.
    pub fn col(mut self, title: &str, get: impl Fn(&T) -> String + 'static) -> Self {
        self.cols.push(GCol {
            title: title.into(),
            get: Box::new(get),
            num: false,
        });
        self
    }
    /// A right-aligned column that sorts by numeric value.
    pub fn num(mut self, title: &str, get: impl Fn(&T) -> String + 'static) -> Self {
        self.cols.push(GCol {
            title: title.into(),
            get: Box::new(get),
            num: true,
        });
        self
    }
    /// Only show rows where some cell contains `query` (case-insensitive).
    pub fn filter(mut self, query: &str) -> Self {
        self.filter = query.to_lowercase();
        self
    }
    /// Highlight the row at this index of `rows`.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }
    /// Called with the index into `rows` when a row is picked.
    pub fn on_select(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on_select = Some(Arc::new(f));
        self
    }
    /// A button at the end of every row.
    pub fn action(
        mut self,
        label: &str,
        f: impl Fn(&mut S, usize) + Send + Sync + 'static,
    ) -> Self {
        self.actions.push((label.into(), Arc::new(f)));
        self
    }
}

fn number(s: &str) -> f64 {
    let t: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '.' | '-'))
        .collect();
    t.parse().unwrap_or(f64::NEG_INFINITY)
}

impl<T, S: 'static> From<Table<'_, T, S>> for Node<Act<S>> {
    fn from(t: Table<'_, T, S>) -> Self {
        let sort = t.sort.get();
        let cells: Vec<Vec<String>> = t
            .rows
            .iter()
            .map(|r| t.cols.iter().map(|c| (c.get)(r)).collect())
            .collect();
        let mut order: Vec<usize> = (0..cells.len())
            .filter(|&i| {
                t.filter.is_empty()
                    || cells[i]
                        .iter()
                        .any(|c| c.to_lowercase().contains(&t.filter))
            })
            .collect();
        if let Some((c, desc)) = sort {
            order.sort_by(|&a, &b| {
                let o = if t.cols[c].num {
                    number(&cells[a][c]).total_cmp(&number(&cells[b][c]))
                } else {
                    cells[a][c].to_lowercase().cmp(&cells[b][c].to_lowercase())
                };
                if desc {
                    o.reverse()
                } else {
                    o
                }
            });
        }

        let mut columns: Vec<w::TableColumn> = t
            .cols
            .iter()
            .enumerate()
            .map(|(i, c)| {
                // Sized to the widest cell: `Fill` columns collapse when the table is narrow.
                let chars = cells
                    .iter()
                    .map(|r| r[i].chars().count())
                    .chain([c.title.chars().count() + 3])
                    .max()
                    .unwrap_or(0);
                let width = Length::Fixed((chars as f32 * 6.6 + 16.0).clamp(70.0, 200.0));
                let col = w::TableColumn::new(c.title.clone(), width);
                if c.num {
                    col.right()
                } else {
                    col
                }
            })
            .collect();
        if !t.actions.is_empty() {
            columns.push(w::TableColumn::new(
                "",
                Length::Fixed(76.0 * t.actions.len() as f32),
            ));
        }

        let sort_cell = t.sort.clone();
        let order_sel = order.clone();
        let on_select = t.on_select.clone();
        let mut grid = w::Table::new(&cx::ui(), t.id, columns, move |pos| {
            let (f, i) = (on_select.clone(), order_sel[pos]);
            Act::new(move |s: &mut S| {
                if let Some(f) = &f {
                    f(s, i)
                }
            })
        })
        .on_sort(move |i| {
            let sort = sort_cell.clone();
            Act::new(move |_: &mut S| {
                sort.with(|v| {
                    *v = match *v {
                        Some((s, false)) if s == i => Some((i, true)),
                        Some((s, true)) if s == i => None,
                        _ => Some((i, false)),
                    }
                })
            })
        })
        .selected(t.selected.and_then(|s| order.iter().position(|&i| i == s)));
        if let Some((c, desc)) = sort {
            grid = grid.sorted(c, !desc);
        }
        for &i in &order {
            let mut row: Vec<_> = cells[i]
                .iter()
                .map(|c| Node::<Act<S>>::from(text(c.clone())).0)
                .collect();
            if !t.actions.is_empty() {
                let btns: Vec<Node<Act<S>>> = t
                    .actions
                    .iter()
                    .map(|(label, f)| {
                        let f = f.clone();
                        button(label.clone())
                            .ghost()
                            .on(Act::new(move |s: &mut S| f(s, i)))
                            .into()
                    })
                    .collect();
                row.push(Node::from(col_row(btns)).0);
            }
            grid = grid.row(row);
        }
        Node(grid.into_element())
    }
}

fn col_row<M: 'static>(items: Vec<Node<M>>) -> Flex<M> {
    crate::view::row(items).gap(4.0)
}

// ---------------------------------------------------------------- forms

/// A labelled form field with an optional validation message under it:
/// `field("Email", input(&s.email).bind(..), (!s.email.contains('@')).then_some("Needs an @"))`.
pub fn field<M: 'static>(label: &str, body: impl Into<Node<M>>, error: Option<&str>) -> Flex<M> {
    let f = col([text(label).label().dim().into(), body.into()]).gap(4.0);
    match error {
        Some(e) => f.push(caption(e).danger()),
        None => f,
    }
}

// ------------------------------------------------------------- app shell

/// A sidebar of pages next to the current page's `body`: `shell(&s.page, |s| &mut s.page, body)`.
/// `page` is a [`choices!`](crate::choices) enum.
pub fn shell<T, S>(
    page: &T,
    lens: impl for<'a> Fn(&'a mut S) -> &'a mut T + Send + Sync + 'static,
    body: impl Into<Node<Act<S>>>,
) -> Node<Act<S>>
where
    T: crate::Choice + Send + Sync,
    S: 'static,
{
    use crate::view::View;
    let side: Node<Act<S>> = crate::nav(page).bind(lens).into();
    crate::view::row([side.surface().w(170.0).fill_h().into(), body.into()])
        .gap(0.0)
        .fill()
        .into()
}

/// A Light / Dark switch for the whole window.
pub fn theme_switch<S: 'static>() -> Node<Act<S>> {
    let dark = alloy_ui_theme::relative_luminance(cx::palette().background) < 0.5;
    crate::choose([("Light", false), ("Dark", true)], &dark)
        .on(|d| Act::new(move |_: &mut S| cx::set_dark(d)))
        .into()
}
