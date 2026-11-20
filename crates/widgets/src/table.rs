//! A simple data table built from the other widgets.

use crate::{Column, Container, List, Row, Rule, Text, TextRole, Ui};
use alloy_ui_core::{
    style::{Align, Padding},
    Element,
};
use alloy_ui_layout::Length;

/// One table column: a header, a width and how its cells align.
#[derive(Debug, Clone)]
pub struct TableColumn {
    /// Header text.
    pub title: String,
    /// Column width; `Fill` columns share what the fixed ones leave.
    pub width: Length,
    /// Horizontal alignment of header and cells. Right-align numbers.
    pub align: Align,
}

impl TableColumn {
    /// A left-aligned column.
    pub fn new(title: impl Into<String>, width: Length) -> Self {
        Self {
            title: title.into(),
            width,
            align: Align::Start,
        }
    }
    /// Right-align this column's contents.
    pub fn right(mut self) -> Self {
        self.align = Align::End;
        self
    }
}

/// A header, a rule and a scrolling, selectable list of rows. Each cell is
/// any element, so a cell can hold text, a progress bar or a button.
pub struct Table<M> {
    ui: Ui,
    id: u64,
    columns: Vec<TableColumn>,
    rows: Vec<Vec<Element<M>>>,
    selected: Option<usize>,
    on_select: Box<dyn Fn(usize) -> M>,
    header: Option<Vec<Element<M>>>,
}

impl<M: 'static> Table<M> {
    /// A table over `columns`. `id` must be unique per window; `on_select`
    /// receives the row index when a row is picked (click or Up/Down).
    pub fn new(
        ui: &Ui,
        id: u64,
        columns: Vec<TableColumn>,
        on_select: impl Fn(usize) -> M + 'static,
    ) -> Self {
        Self {
            ui: ui.clone(),
            id,
            columns,
            rows: Vec::new(),
            selected: None,
            on_select: Box::new(on_select),
            header: None,
        }
    }
    /// Add a row; missing cells render empty, extra cells are dropped.
    pub fn row(mut self, cells: Vec<Element<M>>) -> Self {
        self.rows.push(cells);
        self
    }
    /// Highlight a row.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }
    /// Replace the plain text header with custom cells (for example sort buttons),
    /// one per column, laid out with the column widths.
    pub fn header_cells(mut self, cells: Vec<Element<M>>) -> Self {
        self.header = Some(cells);
        self
    }
    fn line(ui: &Ui, columns: &[TableColumn], cells: Vec<Element<M>>) -> Element<M> {
        let mut cells = cells.into_iter();
        let cols = columns
            .iter()
            .map(|c| {
                let cell = cells
                    .next()
                    .unwrap_or_else(|| Text::new(ui, "").into_element());
                Container::new(cell)
                    .width(c.width)
                    .align_x(c.align)
                    .into_element()
            })
            .collect();
        Row::new(cols)
            .spacing(ui.theme().spacing.md)
            .align(Align::Center)
            .width(Length::Fill(1))
            .into_element()
    }

    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        let t = self.ui.theme();
        let header = Self::line(
            &self.ui,
            &self.columns,
            self.header.unwrap_or_else(|| {
                self.columns
                    .iter()
                    .map(|c| {
                        Text::new(&self.ui, c.title.clone())
                            .role(TextRole::Label)
                            .secondary()
                            .into_element()
                    })
                    .collect()
            }),
        );
        let columns = self.columns;
        let ui = self.ui.clone();
        let rows = self
            .rows
            .into_iter()
            .map(|r| Self::line(&ui, &columns, r))
            .collect();
        let list = List::new(&self.ui, self.id, rows, self.selected, self.on_select).into_element();
        // Same horizontal inset as a list row so header and cells line up.
        let header = Container::new(header)
            .padding(Padding::symmetric(t.spacing.md, t.spacing.xs))
            .width(Length::Fill(1))
            .into_element();
        Column::new(vec![
            header,
            Rule::horizontal(&self.ui).into_element(),
            list,
        ])
        .spacing(t.spacing.xs)
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .into_element()
    }
}
