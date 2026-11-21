//! Calendar and scheduler: month, week and day views with draggable entries.

use crate::badge::Tone;
use crate::date_picker::{days_in_month, Date};
use crate::util::{self, label_cmd, mix};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

/// Which grid the calendar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CalView {
    /// A month of days with entry chips.
    #[default]
    Month,
    /// Seven day columns on an hourly time grid.
    Week,
    /// One day column on an hourly time grid.
    Day,
}

/// Something scheduled: a title on one day between two times.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Stable identity, reported in [`CalAction`]s.
    pub id: u64,
    /// Shown on the entry.
    pub title: String,
    /// The day.
    pub date: Date,
    /// Start, in minutes after midnight.
    pub start: u32,
    /// End, in minutes after midnight (after `start`).
    pub end: u32,
    /// Colour role.
    pub tone: Tone,
}

impl Entry {
    /// An entry from `start` to `end` minutes after midnight on `date`
    /// (`9 * 60 + 30` is 09:30).
    pub fn new(id: u64, title: impl Into<String>, date: Date, start: u32, end: u32) -> Self {
        Self {
            id,
            title: title.into(),
            date,
            start,
            end: end.max(start + 15),
            tone: Tone::Accent,
        }
    }
    /// Colour the entry with `tone`.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

/// What the user did. Your `update` applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalAction {
    /// Show the previous month or week or day.
    Prev,
    /// Show the next one.
    Next,
    /// Jump to today.
    Today,
    /// Switch grid.
    View(CalView),
    /// A day cell was clicked (month view): usually switch to that day.
    Day(Date),
    /// An entry was clicked.
    Open(u64),
    /// An empty slot was clicked: create something on `date` at `minute`
    /// (snapped to 30 minutes).
    Create {
        /// The day.
        date: Date,
        /// Minutes after midnight.
        minute: u32,
    },
    /// An entry was dragged to a new day and/or time (snapped to 15 minutes).
    Move {
        /// The entry.
        id: u64,
        /// New day.
        date: Date,
        /// New start in minutes after midnight; the duration is kept.
        start: u32,
    },
    /// An entry's bottom edge was dragged: its new end in minutes.
    Resize {
        /// The entry.
        id: u64,
        /// New end in minutes after midnight.
        end: u32,
    },
}

type Handler<M> = Rc<dyn Fn(CalAction) -> M>;

/// A calendar with a toolbar (title, previous, today, next, view switch) and
/// a month, week or day grid. The widget is controlled: `anchor` is any date
/// inside the period shown, `view` the grid, and every interaction arrives as
/// a [`CalAction`]. In week and day views, drag an entry to move it (15 minute
/// snap, across days), drag its bottom edge to resize it, click an empty slot
/// to create one. PageUp, PageDown and Home page and jump to today.
pub struct Calendar<M> {
    ui: Ui,
    id: u64,
    anchor: Date,
    view: CalView,
    entries: Vec<Entry>,
    today: Option<Date>,
    now: Option<u32>,
    hours: (u32, u32),
    on_action: Option<Handler<M>>,
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const TOOLBAR: f32 = 52.0;
const GUTTER: f32 = 52.0;
const DAY_HEAD: f32 = 52.0;
const MONTH_HEAD: f32 = 28.0;
const HOUR_H: f32 = 52.0;
const SNAP: u32 = 15;

const K_PRESS: u64 = 0xCA10_0001;
const K_MODE: u64 = 0xCA10_0002;
const K_PX: u64 = 0xCA10_0003;
const K_PY: u64 = 0xCA10_0004;
const K_DRAG: u64 = 0xCA10_0005;
const K_GRAB: u64 = 0xCA10_0006;
const K_CD: u64 = 0xCA10_0007;
const K_CM: u64 = 0xCA10_0008;
const K_SCROLL: u64 = 0xCA10_5C01;

const MODE_MOVE: i64 = 0;
const MODE_RESIZE: i64 = 1;
/// Pressed on empty space: a click there creates or drills in.
const CLICK: i64 = -1;

impl<M: 'static> Calendar<M> {
    /// A calendar showing the period around `anchor` in `view`.
    pub fn new(
        ui: &Ui,
        id: u64,
        anchor: Date,
        view: CalView,
        entries: impl Into<Vec<Entry>>,
    ) -> Self {
        Self {
            ui: ui.clone(),
            id,
            anchor,
            view,
            entries: entries.into(),
            today: None,
            now: None,
            hours: (7, 20),
            on_action: None,
        }
    }
    /// Today's date: its cell gets the accent number.
    pub fn today(mut self, d: Date) -> Self {
        self.today = Some(d);
        self
    }
    /// The current time as minutes after midnight, for the "now" line in week
    /// and day views.
    pub fn now(mut self, minute: u32) -> Self {
        self.now = Some(minute);
        self
    }
    /// First and last hour shown in time grids (default 7 to 20).
    pub fn hours(mut self, from: u32, to: u32) -> Self {
        self.hours = (from.min(23), to.clamp(from + 1, 24));
        self
    }
    /// Receive interactions as messages.
    pub fn on_action(mut self, f: impl Fn(CalAction) -> M + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    // ------------------------------------------------------------ geometry
    fn slot(&self, k: u64) -> Option<i64> {
        self.ui.slot(self.id ^ k)
    }
    fn set(&self, k: u64, v: Option<i64>) {
        self.ui.set_slot(self.id ^ k, v);
    }
    fn week_start(&self) -> Date {
        Date::from_days(self.anchor.to_days() - self.anchor.weekday() as i64)
    }
    fn days(&self) -> Vec<Date> {
        match self.view {
            CalView::Day => vec![self.anchor],
            CalView::Week => (0..7)
                .map(|i| Date::from_days(self.week_start().to_days() + i))
                .collect(),
            CalView::Month => {
                let first = Date::new(self.anchor.year, self.anchor.month, 1);
                let start = first.to_days() - first.weekday() as i64;
                let n = first.weekday() + days_in_month(first.year, first.month);
                let rows = n.div_ceil(7);
                (0..rows as i64 * 7)
                    .map(|i| Date::from_days(start + i))
                    .collect()
            }
        }
    }
    fn body(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x, b.origin.y + TOOLBAR),
            Size::new(b.size.width, (b.size.height - TOOLBAR).max(0.0)),
        )
    }
    /// Toolbar controls: previous, today, next, then the three view tabs.
    fn controls(&self, b: &Rectangle) -> ([Rectangle; 3], [Rectangle; 3]) {
        let t = self.ui.theme();
        let m = self.ui.metrics();
        let lab = t.typography.label;
        let h = 30.0;
        let y = b.origin.y + (TOOLBAR - h) / 2.0;
        let w = |s: &str| m.width_styled(s, lab.size, 600, FontFamily::Sans);
        let mut views = [Rectangle::default(); 3];
        let names = ["Month", "Week", "Day"];
        let mut x = b.origin.x + b.size.width;
        for i in (0..3).rev() {
            let bw = w(names[i]) + 24.0;
            x -= bw;
            views[i] = Rectangle::new(Point::new(x, y), Size::new(bw, h));
        }
        let title_w = m.width_styled(
            &self.title(),
            t.typography.heading.size,
            600,
            FontFamily::Sans,
        );
        let nx = b.origin.x + title_w + 20.0;
        let prev = Rectangle::new(Point::new(nx, y), Size::new(h, h));
        let today_w = w("Today") + 24.0;
        let today = Rectangle::new(Point::new(nx + h + 4.0, y), Size::new(today_w, h));
        let next = Rectangle::new(Point::new(nx + h + 8.0 + today_w, y), Size::new(h, h));
        ([prev, today, next], views)
    }
    fn title(&self) -> String {
        match self.view {
            CalView::Month => format!(
                "{} {}",
                MONTHS[self.anchor.month as usize - 1],
                self.anchor.year
            ),
            CalView::Day => format!(
                "{} {} {}",
                WEEKDAYS[self.anchor.weekday() as usize],
                self.anchor.day,
                MONTHS[self.anchor.month as usize - 1]
            ),
            CalView::Week => {
                let d = self.days();
                let (a, z) = (d[0], d[6]);
                if a.month == z.month {
                    format!(
                        "{} {} – {}, {}",
                        MONTHS[a.month as usize - 1],
                        a.day,
                        z.day,
                        z.year
                    )
                } else {
                    format!(
                        "{} {} – {} {}, {}",
                        &MONTHS[a.month as usize - 1][..3],
                        a.day,
                        &MONTHS[z.month as usize - 1][..3],
                        z.day,
                        z.year
                    )
                }
            }
        }
    }
    fn lo_min(&self) -> u32 {
        self.hours.0 * 60
    }
    fn hi_min(&self) -> u32 {
        self.hours.1 * 60
    }
    fn grid_h(&self) -> f32 {
        (self.hours.1 - self.hours.0) as f32 * HOUR_H
    }
    /// Area that scrolls (time grids): below the day heads.
    fn grid_view(&self, b: &Rectangle) -> Rectangle {
        let body = self.body(b);
        Rectangle::new(
            Point::new(body.origin.x + GUTTER, body.origin.y + DAY_HEAD),
            Size::new(
                (body.size.width - GUTTER).max(0.0),
                (body.size.height - DAY_HEAD).max(0.0),
            ),
        )
    }
    fn scroll(&self, b: &Rectangle) -> f32 {
        let max = (self.grid_h() - self.grid_view(b).size.height).max(0.0);
        (self.ui.scroll_offset(self.id ^ K_SCROLL)).clamp(0.0, max)
    }
    fn col_w(&self, b: &Rectangle) -> f32 {
        self.grid_view(b).size.width / self.days().len() as f32
    }
    fn y_of(&self, b: &Rectangle, minute: u32) -> f32 {
        self.grid_view(b).origin.y + (minute as f32 - self.lo_min() as f32) / 60.0 * HOUR_H
            - self.scroll(b)
    }
    fn minute_at(&self, b: &Rectangle, y: f32) -> f32 {
        self.lo_min() as f32 + (y - self.grid_view(b).origin.y + self.scroll(b)) / HOUR_H * 60.0
    }
    fn snap(m: f32) -> u32 {
        ((m / SNAP as f32).round().max(0.0) as u32) * SNAP
    }
    fn col_at(&self, b: &Rectangle, x: f32) -> usize {
        let v = self.grid_view(b);
        (((x - v.origin.x) / self.col_w(b)).floor().max(0.0) as usize).min(self.days().len() - 1)
    }

    /// Entries of day column `col` with their side-by-side lane `(lane, lanes)`.
    fn lanes(&self, date: Date, skip: Option<u64>) -> Vec<(&Entry, usize, usize)> {
        let mut es: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|e| e.date == date && Some(e.id) != skip)
            .collect();
        es.sort_by_key(|e| (e.start, e.end));
        let mut out: Vec<(&Entry, usize, usize)> = Vec::new();
        let mut cluster: Vec<usize> = Vec::new();
        let mut ends: Vec<u32> = Vec::new();
        let mut cluster_end = 0;
        let flush =
            |out: &mut Vec<(&Entry, usize, usize)>, cluster: &mut Vec<usize>, lanes: usize| {
                for &i in cluster.iter() {
                    out[i].2 = lanes;
                }
                cluster.clear();
            };
        for e in es {
            if e.start >= cluster_end && !cluster.is_empty() {
                let n = ends.len();
                flush(&mut out, &mut cluster, n);
                ends.clear();
            }
            let lane = ends
                .iter()
                .position(|&end| end <= e.start)
                .unwrap_or(ends.len());
            if lane == ends.len() {
                ends.push(e.end);
            } else {
                ends[lane] = e.end;
            }
            cluster_end = cluster_end.max(e.end);
            cluster.push(out.len());
            out.push((e, lane, 1));
        }
        let n = ends.len();
        flush(&mut out, &mut cluster, n.max(1));
        out
    }
    fn entry_rect(
        &self,
        b: &Rectangle,
        col: usize,
        e: &Entry,
        lane: usize,
        lanes: usize,
    ) -> Rectangle {
        let v = self.grid_view(b);
        let cw = self.col_w(b);
        let lw = (cw - 6.0) / lanes as f32;
        let top = self.y_of(b, e.start.max(self.lo_min()));
        let bot = self.y_of(b, e.end.min(self.hi_min()));
        Rectangle::new(
            Point::new(
                v.origin.x + col as f32 * cw + 2.0 + lane as f32 * lw,
                top + 1.0,
            ),
            Size::new((lw - 2.0).max(8.0), (bot - top - 2.0).max(14.0)),
        )
    }
    /// Entry under `p` in a time grid: `(entry, on_bottom_edge)`.
    fn entry_at(&self, b: &Rectangle, p: Point) -> Option<(u64, bool)> {
        if !self.grid_view(b).contains(p) {
            return None;
        }
        for (col, d) in self.days().into_iter().enumerate() {
            for (e, lane, lanes) in self.lanes(d, None).into_iter().rev() {
                let r = self.entry_rect(b, col, e, lane, lanes);
                if r.contains(p) {
                    return Some((
                        e.id,
                        p.y > r.origin.y + r.size.height - 7.0 && r.size.height > 24.0,
                    ));
                }
            }
        }
        None
    }

    // ------------------------------------------------------------ month
    fn month_cell(&self, b: &Rectangle, i: usize) -> Rectangle {
        let body = self.body(b);
        let rows = self.days().len() / 7;
        let cw = body.size.width / 7.0;
        let ch = (body.size.height - MONTH_HEAD) / rows as f32;
        Rectangle::new(
            Point::new(
                body.origin.x + (i % 7) as f32 * cw,
                body.origin.y + MONTH_HEAD + (i / 7) as f32 * ch,
            ),
            Size::new(cw, ch),
        )
    }
    fn month_chip_rows(&self, cell: &Rectangle) -> usize {
        (((cell.size.height - 28.0) / 20.0).floor().max(0.0)) as usize
    }
    fn month_chip(&self, cell: &Rectangle, row: usize) -> Rectangle {
        Rectangle::new(
            Point::new(
                cell.origin.x + 4.0,
                cell.origin.y + 28.0 + row as f32 * 20.0,
            ),
            Size::new(cell.size.width - 8.0, 18.0),
        )
    }
    fn month_hit(&self, b: &Rectangle, p: Point) -> Option<(Date, Option<u64>)> {
        let days = self.days();
        for (i, d) in days.iter().enumerate() {
            let cell = self.month_cell(b, i);
            if !cell.contains(p) {
                continue;
            }
            let es: Vec<&Entry> = self.entries.iter().filter(|e| e.date == *d).collect();
            let rows = self.month_chip_rows(&cell);
            let shown = if es.len() > rows && rows > 0 {
                rows - 1
            } else {
                es.len().min(rows)
            };
            for (k, e) in es.iter().take(shown).enumerate() {
                if self.month_chip(&cell, k).contains(p) {
                    return Some((*d, Some(e.id)));
                }
            }
            return Some((*d, None));
        }
        None
    }

    fn emit(&self, out: &mut Vec<M>, a: CalAction) {
        if let Some(f) = &self.on_action {
            out.push(f(a));
        }
    }
    fn clear_press(&self) {
        for k in [K_PRESS, K_MODE, K_PX, K_PY, K_DRAG, K_GRAB, K_CD, K_CM] {
            self.set(k, None);
        }
    }
    fn find(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }
    /// What dropping the dragged entry at the cursor would do.
    fn drag_result(&self, b: &Rectangle, p: Point) -> Option<(CalAction, Date, u32, u32)> {
        let id = self.slot(K_PRESS)? as u64;
        if self.slot(K_PRESS)? < 0 || self.slot(K_DRAG).is_none() {
            return None;
        }
        let e = self.find(id)?;
        match (self.view, self.slot(K_MODE)) {
            (CalView::Month, _) => {
                let (d, _) = self.month_hit(b, p)?;
                Some((
                    CalAction::Move {
                        id,
                        date: d,
                        start: e.start,
                    },
                    d,
                    e.start,
                    e.end,
                ))
            }
            (_, Some(MODE_RESIZE)) => {
                let end = Self::snap(self.minute_at(b, p.y))
                    .clamp(e.start + SNAP, self.hi_min().max(e.start + SNAP));
                Some((CalAction::Resize { id, end }, e.date, e.start, end))
            }
            _ => {
                let days = self.days();
                let d = days[self.col_at(b, p.x)];
                let grab = self.slot(K_GRAB).unwrap_or(0) as f32;
                let dur = e.end - e.start;
                let raw = Self::snap(self.minute_at(b, p.y) - grab);
                let start = raw.clamp(
                    self.lo_min(),
                    self.hi_min().saturating_sub(dur).max(self.lo_min()),
                );
                Some((
                    CalAction::Move { id, date: d, start },
                    d,
                    start,
                    start + dur,
                ))
            }
        }
    }
}

fn clock(m: u32) -> String {
    format!("{:02}:{:02}", m / 60 % 24, m % 60)
}

impl<M: 'static> Widget<M> for Calendar<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Fill(1))
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(420.0, 360.0)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
                let ([prev, today, next], views) = self.controls(&b);
                if prev.contains(cursor) {
                    self.emit(out, CalAction::Prev);
                } else if next.contains(cursor) {
                    self.emit(out, CalAction::Next);
                } else if today.contains(cursor) {
                    self.emit(out, CalAction::Today);
                } else if let Some(i) = views.iter().position(|r| r.contains(cursor)) {
                    self.emit(
                        out,
                        CalAction::View([CalView::Month, CalView::Week, CalView::Day][i]),
                    );
                } else if self.view == CalView::Month {
                    let Some((d, entry)) = self.month_hit(&b, cursor) else {
                        return true;
                    };
                    self.set(K_PX, Some((cursor.x * 8.0) as i64));
                    self.set(K_PY, Some((cursor.y * 8.0) as i64));
                    match entry {
                        Some(id) => {
                            self.set(K_PRESS, Some(id as i64));
                            self.set(K_MODE, Some(MODE_MOVE));
                        }
                        None => {
                            self.set(K_PRESS, Some(CLICK));
                            self.set(K_CD, Some(d.to_days()));
                        }
                    }
                } else if self.grid_view(&b).contains(cursor) {
                    self.set(K_PX, Some((cursor.x * 8.0) as i64));
                    self.set(K_PY, Some((cursor.y * 8.0) as i64));
                    if let Some((id, edge)) = self.entry_at(&b, cursor) {
                        let e = self.find(id).cloned();
                        self.set(K_PRESS, Some(id as i64));
                        self.set(K_MODE, Some(if edge { MODE_RESIZE } else { MODE_MOVE }));
                        if let Some(e) = e {
                            let grab = self.minute_at(&b, cursor.y) - e.start as f32;
                            self.set(K_GRAB, Some(grab as i64));
                        }
                    } else {
                        let days = self.days();
                        let d = days[self.col_at(&b, cursor.x)];
                        let m = (self.minute_at(&b, cursor.y) / 30.0).floor().max(0.0) as i64 * 30;
                        self.set(K_PRESS, Some(CLICK));
                        self.set(K_CD, Some(d.to_days()));
                        self.set(K_CM, Some(m));
                    }
                }
                true
            }
            Event::CursorMoved(p) => {
                let Some(press) = self.slot(K_PRESS) else {
                    return false;
                };
                if press >= 0 && self.slot(K_DRAG).is_none() {
                    let (sx, sy) = (
                        self.slot(K_PX).unwrap_or(0) as f32 / 8.0,
                        self.slot(K_PY).unwrap_or(0) as f32 / 8.0,
                    );
                    if (p.x - sx).hypot(p.y - sy) >= 4.0 {
                        self.set(K_DRAG, Some(1));
                    }
                }
                true
            }
            Event::MouseReleased(MouseButton::Left) => {
                let Some(press) = self.slot(K_PRESS) else {
                    return false;
                };
                let result = self.drag_result(&b, cursor);
                let click_day = self.slot(K_CD).map(Date::from_days);
                let click_min = self.slot(K_CM).unwrap_or(0) as u32;
                let (px, py) = (
                    self.slot(K_PX).unwrap_or(0) as f32 / 8.0,
                    self.slot(K_PY).unwrap_or(0) as f32 / 8.0,
                );
                let moved = (cursor.x - px).hypot(cursor.y - py) >= 4.0;
                self.clear_press();
                if let Some((action, ..)) = result {
                    self.emit(out, action);
                } else if press >= 0 && !moved {
                    self.emit(out, CalAction::Open(press as u64));
                } else if press == CLICK && !moved {
                    if let Some(d) = click_day {
                        if self.view == CalView::Month {
                            self.emit(out, CalAction::Day(d));
                        } else {
                            self.emit(
                                out,
                                CalAction::Create {
                                    date: d,
                                    minute: click_min,
                                },
                            );
                        }
                    }
                }
                true
            }
            Event::Scrolled { dy, .. }
                if self.view != CalView::Month && self.grid_view(&b).contains(cursor) =>
            {
                let max = (self.grid_h() - self.grid_view(&b).size.height).max(0.0);
                let cur = self.scroll(&b);
                let next = (cur - dy).clamp(0.0, max);
                self.ui.set_scroll_offset(self.id ^ K_SCROLL, next);
                next != cur
            }
            Event::KeyPressed(Key::Named(n), _) if self.ui.is_focused(Some(self.id)) => {
                match n.as_str() {
                    "PageUp" => self.emit(out, CalAction::Prev),
                    "PageDown" => self.emit(out, CalAction::Next),
                    "Home" => self.emit(out, CalAction::Today),
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        self.ui.register(self.id);
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let lab = t.typography.label;
        let m = self.ui.metrics();
        let width_of = |s: &str, size: f32, w: u16| m.width_styled(s, size, w, FontFamily::Sans);
        let b = layout.bounds;
        let cur = self.ui.cursor();
        self.ui.describe(
            Role::Label,
            &format!("Calendar, {}", self.title()),
            &b,
            |_| {},
        );

        // ---- toolbar
        out.push(label_cmd(
            Point::new(
                b.origin.x,
                b.origin.y + (TOOLBAR - t.typography.heading.line_height) / 2.0,
            ),
            self.title(),
            &t.typography.heading,
            600,
            p.text,
        ));
        let ([prev, today, next], views) = self.controls(&b);
        let button = |out: &mut DrawList, r: Rectangle, text: &str, on: bool| {
            let hot = r.contains(cur);
            out.push(DrawCommand::Quad {
                bounds: r,
                fill: if on {
                    p.surface_raised
                } else if hot {
                    p.hover
                } else {
                    Color::TRANSPARENT
                },
                radius: t.radii.medium,
                border: Some(Border {
                    color: if on || hot {
                        p.border_control
                    } else {
                        p.border
                    },
                    width: 1.0,
                }),
                shadow: on.then_some(t.elevation.low),
            });
            let w = width_of(text, lab.size, 600);
            out.push(label_cmd(
                Point::new(
                    r.origin.x + (r.size.width - w) / 2.0,
                    r.origin.y + (r.size.height - lab.line_height) / 2.0,
                ),
                text,
                &lab,
                if on { 600 } else { 400 },
                if on { p.text } else { p.text_secondary },
            ));
        };
        button(out, prev, "‹", false);
        button(out, today, "Today", false);
        button(out, next, "›", false);
        // Segmented view switch on a recessed track.
        let track = Rectangle::new(
            Point::new(views[0].origin.x - 3.0, views[0].origin.y - 3.0),
            Size::new(
                views[2].origin.x + views[2].size.width - views[0].origin.x + 6.0,
                views[0].size.height + 6.0,
            ),
        );
        out.push(DrawCommand::Quad {
            bounds: track,
            fill: p.surface,
            radius: t.radii.medium + 2.0,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: None,
        });
        for (i, (r, name)) in views.iter().zip(["Month", "Week", "Day"]).enumerate() {
            let on = [CalView::Month, CalView::Week, CalView::Day][i] == self.view;
            if on {
                out.push(DrawCommand::Quad {
                    bounds: *r,
                    fill: p.surface_raised,
                    radius: t.radii.medium,
                    border: None,
                    shadow: Some(t.elevation.low),
                });
            } else if r.contains(cur) {
                out.push(DrawCommand::Quad {
                    bounds: *r,
                    fill: p.hover,
                    radius: t.radii.medium,
                    border: None,
                    shadow: None,
                });
            }
            let w = width_of(name, lab.size, 600);
            out.push(label_cmd(
                Point::new(
                    r.origin.x + (r.size.width - w) / 2.0,
                    r.origin.y + (r.size.height - lab.line_height) / 2.0,
                ),
                name,
                &lab,
                if on { 600 } else { 400 },
                if on { p.text } else { p.text_secondary },
            ));
        }

        let drag = self.slot(K_DRAG).is_some();
        let dragged = self.slot(K_PRESS).filter(|v| *v >= 0).map(|v| v as u64);
        let result = if drag {
            self.drag_result(&b, cur)
        } else {
            None
        };
        match self.view {
            CalView::Month => self.draw_month(out, &b, dragged.filter(|_| drag), result.as_ref()),
            _ => self.draw_grid(out, &b, dragged.filter(|_| drag), result.as_ref()),
        }

        // Time readout while dragging.
        if let (Some((_, _, s, e)), CalView::Week | CalView::Day) = (result.as_ref(), self.view) {
            let text = format!("{} – {}", clock(*s), clock(*e));
            let w = width_of(&text, cap.size, 600) + 16.0;
            let h = cap.line_height + 10.0;
            let x = (cur.x + 14.0).min(b.origin.x + b.size.width - w);
            let y = (cur.y - h - 10.0).max(b.origin.y);
            self.ui.overlay(
                None,
                None,
                vec![
                    DrawCommand::Quad {
                        bounds: Rectangle::new(Point::new(x, y), Size::new(w, h)),
                        fill: p.surface_raised,
                        radius: t.radii.medium,
                        border: Some(Border {
                            color: p.border,
                            width: 1.0,
                        }),
                        shadow: Some(t.elevation.medium),
                    },
                    label_cmd(Point::new(x + 8.0, y + 5.0), text, &cap, 600, p.text),
                ],
            );
        }
    }
}

impl<M: 'static> Calendar<M> {
    fn draw_month(
        &self,
        out: &mut DrawList,
        b: &Rectangle,
        dragged: Option<u64>,
        result: Option<&(CalAction, Date, u32, u32)>,
    ) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let m = self.ui.metrics();
        let body = self.body(b);
        let days = self.days();
        let rows = days.len() / 7;
        // Frame and header.
        out.push(DrawCommand::Quad {
            bounds: body,
            fill: p.surface,
            radius: t.radii.large,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: None,
        });
        let cw = body.size.width / 7.0;
        for (i, wd) in WEEKDAYS.iter().enumerate() {
            out.push(label_cmd(
                Point::new(
                    body.origin.x + i as f32 * cw + 10.0,
                    body.origin.y + (MONTH_HEAD - cap.line_height) / 2.0 + 2.0,
                ),
                *wd,
                &cap,
                600,
                p.text_secondary,
            ));
        }
        let drop_day = result.map(|r| r.1);
        for (i, d) in days.iter().enumerate() {
            let cell = self.month_cell(b, i);
            let other = d.month != self.anchor.month;
            // Hairlines: top of every row, left of every column but the first.
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(cell.origin, Size::new(cell.size.width, 1.0)),
                fill: p.border,
                radius: 0.0,
                border: None,
                shadow: None,
            });
            if i % 7 != 0 {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(cell.origin, Size::new(1.0, cell.size.height)),
                    fill: p.border,
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
            if drop_day == Some(*d) {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(cell.origin.x + 1.0, cell.origin.y + 1.0),
                        Size::new(cell.size.width - 1.0, cell.size.height - 1.0),
                    ),
                    fill: p.accent.with_alpha(0.08),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            } else if other {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(cell.origin.x + 1.0, cell.origin.y + 1.0),
                        Size::new(cell.size.width - 1.0, cell.size.height - 1.0),
                    ),
                    fill: p.hover.with_alpha(p.hover.a * 0.5),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
            // Day number.
            let is_today = self.today == Some(*d);
            let num = d.day.to_string();
            let nw = m.width_styled(&num, cap.size, 600, FontFamily::Sans);
            if is_today {
                let c = Rectangle::new(
                    Point::new(cell.origin.x + 6.0, cell.origin.y + 4.0),
                    Size::new(nw.max(8.0) + 12.0, cap.line_height + 2.0),
                );
                out.push(DrawCommand::Quad {
                    bounds: c,
                    fill: p.accent,
                    radius: c.size.height / 2.0,
                    border: None,
                    shadow: None,
                });
                out.push(label_cmd(
                    Point::new(c.origin.x + (c.size.width - nw) / 2.0, c.origin.y + 1.0),
                    num,
                    &cap,
                    600,
                    p.on_accent,
                ));
            } else {
                out.push(label_cmd(
                    Point::new(cell.origin.x + 12.0, cell.origin.y + 5.0),
                    num,
                    &cap,
                    600,
                    if other { p.text_disabled } else { p.text },
                ));
            }
            // Entry chips.
            let es: Vec<&Entry> = {
                let mut v: Vec<&Entry> = self.entries.iter().filter(|e| e.date == *d).collect();
                v.sort_by_key(|e| e.start);
                v
            };
            let rows_fit = self.month_chip_rows(&cell);
            let shown = if es.len() > rows_fit && rows_fit > 0 {
                rows_fit - 1
            } else {
                es.len().min(rows_fit)
            };
            out.push(DrawCommand::PushClip(cell));
            for (k, e) in es.iter().take(shown).enumerate() {
                let r = self.month_chip(&cell, k);
                let ink = e.tone.color(&t);
                let hot = r.contains(self.ui.cursor()) && dragged.is_none();
                let faded = dragged == Some(e.id);
                out.push(DrawCommand::Quad {
                    bounds: r,
                    fill: ink
                        .with_alpha(if hot { 0.2 } else { 0.12 } * if faded { 0.4 } else { 1.0 }),
                    radius: t.radii.small + 1.0,
                    border: None,
                    shadow: None,
                });
                out.push(label_cmd(
                    Point::new(
                        r.origin.x + 6.0,
                        r.origin.y + (r.size.height - cap.line_height) / 2.0,
                    ),
                    format!("{}  {}", clock(e.start), e.title),
                    &cap,
                    400,
                    if faded { p.text_secondary } else { p.text },
                ));
            }
            if es.len() > shown {
                let r = self.month_chip(&cell, shown);
                out.push(label_cmd(
                    Point::new(
                        r.origin.x + 6.0,
                        r.origin.y + (r.size.height - cap.line_height) / 2.0,
                    ),
                    format!("+{} more", es.len() - shown),
                    &cap,
                    600,
                    p.text_secondary,
                ));
            }
            out.push(DrawCommand::PopClip);
        }
        let _ = rows;
        // Chip in hand.
        if let Some(id) = dragged {
            if let Some(e) = self.find(id) {
                let ink = e.tone.color(&t);
                let cell_w = body.size.width / 7.0 - 8.0;
                let cur = self.ui.cursor();
                let r = Rectangle::new(
                    Point::new(cur.x - cell_w / 2.0, cur.y - 9.0),
                    Size::new(cell_w, 18.0),
                );
                self.ui.overlay(
                    None,
                    None,
                    vec![
                        DrawCommand::Quad {
                            bounds: r,
                            fill: mix(p.surface_raised, ink, 0.18),
                            radius: t.radii.small + 1.0,
                            border: Some(Border {
                                color: ink.with_alpha(0.7),
                                width: 1.0,
                            }),
                            shadow: Some(t.elevation.medium),
                        },
                        label_cmd(
                            Point::new(
                                r.origin.x + 6.0,
                                r.origin.y + (r.size.height - cap.line_height) / 2.0,
                            ),
                            e.title.clone(),
                            &cap,
                            400,
                            p.text,
                        ),
                    ],
                );
            }
        }
    }

    fn draw_grid(
        &self,
        out: &mut DrawList,
        b: &Rectangle,
        dragged: Option<u64>,
        result: Option<&(CalAction, Date, u32, u32)>,
    ) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let lab = t.typography.label;
        let m = self.ui.metrics();
        let body = self.body(b);
        let v = self.grid_view(b);
        let days = self.days();
        let cw = self.col_w(b);
        out.push(DrawCommand::Quad {
            bounds: body,
            fill: p.surface,
            radius: t.radii.large,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: None,
        });
        // Day heads.
        for (i, d) in days.iter().enumerate() {
            let x = v.origin.x + i as f32 * cw;
            let is_today = self.today == Some(*d);
            let wd = WEEKDAYS[d.weekday() as usize];
            let num = d.day.to_string();
            let ww = m.width_styled(wd, cap.size, 600, FontFamily::Sans);
            out.push(label_cmd(
                Point::new(x + (cw - ww) / 2.0, body.origin.y + 8.0),
                wd,
                &cap,
                600,
                if is_today { p.accent } else { p.text_secondary },
            ));
            let nw = m.width_styled(&num, lab.size, 600, FontFamily::Sans);
            if is_today {
                let c = Rectangle::new(
                    Point::new(
                        x + (cw - 30.0) / 2.0,
                        body.origin.y + 8.0 + cap.line_height + 2.0,
                    ),
                    Size::new(30.0, 22.0),
                );
                out.push(DrawCommand::Quad {
                    bounds: c,
                    fill: p.accent,
                    radius: 11.0,
                    border: None,
                    shadow: None,
                });
                out.push(label_cmd(
                    Point::new(
                        c.origin.x + (30.0 - nw) / 2.0,
                        c.origin.y + (22.0 - lab.line_height) / 2.0,
                    ),
                    num,
                    &lab,
                    600,
                    p.on_accent,
                ));
            } else {
                out.push(label_cmd(
                    Point::new(
                        x + (cw - nw) / 2.0,
                        body.origin.y + 8.0 + cap.line_height + 3.0,
                    ),
                    num,
                    &lab,
                    600,
                    p.text,
                ));
            }
        }
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(body.origin.x, v.origin.y - 1.0),
                Size::new(body.size.width, 1.0),
            ),
            fill: p.border,
            radius: 0.0,
            border: None,
            shadow: None,
        });

        out.push(DrawCommand::PushClip(Rectangle::new(
            Point::new(body.origin.x + 1.0, v.origin.y),
            Size::new(body.size.width - 2.0, v.size.height - 1.0),
        )));
        // Today column tint.
        for (i, d) in days.iter().enumerate() {
            if self.today == Some(*d) && days.len() > 1 {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(v.origin.x + i as f32 * cw, v.origin.y),
                        Size::new(cw, v.size.height),
                    ),
                    fill: p.accent.with_alpha(0.045),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
        }
        // Hour lines and labels.
        for h in self.hours.0..=self.hours.1 {
            let y = self.y_of(b, h * 60);
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(v.origin.x, y - 0.5),
                    Size::new(v.size.width, 1.0),
                ),
                fill: p.border.with_alpha(0.8),
                radius: 0.0,
                border: None,
                shadow: None,
            });
            if h < self.hours.1 {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(v.origin.x, y + HOUR_H / 2.0 - 0.5),
                        Size::new(v.size.width, 1.0),
                    ),
                    fill: p.border.with_alpha(0.3),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
            if h > self.hours.0 && h < 24 {
                let s = clock(h * 60);
                let w = m.width_styled(&s, cap.size, 400, FontFamily::Sans);
                out.push(label_cmd(
                    Point::new(body.origin.x + GUTTER - 10.0 - w, y - cap.line_height / 2.0),
                    s,
                    &cap,
                    400,
                    p.text_secondary,
                ));
            }
        }
        // Column separators.
        for i in 0..days.len() {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(v.origin.x + i as f32 * cw - 0.5, v.origin.y),
                    Size::new(1.0, v.size.height),
                ),
                fill: p.border,
                radius: 0.0,
                border: None,
                shadow: None,
            });
        }
        // Entries.
        for (col, d) in days.iter().enumerate() {
            for (e, lane, lanes) in self.lanes(*d, None) {
                let r = self.entry_rect(b, col, e, lane, lanes);
                self.draw_entry(out, e, r, dragged == Some(e.id), false);
            }
        }
        // Ghost at the drop position.
        if let (Some(id), Some((_, date, s, en))) = (dragged, result) {
            if let (Some(e), Some(col)) = (self.find(id), days.iter().position(|d| d == date)) {
                let ghost = Entry {
                    date: *date,
                    start: *s,
                    end: *en,
                    ..e.clone()
                };
                let r = self.entry_rect(b, col, &ghost, 0, 1);
                self.draw_entry(out, &ghost, r, false, true);
            }
        }
        // Now line.
        if let Some(now) = self.now {
            if let Some(col) = days.iter().position(|d| Some(*d) == self.today) {
                if now >= self.lo_min() && now <= self.hi_min() {
                    let y = self.y_of(b, now);
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(v.origin.x + col as f32 * cw, y - 1.0),
                            Size::new(cw, 2.0),
                        ),
                        fill: p.danger,
                        radius: 1.0,
                        border: None,
                        shadow: None,
                    });
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(v.origin.x + col as f32 * cw - 4.0, y - 4.0),
                            Size::new(8.0, 8.0),
                        ),
                        fill: p.danger,
                        radius: 4.0,
                        border: None,
                        shadow: None,
                    });
                }
            }
        }
        out.push(DrawCommand::PopClip);
    }

    fn draw_entry(&self, out: &mut DrawList, e: &Entry, r: Rectangle, faded: bool, ghost: bool) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let ink = e.tone.color(&t);
        let hot = r.contains(self.ui.cursor()) && !ghost && self.slot(K_DRAG).is_none();
        let a = if faded { 0.4 } else { 1.0 };
        out.push(DrawCommand::Quad {
            bounds: r,
            fill: mix(
                p.surface,
                ink,
                if ghost {
                    0.28
                } else if hot {
                    0.2
                } else {
                    0.14
                },
            )
            .with_alpha(a),
            radius: t.radii.medium,
            border: Some(Border {
                color: ink.with_alpha(if ghost { 0.9 } else { 0.45 } * a),
                width: 1.0,
            }),
            shadow: ghost.then_some(t.elevation.medium),
        });
        out.push(DrawCommand::PushClip(r));
        out.push(label_cmd(
            Point::new(r.origin.x + 8.0, r.origin.y + 4.0),
            e.title.clone(),
            &cap,
            600,
            if faded { p.text_secondary } else { p.text },
        ));
        if r.size.height > cap.line_height * 2.0 + 6.0 {
            out.push(label_cmd(
                Point::new(r.origin.x + 8.0, r.origin.y + 4.0 + cap.line_height),
                format!("{} – {}", clock(e.start), clock(e.end)),
                &cap,
                400,
                p.text_secondary,
            ));
        }
        out.push(DrawCommand::PopClip);
    }
}
