//! A simple data table built from the other widgets.

use crate::{Column, Container, List, Row, Rule, Text, TextRole, Ui};
use alloy_ui_core::{
    style::{Align, Padding, Style},
    DrawCommand, DrawList, Element, Event, FontFamily, Layout, MouseButton, Point, Size, Widget,
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
    sort: Option<(usize, bool)>,
    on_sort: Option<Box<dyn Fn(usize) -> M>>,
}

impl<M: Clone + 'static> Table<M> {
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
            sort: None,
            on_sort: None,
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
    /// Make headers clickable. `on_sort` gets the column index; the app
    /// sorts its rows (clicking the sorted column again should flip `ascending`).
    pub fn on_sort(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_sort = Some(Box::new(f));
        self
    }
    /// Show `column` as sorted, with an arrow for its direction.
    pub fn sorted(mut self, column: usize, ascending: bool) -> Self {
        self.sort = Some((column, ascending));
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
            self.columns
                .iter()
                .enumerate()
                .map(|(i, c)| match &self.on_sort {
                    Some(f) => Element::new(SortHeader {
                        ui: self.ui.clone(),
                        title: c.title.clone(),
                        dir: self.sort.filter(|s| s.0 == i).map(|s| s.1),
                        msg: Some(f(i)),
                    }),
                    None => Text::new(&self.ui, c.title.clone())
                        .role(TextRole::Label)
                        .secondary()
                        .into_element(),
                })
                .collect(),
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

/// A clickable column header with an optional sort arrow.
struct SortHeader<M> {
    ui: Ui,
    title: String,
    dir: Option<bool>,
    msg: Option<M>,
}

impl<M: Clone + 'static> Widget<M> for SortHeader<M> {
    fn style(&self) -> Style {
        Style::default()
    }
    fn measure(&self, _available: Size) -> Size {
        let s = self.ui.theme().typography.label;
        let w = self
            .ui
            .metrics()
            .width_styled(&self.title, s.size, s.weight.0, FontFamily::Sans);
        Size::new(w + 14.0, s.line_height)
    }
    fn on_event(&mut self, e: &Event, l: &Layout, cursor: Point, out: &mut Vec<M>) -> bool {
        match (e, &self.msg) {
            (Event::MousePressed(MouseButton::Left), Some(m)) if l.bounds.contains(cursor) => {
                out.push(m.clone());
                true
            }
            _ => false,
        }
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let s = t.typography.label;
        let hot = self.ui.hovered(&l.bounds);
        let color = if self.dir.is_some() || hot {
            t.palette.text
        } else {
            t.palette.text_secondary
        };
        let text = |x: f32, content: String| DrawCommand::Text {
            family: FontFamily::Sans,
            weight: s.weight.0,
            line_height: None,
            origin: Point::new(x, l.bounds.origin.y),
            content,
            size: s.size,
            color,
            max_width: None,
        };
        out.push(text(l.bounds.origin.x, self.title.clone()));
        if let Some(up) = self.dir {
            let w =
                self.ui
                    .metrics()
                    .width_styled(&self.title, s.size, s.weight.0, FontFamily::Sans);
            let arrow = if up { "\u{2191}" } else { "\u{2193}" };
            out.push(text(l.bounds.origin.x + w + 4.0, arrow.into()));
        }
        self.ui
            .describe(crate::Role::Button, &self.title, &l.bounds, |_| {});
    }
}
