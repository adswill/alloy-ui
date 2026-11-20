//! High-level building blocks: view-local state, data tables, form fields.

use crate::{
    app::App,
    bind::Place,
    cx::{self, auto_id},
    dsl::{button, caption, space, text, Act},
    view::View,
    view::{col, Flex, Node},
};
use alloy_ui_core::style::Length;
use alloy_ui_widgets as w;
use std::sync::Arc;

// ------------------------------------------------------------ local state

/// A value that lives with the widget tree, not in your app state: a search
/// box, a sort order, an open panel. Made by [`local`] or [`store`]. It is a small
/// `Copy` handle, so closures can capture it freely with no `.clone()`.
pub struct Local<T>(u64, std::marker::PhantomData<fn() -> T>);

impl<T> Clone for Local<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Local<T> {}

impl<T: Send + 'static> Local<T> {
    /// Replace the value.
    pub fn set(self, v: T) {
        self.with(|x| *x = v);
    }
    /// Read or change the value in place.
    pub fn with<R>(self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut cx::slot_by_id::<T>(self.0).lock().unwrap())
    }
}
impl<T: Clone + Send + 'static> Local<T> {
    /// A copy of the value.
    pub fn get(self) -> T {
        self.with(|v| v.clone())
    }
}
impl<T: Clone + Send + std::ops::Not<Output = T> + 'static> Local<T> {
    /// Flip a `bool`.
    pub fn toggle(self) {
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
    let id = auto_id(20);
    cx::slot(id, || init);
    Local(id, std::marker::PhantomData)
}

/// Like [`local`] but saved: with [`Pages::persist`] the value survives restarts, and
/// it is keyed by the page it is built in plus call order, so it needs no name.
/// `init` runs only when there is nothing saved.
pub fn store<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
    init: impl FnOnce() -> T,
) -> Local<T> {
    Local(cx::durable(init), std::marker::PhantomData)
}

/// A [`store`] with a name, so several pages share it: `store_as("sort", || Sort::Recent)`.
pub fn store_as<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
    name: &str,
    init: impl FnOnce() -> T,
) -> Local<T> {
    Local(
        cx::durable_keyed(format!("={name}"), init),
        std::marker::PhantomData,
    )
}

// --------------------------------------------------------------- tables

#[allow(clippy::type_complexity)]
struct GCol<T> {
    title: String,
    get: Box<dyn Fn(&T) -> String>,
    num: bool,
    key: Option<Box<dyn Fn(&T) -> f64>>,
}

type RowFn<S> = Arc<dyn Fn(&mut S, usize) + Send + Sync>;

/// A sortable, filterable table over a slice. See [`table`].
#[allow(clippy::type_complexity)]
pub struct Table<'a, T, S> {
    rows: &'a [T],
    cols: Vec<GCol<T>>,
    filter: String,
    selected: Option<usize>,
    on_select: Option<RowFn<S>>,
    actions: Vec<(String, RowFn<S>)>,
    sort: Local<Option<(usize, bool)>>,
    detail: Option<Box<dyn Fn(&T) -> Node<Act<S>>>>,
    open: Local<Option<usize>>,
    id: u64,
    keep: Option<Vec<bool>>,
    count: Option<String>,
    empty: Option<String>,
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
        detail: None,
        open: local(None),
        id: auto_id(21),
        keep: None,
        count: None,
        empty: None,
    }
}

impl<'a, T, S: 'static> Table<'a, T, S> {
    /// A text column.
    pub fn col(mut self, title: &str, get: impl Fn(&T) -> String + 'static) -> Self {
        self.cols.push(GCol {
            title: title.into(),
            get: Box::new(get),
            num: false,
            key: None,
        });
        self
    }
    /// A right-aligned column that sorts by numeric value.
    pub fn num(mut self, title: &str, get: impl Fn(&T) -> String + 'static) -> Self {
        self.cols.push(GCol {
            title: title.into(),
            get: Box::new(get),
            num: true,
            key: None,
        });
        self
    }
    /// Sort the column added last by this number instead of by its text or the digits in it:
    /// `.num("Size", |f| human(f.bytes)).key(|f| f.bytes as f64)`.
    pub fn key(mut self, key: impl Fn(&T) -> f64 + 'static) -> Self {
        if let Some(c) = self.cols.last_mut() {
            c.key = Some(Box::new(key));
        }
        self
    }
    /// Only show rows where some cell contains `query` (case-insensitive).
    pub fn filter(mut self, query: &str) -> Self {
        self.filter = query.to_lowercase();
        self
    }
    /// Only show rows for which `keep` holds. Indices passed to handlers still refer to `rows`.
    pub fn keep(mut self, keep: impl Fn(&T) -> bool) -> Self {
        self.keep = Some(self.rows.iter().map(keep).collect());
        self
    }
    /// A caption under the table with the number of rows shown: `.count("items")` reads `12 items`.
    pub fn count(mut self, unit: &str) -> Self {
        self.count = Some(unit.into());
        self
    }
    /// What to show instead of the table when no row is left.
    pub fn empty(mut self, message: &str) -> Self {
        self.empty = Some(message.into());
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
    /// A card to the right of the table showing the picked row (a row stays picked until another
    /// is; indices refer to `rows` as given). Picking is handled for you.
    pub fn detail<N: Into<Node<Act<S>>>>(mut self, f: impl Fn(&T) -> N + 'static) -> Self {
        let open = self.open;
        self.detail = Some(Box::new(move |r| f(r).into()));
        self.selected(open.get())
            .on_select(move |_, i| open.set(Some(i)))
    }
    /// Like [`on_select`](Self::on_select) but called with the row itself: `.on_row(|s, f| s.open(f))`.
    pub fn on_row(self, f: impl Fn(&mut S, &T) + Send + Sync + 'static) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        let rows = Arc::new(self.rows.to_vec());
        self.on_select(move |s, i| f(s, &rows[i]))
    }
    /// Highlight the first row for which `is` holds (none if none does).
    pub fn selected_where(self, is: impl Fn(&T) -> bool) -> Self {
        let at = self.rows.iter().position(is);
        self.selected(at)
    }
    /// Bind the selected row to a [`local`] (`None` = nothing selected).
    pub fn pick(self, cell: &Local<Option<usize>>) -> Self {
        let c = *cell;
        self.selected(c.get()).on_select(move |_, i| c.set(Some(i)))
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
            .filter(|&i| t.keep.as_ref().map_or(true, |k| k[i]))
            .filter(|&i| {
                t.filter.is_empty()
                    || cells[i]
                        .iter()
                        .any(|c| c.to_lowercase().contains(&t.filter))
            })
            .collect();
        if let Some((c, desc)) = sort {
            order.sort_by(|&a, &b| {
                let o = if let Some(key) = &t.cols[c].key {
                    key(&t.rows[a]).total_cmp(&key(&t.rows[b]))
                } else if t.cols[c].num {
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
                // The first column takes what the others (sized to their widest cell) leave.
                let chars = cells
                    .iter()
                    .map(|r| r[i].chars().count())
                    .chain([c.title.chars().count() + 3])
                    .max()
                    .unwrap_or(0);
                let width = if i == 0 {
                    Length::Fill(1)
                } else {
                    Length::Fixed((chars as f32 * 6.6 + 16.0).clamp(60.0, 200.0))
                };
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

        if let (true, Some(msg)) = (order.is_empty(), &t.empty) {
            return caption(msg.clone()).dim().into();
        }
        let shown = order.len();
        let sort_cell = t.sort;
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
            let sort = sort_cell;
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
        let table = Node(grid.into_element());
        let node: Node<Act<S>> = match t
            .detail
            .as_ref()
            .zip(t.open.get())
            .and_then(|(f, i)| t.rows.get(i).map(f))
        {
            Some(card) => crate::row([
                table,
                crate::view::node::<Act<S>, _>(card)
                    .card()
                    .pad(20.0)
                    .w(300.0)
                    .into(),
            ])
            .gap(16.0)
            .fill()
            .into(),
            None => table,
        };
        match &t.count {
            Some(unit) => crate::col![node, caption(format!("{shown} {unit}"))]
                .fill()
                .into(),
            None => node,
        }
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
    crate::view::row([
        side.surface().w(170.0).fill_h().into(),
        sidebar_edge(),
        body.into(),
    ])
    .gap(0.0)
    .fill()
    .into()
}

/// The hairline between a sidebar and its page.
fn sidebar_edge<M: 'static>() -> Node<M> {
    use crate::view::View;
    crate::view::col(Vec::<Node<M>>::new())
        .w(1.0)
        .fill_h()
        .bg(cx::palette().border)
        .into()
}

/// A Light / Dark switch for the whole window.
pub fn theme_switch<S: 'static>() -> Node<Act<S>> {
    let dark = store_as("dark", || false);
    crate::choose([("Light", false), ("Dark", true)], dark.get())
        .on(move |d| {
            let dark = dark;
            Act::new(move |_: &mut S| dark.set(d))
        })
        .into()
}

// ------------------------------------------------------------ pages app

type PageFn = Box<dyn Fn() -> Node<Act<()>>>;

/// A multi-page app with a sidebar and no state struct: every page owns its state
/// in [`store`]s. Made by [`app`].
pub struct Pages {
    title: String,
    pages: Vec<(String, PageFn)>,
    size: (f32, f32),
    save: Option<std::path::PathBuf>,
}

/// Start a multi-page app: `app("Admin").page("People", || ..).page("Settings", || ..).run()`.
pub fn app(title: &str) -> Pages {
    Pages {
        title: title.into(),
        pages: Vec::new(),
        size: (1280.0, 720.0),
        save: None,
    }
}

impl Pages {
    /// A "Settings" page: your controls (a [`section`](crate::section) each, say) under the
    /// built-in heading, above the Light / Dark switch every app wants.
    pub fn settings<N: Into<Node<Act<()>>>>(self, body: impl Fn() -> N + 'static) -> Self {
        self.page("Settings", move || {
            col([
                Node::from(crate::heading("Settings")),
                body().into(),
                crate::section("Theme", theme_switch()).into(),
            ])
            .gap(16.0)
            .pad(24.0)
        })
    }
    /// A "Settings" page with only the Light / Dark switch.
    pub fn theme_page(self) -> Self {
        self.settings(|| col::<Act<()>, Node<Act<()>>>([]))
    }
    /// Add a sidebar entry. `view` builds the page while it is showing.
    pub fn page<N: Into<Node<Act<()>>>>(
        mut self,
        name: &str,
        view: impl Fn() -> N + 'static,
    ) -> Self {
        self.pages
            .push((name.into(), Box::new(move || view().into())));
        self
    }
    /// Window size in logical pixels.
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.size = (w, h);
        self
    }
    /// Keep every [`store`] (and the open page) between runs, under the app's title.
    pub fn persist(mut self) -> Self {
        let path = crate::persist::path(&self.title);
        if let Some(saved) = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            cx::durable_load(saved);
        }
        self.save = Some(path);
        self
    }
    /// Open the window and run until it closes.
    pub fn run(self) {
        let (t, (w, h)) = (self.title.clone(), self.size);
        App::run_sized(self, &t, w, h)
    }
}

impl App for Pages {
    type Msg = Act<()>;

    fn update(&mut self, m: Act<()>) {
        (m.0)(&mut ());
        if let Some(path) = &self.save {
            crate::persist::write(path, &cx::durable_dump());
        }
    }

    fn subscriptions(&self) -> Vec<crate::Sub<Act<()>>> {
        crate::resource::pump_sub().into_iter().collect()
    }

    fn view(&self) -> impl View<Act<()>> {
        let dark = store_as("dark", || false).get();
        if (alloy_ui_theme::relative_luminance(cx::palette().background) < 0.5) != dark {
            cx::set_dark(dark);
        }
        let open = store(|| 0usize);
        let at = open.get().min(self.pages.len().saturating_sub(1));
        let names: Vec<&str> = self.pages.iter().map(|(n, _)| n.as_str()).collect();
        let side: Node<Act<()>> = crate::dsl::Nav::of(
            names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.to_string(), i))
                .collect(),
            at,
        )
        .on(move |i| {
            Act::new({
                let open = open;
                move |_: &mut ()| open.set(i)
            })
        })
        .into();
        let body = cx::scope(names[at], || (self.pages[at].1)());
        crate::view::row([
            side.surface().w(170.0).fill_h().into(),
            sidebar_edge(),
            body,
        ])
        .gap(0.0)
        .fill()
    }
}

// ------------------------------------------------------------- wizard

/// A multi-step flow: a numbered header, the current page, and Back / Next buttons where Next
/// stays disabled until the page says it is valid. Made by [`wizard`].
pub struct Wizard<S> {
    at: usize,
    lens: crate::bind::Lens<S, usize>,
    pages: Vec<(String, bool, Node<Act<S>>)>,
    finish: String,
    on_finish: Option<Act<S>>,
    done: Option<Node<Act<S>>>,
}

/// A step-by-step flow whose position lives in a `usize` field:
///
/// ```ignore
/// wizard(at!(s.step))
///     .page("Account", s.account_ok(), col![field("Name", input(at!(s.name)), None)])
///     .page("Review", s.terms, col!["Check everything", check("I agree", at!(s.terms))])
///     .finish("Create account")
///     .on_finish(|s| s.submit())
///     .done(col![heading("All set")])
/// ```
pub fn wizard<S: 'static>(step: Place<'_, S, usize>) -> Wizard<S> {
    Wizard {
        at: *step.cur,
        lens: step.lens,
        pages: Vec::new(),
        finish: "Finish".into(),
        on_finish: None,
        done: None,
    }
}

impl<S: 'static> Wizard<S> {
    /// Add a page. `valid` gates the Next button.
    pub fn page(mut self, title: &str, valid: bool, body: impl Into<Node<Act<S>>>) -> Self {
        self.pages.push((title.into(), valid, body.into()));
        self
    }
    /// Label of the last page's button (default "Finish").
    pub fn finish(mut self, label: &str) -> Self {
        self.finish = label.into();
        self
    }
    /// Run `f` when the last page's button is pressed, before moving on.
    pub fn on_finish(mut self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.on_finish = Some(Act::new(f));
        self
    }
    /// Shown after the last page's button was pressed.
    pub fn done(mut self, body: impl Into<Node<Act<S>>>) -> Self {
        self.done = Some(body.into());
        self
    }
}

impl<S: 'static> From<Wizard<S>> for Node<Act<S>> {
    fn from(w: Wizard<S>) -> Self {
        let n = w.pages.len();
        let at = w.at.min(n);
        let lens = w.lens;
        if at == n {
            return w.done.unwrap_or_else(|| Node::from(caption("Done")));
        }
        let header = crate::view::row(w.pages.iter().enumerate().map(|(i, (t, ..))| {
            text(format!("{}. {t}", i + 1))
                .accent_if(i == at)
                .dim_if(i > at)
        }))
        .gap(24.0);
        let (_, valid, body) = w.pages.into_iter().nth(at).unwrap();
        let back = lens.clone();
        let next = lens;
        let last = at + 1 == n;
        let on_finish = w.on_finish;
        let go = button(if last { w.finish } else { "Next".into() })
            .primary()
            .enabled(valid)
            .act(move |s| {
                if last {
                    if let Some(f) = &on_finish {
                        (f.0)(s);
                    }
                }
                *next(s) += 1;
            });
        let prev = button("Back")
            .enabled(at > 0)
            .act(move |s| *back(s) = back(s).saturating_sub(1));
        crate::col![header, body, crate::row![prev, space(), go].fill_w()]
            .gap(24.0)
            .into()
    }
}
