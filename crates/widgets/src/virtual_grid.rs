//! Virtualised grid: only the cells in view exist, so 100 000 tiles cost the same as 20.

use crate::{ui::Ui, util::focus_ring};
use alloy_ui_core::{
    style::{Axis, Style},
    DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

const BAR: f32 = 8.0;
const BAR_ZONE: f32 = 14.0;
const K_DRAG: u64 = 0x7117_0002 << 20;
/// Offsets below this (in pixels) are smooth-scrolled.
const GLIDE_LIMIT: f64 = 4_000_000.0;

/// How many columns the grid has.
#[derive(Clone, Copy)]
enum Cols {
    Fixed(usize),
    Fit(f32),
}

/// How tall a cell is.
#[derive(Clone, Copy)]
enum CellH {
    Px(f32),
    /// Width divided by this ratio (1.0 is square).
    Aspect(f32),
}

/// The resolved geometry for one laid-out width.
#[derive(Clone, Copy)]
struct Geo {
    cols: usize,
    cell_w: f32,
    cell_h: f32,
    gap: f32,
}

type Build<M> = Rc<dyn Fn(usize) -> Element<M>>;

/// A grid of `count` cells in `columns` columns, built on demand by `cell(index)`. Only the
/// rows in view are built and drawn, per frame, so the cost follows the window, not the data.
/// Selection (click, arrow keys in two dimensions, Home/End, PageUp/PageDown) is reported
/// through `on_select`; the scrollbar thumb drags, and the wheel scrolls.
pub struct VirtualGrid<M> {
    ui: Ui,
    id: u64,
    count: usize,
    cols: Cols,
    cell_h: CellH,
    gap: f32,
    selected: Option<usize>,
    build: Build<M>,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
}

impl<M: 'static> VirtualGrid<M> {
    /// `count` cells in `columns` columns, cell `i` built by `cell(i)`. `id` must be unique per
    /// window.
    pub fn new(
        ui: &Ui,
        id: u64,
        count: usize,
        columns: usize,
        cell: impl Fn(usize) -> Element<M> + 'static,
    ) -> Self {
        Self {
            ui: ui.clone(),
            id,
            count,
            cols: Cols::Fixed(columns.max(1)),
            cell_h: CellH::Px(120.0),
            gap: ui.theme().spacing.md,
            selected: None,
            build: Rc::new(cell),
            on_select: None,
        }
    }
    /// As many columns as fit, each at least `min_width` pixels wide (cells then stretch to
    /// fill the row). Replaces the fixed column count.
    pub fn fit(mut self, min_width: f32) -> Self {
        self.cols = Cols::Fit(min_width.max(8.0));
        self
    }
    /// Cell height in pixels (the default is 120).
    pub fn cell_height(mut self, h: f32) -> Self {
        self.cell_h = CellH::Px(h.max(8.0));
        self
    }
    /// Cell height as `width / ratio`: `1.0` is square, `1.5` is a landscape tile.
    pub fn aspect(mut self, ratio: f32) -> Self {
        self.cell_h = CellH::Aspect(ratio.max(0.1));
        self
    }
    /// Space between cells, in pixels.
    pub fn gap(mut self, px: f32) -> Self {
        self.gap = px.max(0.0);
        self
    }
    /// The highlighted cell.
    pub fn selected(mut self, i: Option<usize>) -> Self {
        self.selected = i;
        self
    }
    /// Called with the index of the cell the user picked.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn geo(&self, b: &Rectangle) -> Geo {
        let usable = (b.size.width - BAR - 4.0).max(1.0);
        let gap = self.gap;
        let cols = match self.cols {
            Cols::Fixed(n) => n,
            Cols::Fit(min) => (((usable + gap) / (min + gap)).floor() as usize).max(1),
        };
        let cell_w = ((usable - gap * (cols as f32 - 1.0)) / cols as f32).max(1.0);
        let cell_h = match self.cell_h {
            CellH::Px(h) => h,
            CellH::Aspect(r) => (cell_w / r).max(8.0),
        };
        Geo {
            cols,
            cell_w,
            cell_h,
            gap,
        }
    }
    fn rows(&self, g: &Geo) -> usize {
        self.count.div_ceil(g.cols)
    }
    // Offsets are f64 for the same reason as in the list: a million tiles outgrow an f32.
    fn content_h(&self, g: &Geo) -> f64 {
        let rows = self.rows(g);
        if rows == 0 {
            0.0
        } else {
            rows as f64 * (g.cell_h + g.gap) as f64 - g.gap as f64
        }
    }
    fn max_off(&self, b: &Rectangle) -> f64 {
        (self.content_h(&self.geo(b)) - b.size.height as f64).max(0.0)
    }
    fn target(&self, b: &Rectangle) -> f64 {
        self.ui
            .scroll_offset_f64(self.id)
            .clamp(0.0, self.max_off(b))
    }
    fn offset(&self, b: &Rectangle) -> f64 {
        let t = self.target(b);
        if t < GLIDE_LIMIT {
            (self.ui.scroll_shown(self.id, self.max_off(b) as f32) as f64)
                .clamp(0.0, self.max_off(b))
        } else {
            t
        }
    }
    fn set_offset(&self, b: &Rectangle, off: f64) {
        self.ui
            .set_scroll_offset_f64(self.id, off.clamp(0.0, self.max_off(b)));
    }
    /// Rows `first..last` that touch the viewport.
    fn row_range(&self, b: &Rectangle) -> std::ops::Range<usize> {
        let g = self.geo(b);
        let rows = self.rows(&g);
        if rows == 0 {
            return 0..0;
        }
        let step = (g.cell_h + g.gap) as f64;
        let off = self.offset(b);
        let first = ((off / step).floor() as usize).min(rows - 1);
        let last = (((off + b.size.height as f64) / step).ceil() as usize).clamp(first + 1, rows);
        first..last
    }
    fn cell_rect(&self, b: &Rectangle, g: &Geo, i: usize) -> Rectangle {
        let (r, c) = (i / g.cols, i % g.cols);
        Rectangle::new(
            Point::new(
                b.origin.x + c as f32 * (g.cell_w + g.gap),
                b.origin.y + (r as f64 * (g.cell_h + g.gap) as f64 - self.offset(b)) as f32,
            ),
            Size::new(g.cell_w, g.cell_h),
        )
    }
    /// The cell under `cursor`, if any.
    fn cell_at(&self, b: &Rectangle, cursor: Point) -> Option<usize> {
        let g = self.geo(b);
        let x = cursor.x - b.origin.x;
        let y = (cursor.y - b.origin.y) as f64 + self.offset(b);
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let (sx, sy) = (g.cell_w + g.gap, (g.cell_h + g.gap) as f64);
        let (c, r) = ((x / sx) as usize, (y / sy) as usize);
        let in_cell = x - c as f32 * sx < g.cell_w && y - r as f64 * sy < g.cell_h as f64;
        let i = r * g.cols + c;
        (in_cell && c < g.cols && i < self.count).then_some(i)
    }
    fn laid(&self, b: &Rectangle, g: &Geo, i: usize) -> (Element<M>, Layout) {
        let r = self.cell_rect(b, g, i);
        let e = (self.build)(i);
        let l = e.0.arrange(r);
        (e, l)
    }
    fn thumb(&self, b: &Rectangle) -> Option<Rectangle> {
        let content = self.content_h(&self.geo(b));
        let max = self.max_off(b);
        if max <= 0.0 || b.size.height <= 0.0 {
            return None;
        }
        let h = (b.size.height as f64 * b.size.height as f64 / content)
            .max(28.0)
            .min(b.size.height as f64) as f32;
        let y = b.origin.y + ((b.size.height - h) as f64 * self.offset(b) / max) as f32;
        Some(Rectangle::new(
            Point::new(b.origin.x + b.size.width - BAR - 2.0, y),
            Size::new(BAR, h),
        ))
    }
    fn drag_to(&self, b: &Rectangle, y: f32, grab: f32) {
        if let Some(th) = self.thumb(b) {
            let travel = (b.size.height - th.size.height).max(1.0);
            let top = (y - grab - b.origin.y).clamp(0.0, travel);
            self.ui.scroll_jump_f64(
                self.id,
                top as f64 / travel as f64 * self.max_off(b),
                self.max_off(b),
            );
        }
    }
    fn reveal(&self, b: &Rectangle, i: usize) {
        let g = self.geo(b);
        let top = (i / g.cols) as f64 * (g.cell_h + g.gap) as f64;
        let off = self.target(b);
        let (ch, vh) = (g.cell_h as f64, b.size.height as f64);
        if top < off {
            self.set_offset(b, top);
        } else if top + ch > off + vh {
            self.set_offset(b, top + ch - vh);
        }
    }
}

impl<M: 'static> Widget<M> for VirtualGrid<M> {
    fn style(&self) -> Style {
        Style {
            axis: Axis::Vertical,
            width: Length::Fill(1),
            height: Length::Fill(1),
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        let h = match self.cell_h {
            CellH::Px(h) => h,
            CellH::Aspect(_) => 120.0,
        };
        Size::new(240.0, h)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let inside = b.contains(cursor);
        match event {
            Event::Scrolled { dy, .. } if inside => {
                let before = self.target(&b);
                self.set_offset(&b, before - *dy as f64);
                return self.target(&b) != before;
            }
            Event::MousePressed(MouseButton::Left) if inside => {
                self.ui.set_focus(Some(self.id));
                let in_bar = cursor.x >= b.origin.x + b.size.width - BAR_ZONE;
                if let (true, Some(th)) = (in_bar, self.thumb(&b)) {
                    let grab = if th.contains(Point::new(th.origin.x + 1.0, cursor.y)) {
                        cursor.y - th.origin.y
                    } else {
                        th.size.height / 2.0
                    };
                    self.ui
                        .set_slot(self.id ^ K_DRAG, Some((grab * 16.0) as i64));
                    self.drag_to(&b, cursor.y, grab);
                    return true;
                }
                if let Some(i) = self.cell_at(&b, cursor) {
                    let g = self.geo(&b);
                    let (mut cell, l) = self.laid(&b, &g, i);
                    if cell.0.on_event(event, &l, cursor, out) {
                        return true;
                    }
                    if let Some(f) = &self.on_select {
                        out.push(f(i));
                    }
                }
                return true;
            }
            Event::CursorMoved(_) => {
                if let Some(grab) = self.ui.slot(self.id ^ K_DRAG) {
                    self.drag_to(&b, cursor.y, grab as f32 / 16.0);
                    return true;
                }
            }
            Event::MouseReleased(MouseButton::Left) => {
                if self.ui.slot(self.id ^ K_DRAG).is_some() {
                    self.ui.set_slot(self.id ^ K_DRAG, None);
                    return true;
                }
                // Let controls inside a cell finish their press.
                let g = self.geo(&b);
                for r in self.row_range(&b) {
                    for i in r * g.cols..((r + 1) * g.cols).min(self.count) {
                        let (mut cell, l) = self.laid(&b, &g, i);
                        if cell.0.on_event(event, &l, cursor, out) {
                            return true;
                        }
                    }
                }
            }
            Event::KeyPressed(Key::Named(k), _)
                if self.ui.is_focused(Some(self.id)) && self.count > 0 =>
            {
                let g = self.geo(&b);
                let last = self.count - 1;
                let page_rows = ((b.size.height / (g.cell_h + g.gap)) as usize)
                    .saturating_sub(1)
                    .max(1);
                let page = page_rows * g.cols;
                let next = match (k.as_str(), self.selected) {
                    ("Home", _) | ("ArrowRight" | "ArrowDown", None) => Some(0),
                    ("End", _) | ("ArrowLeft" | "ArrowUp", None) => Some(last),
                    ("ArrowRight", Some(i)) => Some((i + 1).min(last)),
                    ("ArrowLeft", Some(i)) => Some(i.saturating_sub(1)),
                    // Down from the last row stays put rather than jumping sideways.
                    ("ArrowDown", Some(i)) if i + g.cols <= last => Some(i + g.cols),
                    ("ArrowDown", Some(i)) => Some(i),
                    ("ArrowUp", Some(i)) => Some(if i >= g.cols { i - g.cols } else { i }),
                    ("PageDown", i) => Some((i.unwrap_or(0) + page).min(last)),
                    ("PageUp", i) => Some(i.unwrap_or(0).saturating_sub(page)),
                    _ => None,
                };
                if let Some(i) = next {
                    self.reveal(&b, i);
                    if let Some(f) = &self.on_select {
                        out.push(f(i));
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let t = self.ui.theme();
        let p = &t.palette;
        self.ui.register(self.id);
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(b, t.radii.small, p.focus_ring));
        }
        out.push(DrawCommand::PushClip(b));
        let outer = self.ui.clip();
        self.ui.set_clip(Some(b));
        let cursor = self.ui.cursor();
        let g = self.geo(&b);
        for r in self.row_range(&b) {
            for i in r * g.cols..((r + 1) * g.cols).min(self.count) {
                let rect = self.cell_rect(&b, &g, i);
                let hot = b.contains(cursor) && rect.contains(cursor);
                if self.selected == Some(i) || hot {
                    out.push(DrawCommand::Quad {
                        bounds: rect,
                        fill: if self.selected == Some(i) {
                            p.selection
                        } else {
                            p.hover
                        },
                        radius: t.radii.small,
                        border: None,
                        shadow: None,
                    });
                }
                let (cell, l) = self.laid(&b, &g, i);
                cell.0.draw(&l, out);
            }
        }
        self.ui.set_clip(outer);
        out.push(DrawCommand::PopClip);
        if let Some(th) = self.thumb(&b) {
            let live = self.ui.slot(self.id ^ K_DRAG).is_some() || self.ui.hovered(&th);
            out.push(DrawCommand::Quad {
                bounds: th,
                fill: if live {
                    p.border_strong
                } else {
                    p.border_strong.with_alpha(0.55)
                },
                radius: BAR / 2.0,
                border: None,
                shadow: None,
            });
        }
    }
}
