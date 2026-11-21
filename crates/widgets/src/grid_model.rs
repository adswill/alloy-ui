//! The row model behind a [`DataGrid`](crate::DataGrid): filtering, sorting and grouping over
//! rows you own, without copying them.
//!
//! The model never sees your data. You give it a cell-text function when you
//! [`RowModel::rebuild`]; it returns an index list (the *view*) that the grid reads. A
//! million rows filter and sort in the time it takes to read the visible column once.

use std::{cmp::Ordering, collections::HashSet, rc::Rc};

/// One row of the grid's view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewRow {
    /// A data row: its index in your source data.
    Data(usize),
    /// A group header covering the data rows that follow it.
    Group {
        /// The shared value of the grouped column.
        key: String,
        /// How many data rows the group holds (collapsed or not).
        count: usize,
        /// Whether its rows are hidden.
        collapsed: bool,
    },
}

/// A filter on one column. Plain text matches anywhere, ignoring case; a leading `=`
/// matches the whole cell, and `>`, `>=`, `<`, `<=` compare numbers.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnFilter {
    /// The column.
    pub column: usize,
    /// The filter text as typed.
    pub text: String,
}

impl ColumnFilter {
    /// Does `cell` pass?
    pub fn matches(&self, cell: &str) -> bool {
        let t = self.text.trim();
        if t.is_empty() {
            return true;
        }
        for (op, f) in [
            (">=", (|a: f64, b: f64| a >= b) as fn(f64, f64) -> bool),
            ("<=", |a, b| a <= b),
            (">", |a, b| a > b),
            ("<", |a, b| a < b),
        ] {
            if let Some(rest) = t.strip_prefix(op) {
                return match (parse_num(cell), rest.trim().parse::<f64>()) {
                    (Some(a), Ok(b)) => f(a, b),
                    _ => false,
                };
            }
        }
        if let Some(rest) = t.strip_prefix('=') {
            return cell.eq_ignore_ascii_case(rest.trim());
        }
        let lower = t.to_lowercase();
        cell.to_lowercase().contains(&lower)
    }
}

fn parse_num(s: &str) -> Option<f64> {
    let t: String = s
        .trim()
        .trim_start_matches(['$', '€', '£'])
        .trim_end_matches('%')
        .chars()
        .filter(|c| *c != ',' && *c != '_')
        .collect();
    t.parse().ok()
}

/// Compare cells: numbers by value, everything else case-insensitively and with digit runs
/// compared as numbers ("item 2" before "item 10").
pub fn compare_cells(a: &str, b: &str) -> Ordering {
    if let (Some(x), Some(y)) = (parse_num(a), parse_num(b)) {
        return x.total_cmp(&y);
    }
    let (mut ia, mut ib) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ia.peek().copied(), ib.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                let mut nb = String::new();
                while let Some(c) = ia.peek().copied().filter(char::is_ascii_digit) {
                    na.push(c);
                    ia.next();
                }
                while let Some(c) = ib.peek().copied().filter(char::is_ascii_digit) {
                    nb.push(c);
                    ib.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                let o = x.to_lowercase().cmp(y.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                ia.next();
                ib.next();
            }
        }
    }
}

/// Filtering, sorting and grouping state over `total` source rows. Keep one in your app
/// state, change it from messages, and call [`RowModel::rebuild`] after a change (or when
/// the data changes).
#[derive(Debug, Clone)]
pub struct RowModel {
    total: usize,
    filters: Vec<ColumnFilter>,
    sort: Option<(usize, bool)>,
    group: Option<usize>,
    collapsed: HashSet<String>,
    view: Rc<Vec<ViewRow>>,
    matched: usize,
}

impl RowModel {
    /// A model over `total` source rows showing all of them in source order.
    pub fn new(total: usize) -> Self {
        Self {
            total,
            filters: Vec::new(),
            sort: None,
            group: None,
            collapsed: HashSet::new(),
            view: Rc::new((0..total).map(ViewRow::Data).collect()),
            matched: total,
        }
    }
    /// The number of source rows.
    pub fn total(&self) -> usize {
        self.total
    }
    /// Set the number of source rows (call [`RowModel::rebuild`] after).
    pub fn set_total(&mut self, n: usize) {
        self.total = n;
    }
    /// How many source rows pass the filters.
    pub fn matched(&self) -> usize {
        self.matched
    }
    /// Filter `column` by `text`; empty text removes the filter.
    pub fn filter(&mut self, column: usize, text: impl Into<String>) {
        let text = text.into();
        self.filters.retain(|f| f.column != column);
        if !text.trim().is_empty() {
            self.filters.push(ColumnFilter { column, text });
        }
    }
    /// The filter text on `column`, if any.
    pub fn filter_text(&self, column: usize) -> &str {
        self.filters
            .iter()
            .find(|f| f.column == column)
            .map_or("", |f| f.text.as_str())
    }
    /// Remove every filter.
    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }
    /// Sort by `(column, ascending)`, or restore source order with `None`.
    pub fn sort(&mut self, sort: Option<(usize, bool)>) {
        self.sort = sort;
    }
    /// The current sort.
    pub fn sorted(&self) -> Option<(usize, bool)> {
        self.sort
    }
    /// Group rows by `column` (a collapsible header per distinct value), or ungroup.
    pub fn group_by(&mut self, column: Option<usize>) {
        if self.group != column {
            self.collapsed.clear();
        }
        self.group = column;
    }
    /// The grouped column.
    pub fn grouped(&self) -> Option<usize> {
        self.group
    }
    /// Collapse or expand the group with this key. Takes effect at the next rebuild.
    pub fn toggle_group(&mut self, key: &str) {
        if !self.collapsed.remove(key) {
            self.collapsed.insert(key.to_string());
        }
    }
    /// Collapse every group (`true`) or expand every group (`false`) given the group keys.
    pub fn set_all_collapsed(&mut self, collapsed: bool) {
        if collapsed {
            for r in self.view.iter() {
                if let ViewRow::Group { key, .. } = r {
                    self.collapsed.insert(key.clone());
                }
            }
        } else {
            self.collapsed.clear();
        }
    }

    /// Recompute the view. `cell(row, column)` returns a cell's text; it is called once per
    /// row for each filtered, sorted or grouped column.
    pub fn rebuild(&mut self, cell: &dyn Fn(usize, usize) -> String) {
        let mut rows: Vec<usize> = (0..self.total)
            .filter(|&r| self.filters.iter().all(|f| f.matches(&cell(r, f.column))))
            .collect();
        self.matched = rows.len();
        if let Some((col, asc)) = self.sort {
            // Read each key once, then sort indices by it (stable, so ties keep source order).
            let mut keyed: Vec<(String, usize)> = rows.iter().map(|&r| (cell(r, col), r)).collect();
            keyed.sort_by(|a, b| {
                let o = compare_cells(&a.0, &b.0);
                if asc {
                    o
                } else {
                    o.reverse()
                }
            });
            rows = keyed.into_iter().map(|k| k.1).collect();
        }
        let mut view = Vec::with_capacity(rows.len() + 8);
        match self.group {
            None => view.extend(rows.into_iter().map(ViewRow::Data)),
            Some(g) => {
                let mut order: Vec<String> = Vec::new();
                let mut members: std::collections::HashMap<String, Vec<usize>> =
                    std::collections::HashMap::new();
                for r in rows {
                    let k = cell(r, g);
                    members
                        .entry(k.clone())
                        .or_insert_with(|| {
                            order.push(k);
                            Vec::new()
                        })
                        .push(r);
                }
                if self.sort.map(|s| s.0) == Some(g) {
                    // Sorted by the grouped column: the first-seen order is already the sort.
                } else {
                    order.sort_by(|a, b| compare_cells(a, b));
                }
                for k in order {
                    let m = members.remove(&k).unwrap_or_default();
                    let collapsed = self.collapsed.contains(&k);
                    view.push(ViewRow::Group {
                        key: k,
                        count: m.len(),
                        collapsed,
                    });
                    if !collapsed {
                        view.extend(m.into_iter().map(ViewRow::Data));
                    }
                }
            }
        }
        self.view = Rc::new(view);
    }
    /// Rows in the view (data rows and group headers).
    pub fn len(&self) -> usize {
        self.view.len()
    }
    /// No rows in the view.
    pub fn is_empty(&self) -> bool {
        self.view.is_empty()
    }
    /// The view row at `i`.
    pub fn get(&self, i: usize) -> Option<&ViewRow> {
        self.view.get(i)
    }
    /// The source row behind view row `i`, or `None` for a group header.
    pub fn source(&self, i: usize) -> Option<usize> {
        match self.view.get(i) {
            Some(ViewRow::Data(r)) => Some(*r),
            _ => None,
        }
    }
    pub(crate) fn view(&self) -> Rc<Vec<ViewRow>> {
        self.view.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Vec<[&'static str; 3]> {
        vec![
            ["Ada", "eng", "120"],
            ["Bo", "ops", "95"],
            ["Cy", "eng", "150"],
            ["Di", "ops", "80"],
            ["Ed", "sales", "70"],
        ]
    }
    fn model() -> (RowModel, Vec<[&'static str; 3]>) {
        (RowModel::new(5), data())
    }

    #[test]
    fn filters_match_text_numbers_and_exact() {
        let (mut m, d) = model();
        let cell = |r: usize, c: usize| d[r][c].to_string();
        m.filter(1, "EN");
        m.rebuild(&cell);
        assert_eq!(
            (0..m.len()).filter_map(|i| m.source(i)).collect::<Vec<_>>(),
            [0, 2]
        );
        m.filter(1, "");
        m.filter(2, ">=95");
        m.rebuild(&cell);
        assert_eq!(m.matched(), 3);
        m.filter(2, "");
        m.filter(0, "=bo");
        m.rebuild(&cell);
        assert_eq!(m.source(0), Some(1));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn sort_is_numeric_stable_and_reversible() {
        let (mut m, d) = model();
        let cell = |r: usize, c: usize| d[r][c].to_string();
        m.sort(Some((2, true)));
        m.rebuild(&cell);
        assert_eq!(
            (0..5).filter_map(|i| m.source(i)).collect::<Vec<_>>(),
            [4, 3, 1, 0, 2]
        );
        m.sort(Some((2, false)));
        m.rebuild(&cell);
        assert_eq!(m.source(0), Some(2));
        m.sort(Some((1, true)));
        m.rebuild(&cell);
        // eng, eng, ops, ops, sales with ties in source order.
        assert_eq!(
            (0..5).filter_map(|i| m.source(i)).collect::<Vec<_>>(),
            [0, 2, 1, 3, 4]
        );
    }

    #[test]
    fn natural_order_puts_item_2_before_item_10() {
        assert_eq!(compare_cells("item 2", "item 10"), Ordering::Less);
        assert_eq!(compare_cells("Item 10", "item 10"), Ordering::Equal);
        assert_eq!(compare_cells("$1,200", "950"), Ordering::Greater);
    }

    #[test]
    fn groups_collapse_and_keep_their_counts() {
        let (mut m, d) = model();
        let cell = |r: usize, c: usize| d[r][c].to_string();
        m.group_by(Some(1));
        m.rebuild(&cell);
        // eng(2) Ada Cy, ops(2) Bo Di, sales(1) Ed
        assert_eq!(m.len(), 3 + 5);
        assert_eq!(
            m.get(0),
            Some(&ViewRow::Group {
                key: "eng".into(),
                count: 2,
                collapsed: false
            })
        );
        m.toggle_group("ops");
        m.rebuild(&cell);
        assert_eq!(m.len(), 3 + 3);
        assert!(matches!(
            m.get(3),
            Some(ViewRow::Group {
                collapsed: true,
                count: 2,
                ..
            })
        ));
        // A filter shrinks counts.
        m.filter(2, ">90");
        m.rebuild(&cell);
        assert!(matches!(m.get(0), Some(ViewRow::Group { count: 2, .. })));
        assert!(matches!(
            m.get(3),
            Some(ViewRow::Group {
                count: 1,
                collapsed: true,
                ..
            })
        ));
    }
}
