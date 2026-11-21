//! Editors for real work: a data grid, a code editor and a rich-text editor.
//!
//! ```ignore
//! data_grid(&s.rows)                                   // Arc<Vec<Row>>, 100 000 rows is fine
//!     .col("Name", |r| &r.name).editable()
//!     .col("Score", |r| r.score).right()
//!     .col("Status", |r| GridCell::status(r.tone, &r.state))
//!     .edit(|s: &mut App, row, col, text| ..)          // sorting, resizing, selection are built in
//! code_editor(at!(s.src)).lang(Lang::Rust)             // numbers, highlighting, find, auto-indent
//! rich_editor(at!(s.doc)).placeholder("Write here")    // bold, italic, lists, links; doc.to_markdown()
//! ```

use crate::{
    bind::Val,
    cx::{self, auto_id},
    local,
    view::Node,
    Act,
};
use alloy_ui_widgets as w;
use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
    sync::Arc,
};

pub use w::{Block, BlockKind, GridCell, GridColumn, Lang, Mark, Marks, RichText, Run, Selection};

fn set_with<S: 'static, T: Clone + Send + Sync + 'static>(
    f: impl Fn(&mut S, T) + Send + Sync + 'static,
) -> impl Fn(T) -> Act<S> {
    let f = Arc::new(f);
    move |v| {
        let f = f.clone();
        Act::new(move |s: &mut S| f(s, v.clone()))
    }
}

// ---------------------------------------------------------------- code editor

/// A code editor under construction. See [`code_editor`].
pub struct CodeView<M> {
    value: String,
    lang: Lang,
    on: Option<Box<dyn Fn(String) -> M>>,
    id: u64,
    numbers: bool,
    status: bool,
    tab: usize,
}

/// A code editor for `value`: line numbers, syntax colours (`.lang(..)`), auto-indent and
/// bracket pairing, Tab and Shift+Tab to indent blocks, `Ctrl+/` comments, `Ctrl+F` find. Pass
/// `at!(s.src)` to edit a field, or plain text for a read-only viewer. It fills the space it
/// is given and only draws the lines in view, so a 100 000-line file scrolls smoothly.
#[track_caller]
pub fn code_editor<M>(value: impl Val<String, M>) -> CodeView<M> {
    let (value, on) = value.split();
    CodeView {
        value,
        lang: Lang::Plain,
        on,
        id: auto_id(61),
        numbers: true,
        status: true,
        tab: 4,
    }
}
impl<M> CodeView<M> {
    /// The language to colour.
    pub fn lang(mut self, lang: Lang) -> Self {
        self.lang = lang;
        self
    }
    /// Pick the language from a file name or extension: `.file("main.rs")`.
    pub fn file(mut self, name: &str) -> Self {
        self.lang = Lang::from_name(name);
        self
    }
    /// Use `id` for the cursor and scroll state. Give each document its own when one editor
    /// shows different files.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
    /// Hide the line-number gutter.
    pub fn no_numbers(mut self) -> Self {
        self.numbers = false;
        self
    }
    /// Hide the status bar.
    pub fn no_status(mut self) -> Self {
        self.status = false;
        self
    }
    /// Spaces per indent level (default 4).
    pub fn tab_width(mut self, n: usize) -> Self {
        self.tab = n;
        self
    }
    /// Called with the whole text on every edit.
    pub fn on(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
}
impl<S: 'static> CodeView<Act<S>> {
    /// Closure style: `.act(|state, text| ..)`.
    pub fn act(self, f: impl Fn(&mut S, String) + Send + Sync + 'static) -> Self {
        self.on(set_with(f))
    }
}
impl<M: 'static> From<CodeView<M>> for Node<M> {
    fn from(c: CodeView<M>) -> Self {
        let mut e = w::CodeEditor::new(&cx::ui(), c.id, c.value)
            .lang(c.lang)
            .line_numbers(c.numbers)
            .status(c.status)
            .tab_width(c.tab);
        if let Some(f) = c.on {
            e = e.on_input(f);
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- rich text

/// A rich-text editor under construction. See [`rich_editor`].
pub struct RichView<M> {
    doc: RichText,
    on: Option<Box<dyn Fn(RichText) -> M>>,
    link: Option<Box<dyn Fn(String) -> M>>,
    hint: String,
    toolbar: bool,
    id: u64,
}

/// A rich-text editor for `doc`: a toolbar and shortcuts for **bold**, *italic*, underline,
/// `code`, links, headings, bullet and numbered lists and quotes; typing `- `, `1. `, `# ` or
/// `> ` starts one. Pass `at!(s.doc)` to edit a field. `RichText::to_markdown()` and
/// `RichText::from_markdown(..)` save and load it.
#[track_caller]
pub fn rich_editor<M>(doc: impl Val<RichText, M>) -> RichView<M> {
    let (doc, on) = doc.split();
    RichView {
        doc,
        on,
        link: None,
        hint: String::new(),
        toolbar: true,
        id: auto_id(62),
    }
}
impl<M> RichView<M> {
    /// Placeholder shown while empty.
    pub fn placeholder(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
    /// Hide the toolbar (the shortcuts keep working).
    pub fn no_toolbar(mut self) -> Self {
        self.toolbar = false;
        self
    }
    /// Use `id` for the cursor and scroll state.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }
    /// Called with the new document on every edit.
    pub fn on(mut self, f: impl Fn(RichText) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
    /// Called with a link's target when it is opened (Ctrl/Cmd-click).
    pub fn on_link(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.link = Some(Box::new(f));
        self
    }
}
impl<S: 'static> RichView<Act<S>> {
    /// Closure style: `.act(|state, doc| ..)`.
    pub fn act(self, f: impl Fn(&mut S, RichText) + Send + Sync + 'static) -> Self {
        self.on(set_with(f))
    }
    /// Closure style for opened links: `.link(|state, url| ..)`.
    pub fn link(self, f: impl Fn(&mut S, String) + Send + Sync + 'static) -> Self {
        self.on_link(set_with(f))
    }
}
impl<M: 'static> From<RichView<M>> for Node<M> {
    fn from(r: RichView<M>) -> Self {
        let mut e = w::RichEditor::new(&cx::ui(), r.id, &r.doc)
            .placeholder(r.hint)
            .toolbar(r.toolbar);
        if let Some(f) = r.on {
            e = e.on_change(f);
        }
        if let Some(f) = r.link {
            e = e.on_link(f);
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- data grid

/// The rows a grid shows. An `Arc<Vec<T>>` is shared, not copied, so a 100 000-row grid costs
/// nothing to rebuild; a `&Vec<T>` is cloned (fine for small tables).
pub struct GridRows<T>(Arc<Vec<T>>);
impl<T> From<Arc<Vec<T>>> for GridRows<T> {
    fn from(a: Arc<Vec<T>>) -> Self {
        GridRows(a)
    }
}
impl<T> From<&Arc<Vec<T>>> for GridRows<T> {
    fn from(a: &Arc<Vec<T>>) -> Self {
        GridRows(a.clone())
    }
}
impl<T: Clone> From<&Vec<T>> for GridRows<T> {
    fn from(v: &Vec<T>) -> Self {
        GridRows(Arc::new(v.clone()))
    }
}
impl<T: Clone> From<&[T]> for GridRows<T> {
    fn from(v: &[T]) -> Self {
        GridRows(Arc::new(v.to_vec()))
    }
}

type EditCb<S> = Arc<dyn Fn(&mut S, usize, usize, String) + Send + Sync>;
type SelectCb<S> = Arc<dyn Fn(&mut S, Vec<usize>) + Send + Sync>;
type Getter<T> = Arc<dyn Fn(&T) -> GridCell + Send + Sync>;

struct Col<T> {
    spec: GridColumn,
    get: Getter<T>,
    auto_width: bool,
}

/// A data grid under construction. See [`data_grid`].
pub struct Grid<T, S> {
    rows: Arc<Vec<T>>,
    cols: Vec<Col<T>>,
    edit: Option<EditCb<S>>,
    select: Option<SelectCb<S>>,
    row_h: Option<f32>,
    id: u64,
}

/// A sortable, resizable, selectable, editable grid over `rows`. Describe each column with a
/// getter; sorting (header click, numbers sort as numbers), column resizing (drag a header
/// edge, double-click to fit), row selection (click, Shift/Ctrl-click, Ctrl+A, arrows, Ctrl+C
/// copies) and virtualisation are built in. Only rows in view are read.
///
/// `rows` is usually an `Arc<Vec<Row>>` in your state; edit it with `Arc::make_mut`.
#[track_caller]
pub fn data_grid<T: Send + Sync + 'static, S: 'static>(rows: impl Into<GridRows<T>>) -> Grid<T, S> {
    Grid {
        rows: rows.into().0,
        cols: Vec::new(),
        edit: None,
        select: None,
        row_h: None,
        id: auto_id(63),
    }
}
impl<T: Send + Sync + 'static, S: 'static> Grid<T, S> {
    /// Add a column titled `title` showing `get(row)`: a string, a number or a [`GridCell`].
    /// Its width fits the content; change it with `.width(px)`.
    pub fn col<C: Into<GridCell>>(
        mut self,
        title: &str,
        get: impl Fn(&T) -> C + Send + Sync + 'static,
    ) -> Self {
        self.cols.push(Col {
            spec: GridColumn::new(title, 80.0),
            get: Arc::new(move |r| get(r).into()),
            auto_width: true,
        });
        self
    }
    fn last(&mut self) -> &mut Col<T> {
        self.cols.last_mut().expect("call .col(..) first")
    }
    /// Set the last column's starting width in pixels.
    pub fn width(mut self, px: f32) -> Self {
        let c = self.last();
        c.spec.width = px;
        c.auto_width = false;
        self
    }
    /// Right-align the last column (for numbers).
    pub fn right(mut self) -> Self {
        let s = std::mem::replace(&mut self.last().spec, GridColumn::new("", 0.0));
        self.last().spec = s.right();
        self
    }
    /// Let the last column's cells be edited in place; report edits with [`Grid::edit`].
    pub fn editable(mut self) -> Self {
        let s = std::mem::replace(&mut self.last().spec, GridColumn::new("", 0.0));
        self.last().spec = s.editable();
        self
    }
    /// Let the last column take the width the others leave.
    pub fn flex(mut self) -> Self {
        let s = std::mem::replace(&mut self.last().spec, GridColumn::new("", 0.0));
        self.last().spec = s.flex();
        self
    }
    /// A committed cell edit: `(state, row, column, text)`. `row` indexes your `rows`, not the
    /// sorted view.
    pub fn edit(
        mut self,
        f: impl Fn(&mut S, usize, usize, String) + Send + Sync + 'static,
    ) -> Self {
        self.edit = Some(Arc::new(f));
        self
    }
    /// The selected rows changed: `(state, row indexes into your rows, ascending)`.
    pub fn on_select(mut self, f: impl Fn(&mut S, Vec<usize>) + Send + Sync + 'static) -> Self {
        self.select = Some(Arc::new(f));
        self
    }
    /// Row height in pixels.
    pub fn row_height(mut self, px: f32) -> Self {
        self.row_h = Some(px);
        self
    }
}

/// Numbers sort as numbers (`$1,200`, `45%`, `3.5`); everything else sorts case-insensitively.
fn cmp_cells(a: &str, b: &str) -> Ordering {
    fn num(s: &str) -> Option<f64> {
        let t: String = s
            .chars()
            .filter(|c| !matches!(c, ',' | '$' | '%' | ' ' | '\u{20ac}' | '\u{a3}'))
            .collect();
        t.parse().ok()
    }
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

#[derive(Default)]
struct Order {
    key: u64,
    perm: Arc<Vec<usize>>,
}

impl<T: Send + Sync + 'static, S: 'static> From<Grid<T, S>> for Node<Act<S>> {
    fn from(g: Grid<T, S>) -> Self {
        let sort = local(None::<(usize, bool)>);
        let order = local(Order::default());
        let ui = cx::ui();
        let n = g.rows.len();
        let cur = sort.get();

        // Sort a permutation of row indexes; reuse it while the sort and the data look the same.
        let perm: Option<Arc<Vec<usize>>> = cur.and_then(|(c, asc)| {
            let col = g.cols.get(c)?;
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (c, asc, n).hash(&mut h);
            for i in (0..n).step_by((n / 61).max(1)) {
                (col.get)(&g.rows[i]).text().hash(&mut h);
            }
            let key = h.finish();
            let hit = order.with(|o| (o.key == key).then(|| o.perm.clone()));
            Some(hit.unwrap_or_else(|| {
                let keys: Vec<String> = g.rows.iter().map(|r| (col.get)(r).text()).collect();
                let mut p: Vec<usize> = (0..n).collect();
                p.sort_by(|&a, &b| {
                    let o = cmp_cells(&keys[a], &keys[b]);
                    if asc {
                        o
                    } else {
                        o.reverse()
                    }
                });
                let p = Arc::new(p);
                order.with(|o| {
                    *o = Order {
                        key,
                        perm: p.clone(),
                    }
                });
                p
            }))
        });

        // Starting widths that fit the content (first rows), unless given.
        let metrics = ui.metrics();
        let body = ui.theme().typography.body.size;
        let specs: Vec<GridColumn> = g
            .cols
            .iter()
            .map(|c| {
                let mut s = c.spec.clone();
                if c.auto_width {
                    let head =
                        metrics.width_styled(&s.title, body, 600, alloy_ui_core::FontFamily::Sans)
                            + 40.0;
                    let content = g
                        .rows
                        .iter()
                        .take(40)
                        .map(|r| match (c.get)(r) {
                            GridCell::Bar(_) => 130.0,
                            GridCell::Status(_, t) => metrics.width(&t, body) + 44.0,
                            GridCell::Text(t) => metrics.width(&t, body) + 28.0,
                        })
                        .fold(0.0f32, f32::max);
                    s.width = head.max(content).clamp(80.0, 320.0);
                }
                s
            })
            .collect();

        let rows = g.rows.clone();
        let getters: Vec<Getter<T>> = g.cols.iter().map(|c| c.get.clone()).collect();
        let map = perm.clone();
        let at = move |r: usize| map.as_ref().map_or(r, |p| p[r]);
        let at_cell = at.clone();
        let mut e = w::DataGrid::new(&ui, g.id, specs, n, move |r, c| {
            (getters[c])(&rows[at_cell(r)])
        })
        .sorted(cur)
        .on_sort(move |c, asc| Act::new(move |_: &mut S| sort.set(Some((c, asc)))));
        if let Some(h) = g.row_h {
            e = e.row_height(h);
        }
        if let Some(f) = g.edit {
            let at = at.clone();
            e = e.on_edit(move |r, c, text| {
                let (f, src) = (f.clone(), at(r));
                Act::new(move |s: &mut S| f(s, src, c, text.clone()))
            });
        }
        if let Some(f) = g.select {
            let at = at.clone();
            e = e.on_select(move |sel| {
                let f = f.clone();
                let mut rows: Vec<usize> = sel.rows().map(&at).collect();
                rows.sort_unstable();
                Act::new(move |s: &mut S| f(s, rows.clone()))
            });
        }
        Node(e.into_element())
    }
}

#[cfg(test)]
mod tests {
    use super::cmp_cells;
    use std::cmp::Ordering::*;

    #[test]
    fn numbers_sort_as_numbers_and_words_ignore_case() {
        assert_eq!(cmp_cells("9", "10"), Less);
        assert_eq!(cmp_cells("$1,200", "$950"), Greater);
        assert_eq!(cmp_cells("45%", "5%"), Greater);
        assert_eq!(cmp_cells("apple", "Banana"), Less);
        assert_eq!(
            cmp_cells("x2", "x10"),
            Greater,
            "mixed text is plain string order"
        );
    }
}
