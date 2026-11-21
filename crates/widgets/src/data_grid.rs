//! A data grid: virtualised rows, sortable and resizable columns, selection, inline editing.

use crate::{
    badge::Tone,
    grid_model::{RowModel, ViewRow},
    text_edit::{apply_key, primary, sanitize, Edit, KeyOutcome},
    ui::Ui,
    util::{self, focus_ring, mix},
    Role,
};
use alloy_ui_core::{
    style::{Align, Style},
    Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout, MouseButton,
    Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::{ops::Range, rc::Rc};

const BAR: f32 = 8.0;
const HEADER_H: f32 = 34.0;
const HANDLE: f32 = 5.0;
const MIN_COL: f32 = 48.0;
const K_DRAG: u64 = 0x_d47a_0001 << 20;

/// What one cell shows. Plain strings convert automatically, so a cell function can just
/// return `format!(..)`.
#[derive(Debug, Clone, PartialEq)]
pub enum GridCell {
    /// Plain text.
    Text(String),
    /// A status: a dot in the tone's colour, then the word. Colour is never the only signal.
    Status(Tone, String),
    /// A thin bar for a value from 0.0 to 1.0, with the percentage beside it.
    Bar(f32),
}

impl GridCell {
    /// The text a cell edits and copies as.
    pub fn text(&self) -> String {
        match self {
            GridCell::Text(s) | GridCell::Status(_, s) => s.clone(),
            GridCell::Bar(v) => format!("{:.0}%", v * 100.0),
        }
    }
}
macro_rules! from_number {
    ($($t:ty),*) => {$(
        impl From<$t> for GridCell {
            fn from(n: $t) -> Self {
                GridCell::Text(n.to_string())
            }
        }
    )*};
}
from_number!(i32, i64, u32, u64, usize, isize);
impl From<f64> for GridCell {
    /// Whole numbers print without a fraction; others with up to two decimals.
    fn from(n: f64) -> Self {
        GridCell::Text(if n.fract() == 0.0 {
            format!("{n:.0}")
        } else {
            format!("{n:.2}")
        })
    }
}
impl GridCell {
    /// A status cell: a dot in `tone`'s colour, then `label`.
    pub fn status(tone: Tone, label: impl Into<String>) -> Self {
        GridCell::Status(tone, label.into())
    }
    /// A thin bar for `value` in `0.0..=1.0`.
    pub fn bar(value: f32) -> Self {
        GridCell::Bar(value)
    }
}
impl From<String> for GridCell {
    fn from(s: String) -> Self {
        GridCell::Text(s)
    }
}
impl From<&str> for GridCell {
    fn from(s: &str) -> Self {
        GridCell::Text(s.into())
    }
}
impl From<&String> for GridCell {
    fn from(s: &String) -> Self {
        GridCell::Text(s.clone())
    }
}

/// One grid column.
#[derive(Debug, Clone)]
pub struct GridColumn {
    /// Header text.
    pub title: String,
    /// Starting width in pixels (the user can drag it).
    pub width: f32,
    /// Horizontal alignment of header and cells. Right-align numbers.
    pub align: Align,
    /// Whether double-click, Enter or typing edits cells in this column.
    pub editable: bool,
    /// Whether clicking the header sorts.
    pub sortable: bool,
    /// Takes the width left over when the columns do not fill the grid.
    pub flex: bool,
    /// Stays at the left edge while the other columns scroll under it.
    pub pinned: bool,
}

impl GridColumn {
    /// A left-aligned, sortable, read-only column `width` pixels wide.
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width: width.max(MIN_COL),
            align: Align::Start,
            editable: false,
            sortable: true,
            flex: false,
            pinned: false,
        }
    }
    /// Pin to the left edge: the column stays in view while the rest scroll horizontally.
    /// Pinned columns draw in the order given, before the unpinned ones, so pin leading
    /// columns.
    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
    /// Right-align (for numbers).
    pub fn right(mut self) -> Self {
        self.align = Align::End;
        self
    }
    /// Let cells in this column be edited in place.
    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }
    /// Header clicks do nothing.
    pub fn unsortable(mut self) -> Self {
        self.sortable = false;
        self
    }
    /// Grow to fill spare width.
    pub fn flex(mut self) -> Self {
        self.flex = true;
        self
    }
}

/// The selected rows of a grid, stored as ranges so selecting 100 000 rows costs nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    /// Sorted, non-overlapping, non-adjacent `start..end` row ranges.
    ranges: Vec<(usize, usize)>,
}

impl Selection {
    /// Nothing selected.
    pub fn none() -> Self {
        Self::default()
    }
    /// Is `row` selected?
    pub fn contains(&self, row: usize) -> bool {
        let i = self.ranges.partition_point(|r| r.1 <= row);
        self.ranges.get(i).is_some_and(|r| r.0 <= row)
    }
    /// Number of selected rows.
    pub fn len(&self) -> usize {
        self.ranges.iter().map(|r| r.1 - r.0).sum()
    }
    /// Nothing selected.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
    /// The selected rows in order.
    pub fn rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.ranges.iter().flat_map(|r| r.0..r.1)
    }
    /// The selected row ranges.
    pub fn ranges(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.ranges.iter().map(|r| r.0..r.1)
    }
    /// The first (topmost) selected row.
    pub fn first(&self) -> Option<usize> {
        self.ranges.first().map(|r| r.0)
    }
    /// Select exactly `rows`.
    pub fn only(rows: Range<usize>) -> Self {
        let mut s = Self::none();
        s.add(rows);
        s
    }
    /// Add `rows` to the selection.
    pub fn add(&mut self, rows: Range<usize>) {
        if rows.start >= rows.end {
            return;
        }
        let (mut a, mut z) = (rows.start, rows.end);
        let lo = self.ranges.partition_point(|r| r.1 < a);
        let hi = self.ranges.partition_point(|r| r.0 <= z);
        if lo < hi {
            a = a.min(self.ranges[lo].0);
            z = z.max(self.ranges[hi - 1].1);
        }
        self.ranges.splice(lo..hi, [(a, z)]);
    }
    /// Remove `rows` from the selection.
    pub fn remove(&mut self, rows: Range<usize>) {
        let (a, z) = (rows.start, rows.end);
        let mut out = Vec::with_capacity(self.ranges.len() + 1);
        for &(s, e) in &self.ranges {
            if e <= a || s >= z {
                out.push((s, e));
                continue;
            }
            if s < a {
                out.push((s, a));
            }
            if e > z {
                out.push((z, e));
            }
        }
        self.ranges = out;
    }
    /// Flip one row.
    pub fn toggle(&mut self, row: usize) {
        if self.contains(row) {
            self.remove(row..row + 1);
        } else {
            self.add(row..row + 1);
        }
    }
}

/// What the pointer is dragging.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Drag {
    #[default]
    None,
    /// Resizing a column: index, pointer x and width at the start.
    Resize(usize, f32, f32),
    /// Extending the row selection from the press.
    Select,
}

/// Per-grid state kept across rebuilds.
#[derive(Default, Clone)]
struct GridState {
    widths: Vec<f32>,
    sel: Selection,
    anchor: usize,
    /// The active cell `(row, col)`.
    active: (usize, usize),
    has_active: bool,
    /// The cell being edited, and its text.
    editing: Option<(usize, usize)>,
    text: String,
    drag: Drag,
    scroll_x: f32,
    /// Marker of the active cell the view last scrolled for.
    revealed: Option<(usize, usize)>,
}

type EditFn<M> = Rc<dyn Fn(usize, usize, String) -> M>;
type CellFn = Rc<dyn Fn(usize, usize) -> GridCell>;

/// A spreadsheet-grade table over `rows` rows that you describe with a cell function. Only
/// rows in view are asked for, so 100 000 rows cost what 20 do, and your data stays yours:
/// the grid never copies it.
///
/// * **Sort**: header click reports `on_sort(column, ascending)`; you reorder, and show the
///   arrow with [`DataGrid::sorted`].
/// * **Resize**: drag a header edge; double-click it to fit the content.
/// * **Select**: click, Shift+click for a range, Ctrl/Cmd+click to toggle, Ctrl+A for all;
///   arrows move the active cell and Ctrl+C copies the selected rows as tab-separated text.
/// * **Edit**: double-click, Enter, F2 or just start typing on an editable column; Enter
///   commits (and moves down), Tab commits (and moves right), Escape cancels.
pub struct DataGrid<M> {
    ui: Ui,
    id: u64,
    columns: Vec<GridColumn>,
    rows: usize,
    cell: CellFn,
    row_h: f32,
    sort: Option<(usize, bool)>,
    on_sort: Option<Rc<dyn Fn(usize, bool) -> M>>,
    on_edit: Option<EditFn<M>>,
    on_select: Option<Rc<dyn Fn(Selection) -> M>>,
    multi: bool,
    /// Present when built from a [`RowModel`]: the view rows (data and group headers).
    view: Option<Rc<Vec<ViewRow>>>,
    on_group: Option<Rc<dyn Fn(String) -> M>>,
}

impl<M: 'static> DataGrid<M> {
    /// A grid of `rows` rows. `cell(row, column)` is called for visible cells only.
    /// `id` must be unique per window.
    pub fn new<C: Into<GridCell>>(
        ui: &Ui,
        id: u64,
        columns: Vec<GridColumn>,
        rows: usize,
        cell: impl Fn(usize, usize) -> C + 'static,
    ) -> Self {
        let t = ui.theme();
        Self {
            ui: ui.clone(),
            id,
            columns,
            rows,
            cell: Rc::new(move |r, c| cell(r, c).into()),
            row_h: (t.typography.body.line_height + 2.0 * t.spacing.sm).round(),
            sort: None,
            on_sort: None,
            on_edit: None,
            on_select: None,
            multi: true,
            view: None,
            on_group: None,
        }
    }
    /// A grid over a [`RowModel`]: rows are the model's filtered, sorted and grouped view.
    /// `cell(source_row, column)` reads your data by *source* row; group headers draw
    /// themselves. [`DataGrid::on_edit`] then reports the *source* row, while selections
    /// stay in view rows (map them with [`RowModel::source`]). Header clicks still call
    /// [`DataGrid::on_sort`]; apply the sort to the model and rebuild it.
    pub fn from_model<C: Into<GridCell>>(
        ui: &Ui,
        id: u64,
        columns: Vec<GridColumn>,
        model: &RowModel,
        cell: impl Fn(usize, usize) -> C + 'static,
    ) -> Self {
        let view = model.view();
        let v = view.clone();
        let mut g = Self::new(ui, id, columns, view.len(), move |r, c| match v.get(r) {
            Some(ViewRow::Data(src)) => cell(*src, c).into(),
            _ => GridCell::Text(String::new()),
        });
        g.view = Some(view);
        g.sort = model.sorted();
        g
    }
    /// A click (or Enter, Left or Right) on a group header reports its key; toggle it on the
    /// [`RowModel`] and rebuild.
    pub fn on_toggle_group(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.on_group = Some(Rc::new(f));
        self
    }
    /// Show an arrow on `column`; `ascending` picks the direction. Clicking the sorted column
    /// again reports the opposite direction.
    pub fn sorted(mut self, sort: Option<(usize, bool)>) -> Self {
        self.sort = sort;
        self
    }
    /// Header clicks report `(column, ascending)`. Selection clears, since row positions change.
    pub fn on_sort(mut self, f: impl Fn(usize, bool) -> M + 'static) -> Self {
        self.on_sort = Some(Rc::new(f));
        self
    }
    /// A committed edit: `(row, column, new text)`. Without it the grid is read-only.
    pub fn on_edit(mut self, f: impl Fn(usize, usize, String) -> M + 'static) -> Self {
        self.on_edit = Some(Rc::new(f));
        self
    }
    /// The selection changed.
    pub fn on_select(mut self, f: impl Fn(Selection) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
    /// One row at a time (default allows ranges and toggling).
    pub fn single(mut self) -> Self {
        self.multi = false;
        self
    }
    /// Row height in pixels.
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_h = h.max(16.0);
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    // ------------------------------------------------------------ geometry

    /// The group header at view row `r`: `(key, count, collapsed)`.
    fn group_at(&self, r: usize) -> Option<(&str, usize, bool)> {
        match self.view.as_ref()?.get(r)? {
            ViewRow::Group {
                key,
                count,
                collapsed,
            } => Some((key.as_str(), *count, *collapsed)),
            ViewRow::Data(_) => None,
        }
    }
    /// The source row for view row `r` (the row itself without a model).
    fn src_row(&self, r: usize) -> usize {
        match self.view.as_ref().and_then(|v| v.get(r)) {
            Some(ViewRow::Data(s)) => *s,
            _ => r,
        }
    }
    /// Total width of the pinned columns.
    fn pinned_w(&self, w: &[f32]) -> f32 {
        (0..self.columns.len())
            .filter(|&i| self.columns[i].pinned)
            .map(|i| w[i])
            .sum()
    }

    fn state(&self) -> GridState {
        let n = self.columns.len();
        self.ui.state(self.id, |s: &mut GridState| {
            if s.widths.len() != n {
                s.widths = self.columns.iter().map(|c| c.width).collect();
            }
            s.clone()
        })
    }
    fn put(&self, s: GridState) {
        self.ui.state(self.id, |g: &mut GridState| *g = s);
    }
    fn body(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x, b.origin.y + HEADER_H),
            Size::new(b.size.width, (b.size.height - HEADER_H).max(0.0)),
        )
    }
    fn content_h(&self) -> f32 {
        self.rows as f32 * self.row_h
    }
    fn max_y(&self, b: &Rectangle) -> f32 {
        (self.content_h() - self.body(b).size.height).max(0.0)
    }
    fn off_y(&self, b: &Rectangle) -> f32 {
        self.ui.scroll_offset(self.id).clamp(0.0, self.max_y(b))
    }
    /// Column widths with spare space handed to flex columns.
    fn widths(&self, b: &Rectangle, s: &GridState) -> Vec<f32> {
        let bar = if self.max_y(b) > 0.0 { BAR + 4.0 } else { 0.0 };
        let avail = (b.size.width - bar).max(0.0);
        let total: f32 = s.widths.iter().sum();
        let flex: Vec<usize> = (0..self.columns.len())
            .filter(|&i| self.columns[i].flex)
            .collect();
        let mut w = s.widths.clone();
        if total < avail && !flex.is_empty() {
            let extra = (avail - total) / flex.len() as f32;
            for i in flex {
                w[i] += extra;
            }
        }
        w
    }
    fn max_x(&self, b: &Rectangle, w: &[f32]) -> f32 {
        let bar = if self.max_y(b) > 0.0 { BAR + 4.0 } else { 0.0 };
        (w.iter().sum::<f32>() - (b.size.width - bar)).max(0.0)
    }
    /// The x of each column's left edge relative to the grid, after horizontal scroll.
    fn lefts(&self, w: &[f32], scroll_x: f32) -> Vec<f32> {
        let pinned = self.pinned_w(w);
        let (mut px, mut sx) = (0.0, pinned - scroll_x);
        w.iter()
            .enumerate()
            .map(|(i, cw)| {
                let x = if self.columns[i].pinned {
                    &mut px
                } else {
                    &mut sx
                };
                let l = *x;
                *x += cw;
                l
            })
            .collect()
    }
    fn row_at(&self, b: &Rectangle, y: f32) -> Option<usize> {
        let r = ((y - self.body(b).origin.y + self.off_y(b)) / self.row_h).floor();
        (r >= 0.0 && (r as usize) < self.rows).then_some(r as usize)
    }
    fn col_at(&self, b: &Rectangle, w: &[f32], sx: f32, x: f32) -> Option<usize> {
        let l = self.lefts(w, sx);
        let pinned = self.pinned_w(w);
        let hit = |i: usize| x - b.origin.x >= l[i] && x - b.origin.x < l[i] + w[i];
        // Pinned columns sit on top, so they win over whatever scrolled under them.
        (0..w.len())
            .find(|&i| self.columns[i].pinned && hit(i))
            .or_else(|| {
                (0..w.len())
                    .find(|&i| !self.columns[i].pinned && hit(i) && x - b.origin.x >= pinned)
            })
    }
    fn pad(&self) -> f32 {
        self.ui.theme().spacing.md
    }
    fn body_size(&self) -> f32 {
        self.ui.theme().typography.body.size
    }
    fn text_w(&self, s: &str, weight: u16) -> f32 {
        self.ui
            .metrics()
            .width_styled(s, self.body_size(), weight, FontFamily::Sans)
    }
    fn thumb(&self, b: &Rectangle) -> Option<Rectangle> {
        let max = self.max_y(b);
        let body = self.body(b);
        if max <= 0.0 {
            return None;
        }
        let h =
            (body.size.height * body.size.height / self.content_h()).clamp(28.0, body.size.height);
        let y = body.origin.y + (body.size.height - h) * self.off_y(b) / max;
        Some(Rectangle::new(
            Point::new(b.origin.x + b.size.width - BAR - 2.0, y),
            Size::new(BAR, h),
        ))
    }
    fn drag_thumb(&self, b: &Rectangle, y: f32, grab: f32) {
        if let Some(th) = self.thumb(b) {
            let body = self.body(b);
            let travel = (body.size.height - th.size.height).max(1.0);
            let top = (y - grab - body.origin.y).clamp(0.0, travel);
            self.ui
                .set_scroll_offset(self.id, top / travel * self.max_y(b));
        }
    }
    /// Fit text into `max` pixels, ending with an ellipsis when it must be cut.
    fn fit(&self, s: &str, max: f32, weight: u16) -> String {
        if max <= 0.0 {
            return String::new();
        }
        if self.text_w(s, weight) <= max {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let t: String = chars[..mid].iter().chain(['\u{2026}'].iter()).collect();
            if self.text_w(&t, weight) <= max {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let mut t: String = chars[..lo].iter().collect();
        t.push('\u{2026}');
        t
    }
    /// The width that fits column `c`'s header and the rows around the view.
    fn autofit(&self, b: &Rectangle, c: usize) -> f32 {
        let first = (self.off_y(b) / self.row_h) as usize;
        let mut w = self.text_w(&self.columns[c].title, 600) + 28.0;
        for r in first.saturating_sub(100)..(first + 200).min(self.rows) {
            let cell = (self.cell)(r, c);
            let cw = match &cell {
                GridCell::Text(s) => self.text_w(s, 400),
                GridCell::Status(_, s) => self.text_w(s, 400) + 16.0,
                GridCell::Bar(_) => 110.0,
            };
            w = w.max(cw + 2.0 * self.pad());
        }
        w.clamp(MIN_COL, 640.0)
    }

    // ------------------------------------------------------------ selection

    fn emit_select(&self, s: &GridState, out: &mut Vec<M>) {
        if let Some(f) = &self.on_select {
            out.push(f(s.sel.clone()));
        }
    }
    fn go_to(&self, s: &mut GridState, row: usize, col: usize, extend: bool, out: &mut Vec<M>) {
        if self.rows == 0 {
            return;
        }
        let row = row.min(self.rows - 1);
        let col = col.min(self.columns.len().saturating_sub(1));
        if !s.has_active {
            s.anchor = row;
        }
        s.active = (row, col);
        s.has_active = true;
        if extend && self.multi {
            let (a, z) = (s.anchor.min(row), s.anchor.max(row));
            s.sel = Selection::only(a..z + 1);
        } else {
            s.anchor = row;
            s.sel = Selection::only(row..row + 1);
        }
        self.emit_select(s, out);
    }
    fn reveal(&self, b: &Rectangle, row: usize) {
        let body = self.body(b);
        let top = row as f32 * self.row_h;
        let off = self.off_y(b);
        let v = if top < off {
            top
        } else if top + self.row_h > off + body.size.height {
            top + self.row_h - body.size.height
        } else {
            return;
        };
        self.ui
            .set_scroll_offset(self.id, v.clamp(0.0, self.max_y(b)));
    }
    fn copy_selection(&self, s: &GridState) {
        let rows: Vec<usize> = if s.sel.is_empty() {
            if s.has_active {
                vec![s.active.0]
            } else {
                return;
            }
        } else {
            s.sel.rows().collect()
        };
        let mut text = String::new();
        for r in rows.into_iter().filter(|&r| self.group_at(r).is_none()) {
            let line: Vec<String> = (0..self.columns.len())
                .map(|c| (self.cell)(r, c).text())
                .collect();
            text.push_str(&line.join("\t"));
            text.push('\n');
        }
        self.ui.clipboard().set(text.trim_end().to_string());
    }

    /// Paste tab-separated clipboard text with its top-left at the active cell, one
    /// `on_edit` per editable cell it lands on.
    fn paste(&self, s: &GridState, out: &mut Vec<M>) {
        let (Some(f), Some(text), true) = (&self.on_edit, self.ui.clipboard().get(), s.has_active)
        else {
            return;
        };
        let (r0, c0) = s.active;
        let mut r = r0;
        for line in text.lines() {
            while r < self.rows && self.group_at(r).is_some() {
                r += 1;
            }
            if r >= self.rows {
                break;
            }
            for (j, val) in line.split('\t').enumerate() {
                let c = c0 + j;
                if c < self.columns.len() && self.columns[c].editable {
                    out.push(f(self.src_row(r), c, val.to_string()));
                }
            }
            r += 1;
        }
    }

    // ------------------------------------------------------------ editing

    fn begin_edit(&self, s: &mut GridState, row: usize, col: usize, seed: Option<&str>) {
        if self.on_edit.is_none() || !self.columns[col].editable || self.group_at(row).is_some() {
            return;
        }
        s.editing = Some((row, col));
        s.text = match seed {
            Some(t) => t.to_string(),
            None => (self.cell)(row, col).text(),
        };
        let n = s.text.chars().count();
        let mut e = Edit {
            caret: n,
            anchor: if seed.is_some() { n } else { 0 },
            ..Edit::default()
        };
        e.blink_epoch = self.ui.now();
        self.ui.set_edit(self.id ^ 0xED17, e);
    }
    fn finish_edit(&self, s: &mut GridState, commit: bool, out: &mut Vec<M>) {
        if let (Some((r, c)), true, Some(f)) = (s.editing.take(), commit, &self.on_edit) {
            if (self.cell)(r, c).text() != s.text {
                out.push(f(self.src_row(r), c, s.text.clone()));
            }
        }
    }
}

impl<M: 'static> Widget<M> for DataGrid<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Fill(1))
    }
    fn measure(&self, _a: Size) -> Size {
        Size::new(
            self.columns.iter().map(|c| c.width).sum::<f32>().min(640.0),
            HEADER_H + self.row_h * self.rows.min(8) as f32,
        )
    }
    fn animating(&self) -> bool {
        self.ui.is_focused(Some(self.id))
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let mut s = self.state();
        let focused = self.ui.is_focused(Some(self.id));
        let w = self.widths(&b, &s);
        let ncols = self.columns.len();
        let consumed = 'ev: {
            match event {
                Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                    self.ui.set_focus(Some(self.id));
                    let mods_clicks = {
                        let mut e = self.ui.edit(self.id);
                        let now = self.ui.now();
                        let near = (cursor.x - e.click_at.0).abs() <= 4.0
                            && (cursor.y - e.click_at.1).abs() <= 4.0;
                        e.clicks = if e.clicks > 0 && near && now.saturating_sub(e.click_ms) < 500 {
                            2
                        } else {
                            1
                        };
                        (e.click_ms, e.click_at) = (now, (cursor.x, cursor.y));
                        let c = e.clicks;
                        self.ui.set_edit(self.id, e);
                        c
                    };
                    let local_x = cursor.x - b.origin.x;
                    let lefts = self.lefts(&w, s.scroll_x);
                    // Header: resize handle first, then sort.
                    if cursor.y < b.origin.y + HEADER_H {
                        self.finish_edit(&mut s, true, out);
                        let edge =
                            (0..ncols).find(|&i| (local_x - (lefts[i] + w[i])).abs() <= HANDLE);
                        if let Some(i) = edge {
                            if mods_clicks == 2 {
                                s.widths[i] = self.autofit(&b, i);
                            } else {
                                s.drag = Drag::Resize(i, cursor.x, w[i]);
                            }
                            break 'ev true;
                        }
                        if let (Some(c), Some(f)) =
                            (self.col_at(&b, &w, s.scroll_x, cursor.x), &self.on_sort)
                        {
                            if self.columns[c].sortable {
                                let asc = !matches!(self.sort, Some((sc, true)) if sc == c);
                                s.sel = Selection::none();
                                s.has_active = false;
                                self.emit_select(&s, out);
                                out.push(f(c, asc));
                            }
                        }
                        break 'ev true;
                    }
                    // Scrollbar.
                    if let Some(th) = self.thumb(&b) {
                        if cursor.x >= b.origin.x + b.size.width - BAR - 6.0 {
                            let grab = if cursor.y >= th.origin.y
                                && cursor.y <= th.origin.y + th.size.height
                            {
                                cursor.y - th.origin.y
                            } else {
                                th.size.height / 2.0
                            };
                            self.ui
                                .set_slot(self.id ^ K_DRAG, Some((grab * 16.0) as i64));
                            self.drag_thumb(&b, cursor.y, grab);
                            break 'ev true;
                        }
                    }
                    let (Some(r), Some(c)) = (
                        self.row_at(&b, cursor.y),
                        self.col_at(&b, &w, s.scroll_x, cursor.x),
                    ) else {
                        break 'ev true;
                    };
                    // A click inside the cell being edited belongs to the editor.
                    if s.editing == Some((r, c)) {
                        break 'ev true;
                    }
                    self.finish_edit(&mut s, true, out);
                    if let Some((key, _, _)) = self.group_at(r) {
                        // A group header: select it and toggle the group.
                        s.active = (r, c);
                        s.has_active = true;
                        s.anchor = r;
                        s.sel = Selection::only(r..r + 1);
                        self.emit_select(&s, out);
                        if let Some(f) = &self.on_group {
                            out.push(f(key.to_string()));
                        }
                        break 'ev true;
                    }
                    let m = self.mods();
                    if mods_clicks == 2 && s.active == (r, c) {
                        self.begin_edit(&mut s, r, c, None);
                        if s.editing.is_some() {
                            break 'ev true;
                        }
                    }
                    s.active = (r, c);
                    if m.shift && self.multi && s.has_active {
                        let (a, z) = (s.anchor.min(r), s.anchor.max(r));
                        s.sel = Selection::only(a..z + 1);
                    } else if (m.ctrl || m.logo) && self.multi {
                        s.sel.toggle(r);
                        s.anchor = r;
                    } else {
                        s.sel = Selection::only(r..r + 1);
                        s.anchor = r;
                        s.drag = Drag::Select;
                    }
                    s.has_active = true;
                    self.emit_select(&s, out);
                    true
                }
                Event::CursorMoved(p) if self.ui.slot(self.id ^ K_DRAG).is_some() => {
                    let g = self.ui.slot(self.id ^ K_DRAG).unwrap_or(0) as f32 / 16.0;
                    self.drag_thumb(&b, p.y, g);
                    return true;
                }
                Event::CursorMoved(p) => match s.drag {
                    Drag::Resize(i, x0, w0) => {
                        s.widths[i] = (w0 + p.x - x0).max(MIN_COL);
                        true
                    }
                    Drag::Select if self.multi => {
                        let body = self.body(&b);
                        if p.y < body.origin.y {
                            self.ui
                                .set_scroll_offset(self.id, (self.off_y(&b) - self.row_h).max(0.0));
                        } else if p.y > body.origin.y + body.size.height {
                            self.ui.set_scroll_offset(
                                self.id,
                                (self.off_y(&b) + self.row_h).min(self.max_y(&b)),
                            );
                        }
                        let y =
                            p.y.clamp(body.origin.y, body.origin.y + body.size.height - 1.0);
                        if let Some(r) = self.row_at(&b, y) {
                            s.active.0 = r;
                            let (a, z) = (s.anchor.min(r), s.anchor.max(r));
                            let sel = Selection::only(a..z + 1);
                            if sel != s.sel {
                                s.sel = sel;
                                self.emit_select(&s, out);
                            }
                        }
                        true
                    }
                    _ => false,
                },
                Event::MouseReleased(MouseButton::Left) => {
                    let had = s.drag != Drag::None || self.ui.slot(self.id ^ K_DRAG).is_some();
                    s.drag = Drag::None;
                    self.ui.set_slot(self.id ^ K_DRAG, None);
                    had
                }
                Event::Scrolled { dx, dy } if b.contains(cursor) => {
                    let (oy, ox) = (self.off_y(&b), s.scroll_x);
                    self.ui
                        .set_scroll_offset(self.id, (oy - dy).clamp(0.0, self.max_y(&b)));
                    s.scroll_x = (ox - dx).clamp(0.0, self.max_x(&b, &w));
                    self.off_y(&b) != oy || s.scroll_x != ox
                }
                Event::TextInput(t) if focused && s.editing.is_some() => {
                    let mut e = self.ui.edit(self.id ^ 0xED17);
                    let clean = sanitize(t, false);
                    let n = s.text.chars().count();
                    e.clamp(n);
                    s.text = e.insert(&s.text, &clean);
                    e.blink_epoch = self.ui.now();
                    self.ui.set_edit(self.id ^ 0xED17, e);
                    true
                }
                Event::TextInput(t) if focused && s.has_active => {
                    // Typing on an editable cell starts an edit that replaces its content.
                    let clean = sanitize(t, false);
                    let (r, c) = s.active;
                    self.begin_edit(&mut s, r, c, Some(&clean));
                    s.editing.is_some()
                }
                Event::KeyPressed(k, m) if focused && s.editing.is_some() => {
                    let (r, c) = s.editing.unwrap_or_default();
                    match k {
                        Key::Enter => {
                            self.finish_edit(&mut s, true, out);
                            self.go_to(&mut s, r + 1, c, false, out);
                            self.reveal(&b, s.active.0);
                        }
                        Key::Escape => self.finish_edit(&mut s, false, out),
                        Key::Tab => {
                            self.finish_edit(&mut s, true, out);
                            let next = if m.shift { c.wrapping_sub(1) } else { c + 1 };
                            if next < ncols {
                                self.go_to(&mut s, r, next, false, out);
                            }
                        }
                        _ => {
                            let mut e = self.ui.edit(self.id ^ 0xED17);
                            e.clamp(s.text.chars().count());
                            match apply_key(
                                &mut e,
                                &s.text,
                                k,
                                m,
                                &*self.ui.clipboard(),
                                true,
                                false,
                            ) {
                                KeyOutcome::Changed(v) => s.text = v,
                                KeyOutcome::Handled | KeyOutcome::Ignored => {}
                            }
                            e.blink_epoch = self.ui.now();
                            self.ui.set_edit(self.id ^ 0xED17, e);
                        }
                    }
                    true
                }
                Event::KeyPressed(k, m) if focused => {
                    let (r, c) = s.active;
                    let page = ((self.body(&b).size.height / self.row_h) as usize).max(2) - 1;
                    let cmd = primary(m);
                    if self.rows == 0 {
                        break 'ev false;
                    }
                    if !s.has_active && !matches!(k, Key::Character(_)) {
                        // The first arrow press lands on the first cell.
                        if matches!(k, Key::Named(n) if matches!(n.as_str(), "ArrowDown" | "ArrowUp" | "ArrowLeft" | "ArrowRight" | "Home" | "End" | "PageDown" | "PageUp"))
                        {
                            self.go_to(&mut s, 0, 0, false, out);
                            self.reveal(&b, 0);
                            break 'ev true;
                        }
                    }
                    match k {
                        Key::Enter if self.group_at(r).is_some() && self.on_group.is_some() => {
                            if let (Some((key, _, _)), Some(f)) = (self.group_at(r), &self.on_group)
                            {
                                out.push(f(key.to_string()));
                            }
                            true
                        }
                        Key::Named(n)
                            if self.group_at(r).is_some()
                                && self.on_group.is_some()
                                && (n == "ArrowLeft" || n == "ArrowRight") =>
                        {
                            if let (Some((key, _, collapsed)), Some(f)) =
                                (self.group_at(r), &self.on_group)
                            {
                                if (n == "ArrowLeft") != collapsed {
                                    out.push(f(key.to_string()));
                                }
                            }
                            true
                        }
                        Key::Enter | Key::Named(_)
                            if matches!(k, Key::Enter)
                                || matches!(k, Key::Named(n) if n == "F2") =>
                        {
                            self.begin_edit(&mut s, r, c, None);
                            s.editing.is_some()
                        }
                        Key::Escape => {
                            if s.sel.is_empty() {
                                self.ui.set_focus(None);
                            } else {
                                s.sel = Selection::none();
                                self.emit_select(&s, out);
                            }
                            true
                        }
                        Key::Character(ch) if cmd && ch.eq_ignore_ascii_case("a") && self.multi => {
                            s.sel = Selection::only(0..self.rows);
                            self.emit_select(&s, out);
                            true
                        }
                        Key::Character(ch) if cmd && ch.eq_ignore_ascii_case("c") => {
                            self.copy_selection(&s);
                            true
                        }
                        Key::Character(ch) if cmd && ch.eq_ignore_ascii_case("v") => {
                            self.paste(&s, out);
                            true
                        }
                        Key::Named(n) => {
                            let ext = m.shift;
                            let moved = match (n.as_str(), cmd) {
                                ("ArrowDown", true) => Some((self.rows - 1, c)),
                                ("ArrowUp", true) => Some((0, c)),
                                ("ArrowRight", true) => Some((r, ncols - 1)),
                                ("ArrowLeft", true) => Some((r, 0)),
                                ("ArrowDown", _) => Some((r + 1, c)),
                                ("ArrowUp", _) => Some((r.saturating_sub(1), c)),
                                ("ArrowRight", _) => Some((r, (c + 1).min(ncols - 1))),
                                ("ArrowLeft", _) => Some((r, c.saturating_sub(1))),
                                ("PageDown", _) => Some((r + page, c)),
                                ("PageUp", _) => Some((r.saturating_sub(page), c)),
                                ("Home", true) => Some((0, 0)),
                                ("End", true) => Some((self.rows - 1, ncols - 1)),
                                ("Home", false) => Some((r, 0)),
                                ("End", false) => Some((r, ncols - 1)),
                                _ => None,
                            };
                            match moved {
                                Some((nr, nc)) => {
                                    self.go_to(&mut s, nr, nc, ext, out);
                                    self.reveal(&b, s.active.0);
                                    true
                                }
                                None => false,
                            }
                        }
                        _ => false,
                    }
                }
                _ => false,
            }
        };
        if consumed {
            let mut e = self.ui.edit(self.id ^ 0xED17);
            e.blink_epoch = self.ui.now();
            self.ui.set_edit(self.id ^ 0xED17, e);
        }
        self.put(s);
        consumed
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let focused = self.ui.is_focused(Some(self.id));
        self.ui.register(self.id);
        self.ui.describe(
            Role::Label,
            &format!("Data grid, {} rows", self.rows),
            &b,
            |n| n.id = Some(self.id),
        );
        let mut s = self.state();
        let w = self.widths(&b, &s);
        s.scroll_x = s.scroll_x.clamp(0.0, self.max_x(&b, &w));
        if focused && s.has_active && s.revealed != Some(s.active) {
            self.reveal(&b, s.active.0);
            // Bring the active column into view too.
            let l = self.lefts(&w, 0.0);
            let (cl, cw) = (l[s.active.1], w[s.active.1]);
            let vw = b.size.width - BAR - 4.0;
            let pw = self.pinned_w(&w);
            if self.columns[s.active.1].pinned {
                // Pinned columns are always in view.
            } else if cl - pw < s.scroll_x {
                s.scroll_x = cl - pw;
            } else if cl + cw > s.scroll_x + vw {
                s.scroll_x = (cl + cw - vw).max(0.0);
            }
            s.revealed = Some(s.active);
        }
        let off = self.off_y(&b);
        let lefts = self.lefts(&w, s.scroll_x);
        let body = self.body(&b);
        let (rh, pad) = (self.row_h, self.pad());
        let size = self.body_size();
        let quad = |r: Rectangle, c: Color, radius: f32| DrawCommand::Quad {
            bounds: r,
            fill: c,
            radius,
            border: None,
            shadow: None,
        };

        if focused {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: p.surface,
            radius: t.radii.medium,
            border: Some(Border {
                color: mix(
                    p.border,
                    p.border_strong,
                    self.ui.ease(
                        Ui::anim_key(&b, 13),
                        (focused || self.ui.hovered(&b)) as u8 as f32,
                    ),
                ),
                width: 1.0,
            }),
            shadow: None,
        });
        out.push(DrawCommand::PushClip(Rectangle::new(
            Point::new(b.origin.x + 1.0, b.origin.y + 1.0),
            Size::new(b.size.width - 2.0, b.size.height - 2.0),
        )));
        let outer = self.ui.clip();
        self.ui.set_clip(Some(b));
        let cursor = self.ui.cursor();
        let right = b.origin.x + b.size.width;

        // Body rows.
        out.push(DrawCommand::PushClip(body));
        let first = (off / rh).floor() as usize;
        let last = (((off + body.size.height) / rh).ceil() as usize).min(self.rows);
        let cell_text = |x: f32, y: f32, s: String, weight: u16, color: Color| DrawCommand::Text {
            family: FontFamily::Sans,
            weight,
            line_height: Some(rh),
            origin: Point::new(x, y),
            content: s,
            size,
            color,
            max_width: None,
        };
        for r in first..last {
            let y = body.origin.y + r as f32 * rh - off;
            let row = Rectangle::new(Point::new(b.origin.x, y), Size::new(b.size.width, rh));
            let selected = s.sel.contains(r);
            let hot = body.contains(cursor) && row.contains(cursor) && s.drag == Drag::None;
            if selected {
                out.push(quad(row, p.selection, 0.0));
            } else if hot {
                out.push(quad(row, p.hover, 0.0));
            }
            if !selected && !s.sel.contains(r + 1) {
                out.push(quad(
                    Rectangle::new(
                        Point::new(b.origin.x, y + rh - 1.0),
                        Size::new(b.size.width, 1.0),
                    ),
                    p.border.with_alpha(0.55),
                    0.0,
                ));
            }
            if let Some((key, count, collapsed)) = self.group_at(r) {
                // A group header: a full-width band with a chevron, the key and the count.
                out.push(quad(row, p.surface_raised, 0.0));
                let (cx, cy) = (b.origin.x + pad + 5.0, y + rh / 2.0);
                let tri = if collapsed {
                    vec![
                        Point::new(cx - 3.0, cy - 5.0),
                        Point::new(cx + 4.0, cy),
                        Point::new(cx - 3.0, cy + 5.0),
                    ]
                } else {
                    vec![
                        Point::new(cx - 5.0, cy - 3.0),
                        Point::new(cx + 5.0, cy - 3.0),
                        Point::new(cx, cy + 4.0),
                    ]
                };
                out.push(DrawCommand::Polygon {
                    points: tri,
                    fill: p.text_secondary,
                });
                let label = self.fit(key, (b.size.width - 2.0 * pad - 90.0).max(40.0), 600);
                let lw = self.text_w(&label, 600);
                out.push(cell_text(b.origin.x + pad + 20.0, y, label, 600, p.text));
                out.push(cell_text(
                    b.origin.x + pad + 20.0 + lw + 10.0,
                    y,
                    format!("{count}"),
                    400,
                    p.text_secondary,
                ));
                if s.active.0 == r && s.has_active && focused {
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(b.origin.x + 0.5, y + 0.5),
                            Size::new(b.size.width - 1.0, rh - 1.0),
                        ),
                        fill: Color::TRANSPARENT,
                        radius: t.radii.small,
                        border: Some(Border {
                            color: p.accent,
                            width: 1.0,
                        }),
                        shadow: None,
                    });
                }
                continue;
            }
            let pw = self.pinned_w(&w);
            for pass in 0..2u8 {
                if pass == 1 && pw <= 0.0 {
                    break;
                }
                if pass == 0 {
                    out.push(DrawCommand::PushClip(Rectangle::new(
                        Point::new(b.origin.x + pw, body.origin.y),
                        Size::new((b.size.width - pw).max(0.0), body.size.height),
                    )));
                } else {
                    // Pinned cells cover whatever scrolled under them.
                    let pr = Rectangle::new(Point::new(b.origin.x, y), Size::new(pw, rh));
                    out.push(quad(pr, p.surface, 0.0));
                    if selected {
                        out.push(quad(pr, p.selection, 0.0));
                    } else if hot {
                        out.push(quad(pr, p.hover, 0.0));
                    }
                }
                for (c, col) in self.columns.iter().enumerate() {
                    if col.pinned != (pass == 1) {
                        continue;
                    }
                    let x = b.origin.x + lefts[c];
                    if x + w[c] < b.origin.x || x > right {
                        continue;
                    }
                    let inner = w[c] - 2.0 * pad;
                    let editing = s.editing == Some((r, c));
                    if !editing {
                        let cell = (self.cell)(r, c);
                        let place = |tw: f32| match col.align {
                            Align::End => x + w[c] - pad - tw,
                            Align::Center => x + (w[c] - tw) / 2.0,
                            _ => x + pad,
                        };
                        match cell {
                            GridCell::Text(txt) => {
                                let txt = self.fit(&txt, inner, 400);
                                let tw = if col.align == Align::Start {
                                    0.0
                                } else {
                                    self.text_w(&txt, 400)
                                };
                                out.push(cell_text(place(tw), y, txt, 400, p.text));
                            }
                            GridCell::Status(tone, txt) => {
                                let ink = tone.color(&t);
                                let txt = self.fit(&txt, inner - 14.0, 400);
                                let tw = self.text_w(&txt, 400) + 14.0;
                                let x0 = place(tw);
                                out.push(quad(
                                    Rectangle::new(
                                        Point::new(x0, y + rh / 2.0 - 3.5),
                                        Size::new(7.0, 7.0),
                                    ),
                                    if tone == Tone::Neutral {
                                        p.border_strong
                                    } else {
                                        ink
                                    },
                                    3.5,
                                ));
                                out.push(cell_text(x0 + 14.0, y, txt, 400, p.text));
                            }
                            GridCell::Bar(v) => {
                                let v = v.clamp(0.0, 1.0);
                                let label = format!("{:.0}%", v * 100.0);
                                let lw = self.text_w(&label, 400);
                                let track = (inner - lw - 10.0).clamp(12.0, 160.0);
                                let x0 = place(track + lw + 10.0);
                                let tr = Rectangle::new(
                                    Point::new(x0, y + rh / 2.0 - 2.5),
                                    Size::new(track, 5.0),
                                );
                                out.push(quad(tr, p.border.with_alpha(0.8), 2.5));
                                if v > 0.0 {
                                    out.push(quad(
                                        Rectangle::new(
                                            tr.origin,
                                            Size::new((track * v).max(5.0), 5.0),
                                        ),
                                        p.accent,
                                        2.5,
                                    ));
                                }
                                out.push(cell_text(
                                    x0 + track + 10.0,
                                    y,
                                    label,
                                    400,
                                    p.text_secondary,
                                ));
                            }
                        }
                    }
                    if s.active == (r, c) && s.has_active && (focused || editing) {
                        out.push(DrawCommand::Quad {
                            bounds: Rectangle::new(
                                Point::new(x + 0.5, y + 0.5),
                                Size::new(w[c] - 1.0, rh - 1.0),
                            ),
                            fill: if editing {
                                p.surface_raised
                            } else {
                                Color::TRANSPARENT
                            },
                            radius: t.radii.small,
                            border: Some(Border {
                                color: p.accent,
                                width: if editing { 1.5 } else { 1.0 },
                            }),
                            shadow: None,
                        });
                    }
                    if editing {
                        let e = self.ui.edit(self.id ^ 0xED17);
                        let x0 = x + pad;
                        let before: String = s.text.chars().take(e.caret).collect();
                        let cx = x0 + self.text_w(&before, 400);
                        if e.has_selection() {
                            let (a, z) = e.range();
                            let pa: String = s.text.chars().take(a).collect();
                            let pz: String = s.text.chars().take(z).collect();
                            let (xa, xz) = (x0 + self.text_w(&pa, 400), x0 + self.text_w(&pz, 400));
                            out.push(quad(
                                Rectangle::new(
                                    Point::new(xa, y + 4.0),
                                    Size::new(xz - xa, rh - 8.0),
                                ),
                                p.selection,
                                t.radii.small,
                            ));
                        }
                        out.push(DrawCommand::PushClip(Rectangle::new(
                            Point::new(x + 2.0, y),
                            Size::new(w[c] - 4.0, rh),
                        )));
                        out.push(cell_text(x0, y, s.text.clone(), 400, p.text));
                        if (self.ui.now().saturating_sub(e.blink_epoch) / 530) % 2 == 0 {
                            out.push(quad(
                                Rectangle::new(Point::new(cx, y + 6.0), Size::new(1.5, rh - 12.0)),
                                p.text,
                                0.0,
                            ));
                        }
                        out.push(DrawCommand::PopClip);
                    }
                }
                if pass == 0 {
                    out.push(DrawCommand::PopClip);
                } else {
                    // A hairline marks where the pinned columns end.
                    out.push(quad(
                        Rectangle::new(Point::new(b.origin.x + pw - 1.0, y), Size::new(1.0, rh)),
                        p.border_strong.with_alpha(0.7),
                        0.0,
                    ));
                }
            }
        }
        out.push(DrawCommand::PopClip);

        // Header, drawn over the rows so scrolled content slides underneath.
        let head = Rectangle::new(b.origin, Size::new(b.size.width, HEADER_H));
        out.push(quad(head, p.surface_raised, 0.0));
        out.push(quad(
            Rectangle::new(
                Point::new(b.origin.x, b.origin.y + HEADER_H - 1.0),
                Size::new(b.size.width, 1.0),
            ),
            p.border,
            0.0,
        ));
        let label = t.typography.label;
        let pw = self.pinned_w(&w);
        for pass in 0..2u8 {
            if pass == 1 && pw <= 0.0 {
                break;
            }
            if pass == 0 {
                out.push(DrawCommand::PushClip(Rectangle::new(
                    Point::new(b.origin.x + pw, b.origin.y),
                    Size::new((b.size.width - pw).max(0.0), HEADER_H),
                )));
            } else {
                out.push(quad(
                    Rectangle::new(b.origin, Size::new(pw, HEADER_H - 1.0)),
                    p.surface_raised,
                    0.0,
                ));
            }
            for (c, col) in self.columns.iter().enumerate() {
                if col.pinned != (pass == 1) {
                    continue;
                }
                let x = b.origin.x + lefts[c];
                if x + w[c] < b.origin.x || x > right {
                    continue;
                }
                let cellr =
                    Rectangle::new(Point::new(x, b.origin.y), Size::new(w[c], HEADER_H - 1.0));
                let sorted = self.sort.filter(|s| s.0 == c).map(|s| s.1);
                if col.sortable
                    && self.on_sort.is_some()
                    && cellr.contains(cursor)
                    && s.drag == Drag::None
                {
                    out.push(quad(cellr, p.hover, 0.0));
                }
                let arrow = sorted.map(|up| if up { "\u{2191}" } else { "\u{2193}" });
                let tw =
                    self.ui
                        .metrics()
                        .width_styled(&col.title, label.size, 600, FontFamily::Sans);
                let aw = if arrow.is_some() { 16.0 } else { 0.0 };
                let inner = w[c] - 2.0 * pad - aw;
                let title = if tw > inner {
                    self.fit(&col.title, inner, 600)
                } else {
                    col.title.clone()
                };
                let tw = tw.min(inner);
                let x0 = match col.align {
                    Align::End => x + w[c] - pad - tw - aw,
                    _ => x + pad,
                };
                let ink = if sorted.is_some() {
                    p.text
                } else {
                    p.text_secondary
                };
                out.push(DrawCommand::Text {
                    family: FontFamily::Sans,
                    weight: 600,
                    line_height: Some(HEADER_H - 1.0),
                    origin: Point::new(x0, b.origin.y),
                    content: title,
                    size: label.size,
                    color: ink,
                    max_width: None,
                });
                if let Some(a) = arrow {
                    out.push(DrawCommand::Text {
                        family: FontFamily::Sans,
                        weight: 600,
                        line_height: Some(HEADER_H - 1.0),
                        origin: Point::new(x0 + tw + 6.0, b.origin.y),
                        content: a.into(),
                        size: label.size,
                        color: p.accent,
                        max_width: None,
                    });
                }
                // Column divider; brighter under the pointer or while dragging.
                let edge = Rectangle::new(
                    Point::new(x + w[c] - 1.0, b.origin.y + 8.0),
                    Size::new(1.0, HEADER_H - 17.0),
                );
                let live = matches!(s.drag, Drag::Resize(i, ..) if i == c)
                    || (head.contains(cursor)
                        && (cursor.x - (x + w[c])).abs() <= HANDLE
                        && s.drag == Drag::None);
                out.push(if live {
                    quad(
                        Rectangle::new(
                            Point::new(x + w[c] - 1.0, b.origin.y),
                            Size::new(2.0, HEADER_H - 1.0),
                        ),
                        p.accent.with_alpha(if matches!(s.drag, Drag::Resize(..)) {
                            0.9
                        } else {
                            0.55
                        }),
                        0.0,
                    )
                } else {
                    quad(edge, p.border, 0.0)
                });
            }
            if pass == 0 {
                out.push(DrawCommand::PopClip);
            } else {
                out.push(quad(
                    Rectangle::new(
                        Point::new(b.origin.x + pw - 1.0, b.origin.y),
                        Size::new(1.0, HEADER_H - 1.0),
                    ),
                    p.border_strong.with_alpha(0.7),
                    0.0,
                ));
            }
        }
        self.ui.set_clip(outer);
        out.push(DrawCommand::PopClip);

        // Scrollbars.
        if let Some(th) = self.thumb(&b) {
            let live = self.ui.slot(self.id ^ K_DRAG).is_some() || self.ui.hovered(&th);
            out.push(quad(
                th,
                if live {
                    p.border_strong
                } else {
                    p.border_strong.with_alpha(0.5)
                },
                BAR / 2.0,
            ));
        }
        let mx = self.max_x(&b, &w);
        if mx > 0.0 {
            let vw = b.size.width - BAR - 4.0;
            let tw = (vw * vw / (vw + mx)).clamp(28.0, vw);
            let x = b.origin.x + (vw - tw) * s.scroll_x / mx;
            out.push(quad(
                Rectangle::new(
                    Point::new(x, b.origin.y + b.size.height - BAR - 2.0),
                    Size::new(tw, BAR),
                ),
                p.border_strong.with_alpha(0.5),
                BAR / 2.0,
            ));
        }
        self.put(s);
    }
}

impl<M: 'static> DataGrid<M> {
    /// Modifier keys are not part of mouse events; the widgets track them from key events
    /// through the window's `Ui`.
    fn mods(&self) -> alloy_ui_core::Modifiers {
        self.ui.modifiers()
    }
}

#[cfg(test)]
mod tests {
    use super::Selection;

    #[test]
    fn selection_ranges_merge_split_and_toggle() {
        let mut s = Selection::only(2..5);
        s.add(5..8);
        assert_eq!(
            s.ranges().collect::<Vec<_>>(),
            std::slice::from_ref(&(2..8)),
            "adjacent ranges merge"
        );
        s.remove(4..6);
        assert_eq!(s.ranges().collect::<Vec<_>>(), [2..4, 6..8]);
        assert!(s.contains(3) && !s.contains(5) && s.contains(7) && !s.contains(8));
        s.toggle(5);
        s.toggle(3);
        assert_eq!(s.rows().collect::<Vec<_>>(), [2, 5, 6, 7]);
        s.add(0..100);
        assert_eq!(s.len(), 100);
        assert_eq!(Selection::only(3..3).len(), 0);
    }
}
