//! Data tables: sorting, filtering and selection from a slice of rows.

use super::*;

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
