//! Date field with a calendar popup.

use crate::a11y::Role;
use crate::{
    ui::Ui,
    util::{self, focus_ring},
};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// A calendar date (proleptic Gregorian).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    /// Year.
    pub year: i32,
    /// Month, 1 to 12.
    pub month: u32,
    /// Day of month, 1 to 31.
    pub day: u32,
}

impl Date {
    /// A date; the day is clamped into the month.
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        let month = month.clamp(1, 12);
        Self {
            year,
            month,
            day: day.clamp(1, days_in_month(year, month)),
        }
    }
    /// Days from 1970-01-01 (negative before).
    pub fn to_days(self) -> i64 {
        let y = self.year as i64 - (self.month <= 2) as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = self.month as i64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }
    /// The date `days` days after 1970-01-01.
    pub fn from_days(days: i64) -> Self {
        let z = days + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400) as i32 + (month <= 2) as i32;
        Self { year, month, day }
    }
    /// 0 = Monday … 6 = Sunday.
    pub fn weekday(self) -> u32 {
        (self.to_days() + 3).rem_euclid(7) as u32
    }
    /// The same day `n` months later, clamped to the month's length.
    pub fn add_months(self, n: i32) -> Self {
        let m = self.year * 12 + self.month as i32 - 1 + n;
        Self::new(m.div_euclid(12), m.rem_euclid(12) as u32 + 1, self.day)
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Days in `month` (1 to 12) of `year`.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
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
const CELL: f32 = 34.0;
const HEAD: f32 = 40.0;

/// Picks a [`Date`]. The calendar opens on the overlay layer; Enter or Space
/// toggles it, arrows move the day (Up/Down by a week), PageUp/PageDown the
/// month, Escape closes. Controlled; `today` (if given) is ringed.
pub struct DatePicker<M> {
    ui: Ui,
    id: u64,
    selected: Option<Date>,
    today: Option<Date>,
    placeholder: String,
    on_select: Option<Box<dyn Fn(Date) -> M>>,
    width: Length,
}

impl<M: 'static> DatePicker<M> {
    /// A picker showing `selected`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, selected: Option<Date>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            selected,
            today: None,
            placeholder: "Pick a date".into(),
            on_select: None,
            width: Length::Shrink,
        }
    }
    /// Mark this day as today.
    pub fn today(mut self, d: Date) -> Self {
        self.today = Some(d);
        self
    }
    /// Text shown while nothing is selected.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// Called with the chosen date; without it the field is disabled.
    pub fn on_select(mut self, f: impl Fn(Date) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
    /// Width sizing.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn is_open(&self) -> bool {
        self.ui.popup() == Some(self.id)
    }
    /// First of the month being shown.
    fn shown(&self) -> Date {
        match self.ui.slot(self.id) {
            Some(m) => Date::new(m.div_euclid(12) as i32, m.rem_euclid(12) as u32 + 1, 1),
            None => {
                let d = self
                    .selected
                    .or(self.today)
                    .unwrap_or(Date::new(2026, 1, 1));
                Date::new(d.year, d.month, 1)
            }
        }
    }
    fn show(&self, d: Date) {
        self.ui
            .set_slot(self.id, Some(d.year as i64 * 12 + d.month as i64 - 1));
    }
    fn popup(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x, b.origin.y + b.size.height + 4.0),
            Size::new(7.0 * CELL + 16.0, HEAD + 7.0 * CELL + 16.0),
        )
    }
    fn cell(&self, pop: &Rectangle, week: u32, wd: u32) -> Rectangle {
        Rectangle::new(
            Point::new(
                pop.origin.x + 8.0 + wd as f32 * CELL,
                pop.origin.y + 8.0 + HEAD + (week + 1) as f32 * CELL,
            ),
            Size::new(CELL, CELL),
        )
    }
    fn arrow(&self, pop: &Rectangle, next: bool) -> Rectangle {
        let x = if next {
            pop.origin.x + pop.size.width - 8.0 - CELL
        } else {
            pop.origin.x + 8.0
        };
        Rectangle::new(
            Point::new(x, pop.origin.y + 8.0),
            Size::new(CELL, HEAD - 4.0),
        )
    }
    /// The date in the cell under `p`, if any.
    fn day_at(&self, pop: &Rectangle, p: Point) -> Option<Date> {
        let first = self.shown();
        (0..6)
            .flat_map(|w| (0..7).map(move |d| (w, d)))
            .find_map(|(w, wd)| {
                let n = (w * 7 + wd) as i64 - first.weekday() as i64;
                let date = Date::from_days(first.to_days() + n);
                (date.month == first.month && self.cell(pop, w, wd).contains(p)).then_some(date)
            })
    }
    fn click(&self, pop: &Rectangle, p: Point, on: &dyn Fn(Date) -> M, out: &mut Vec<M>) {
        if self.arrow(pop, false).contains(p) {
            self.show(self.shown().add_months(-1));
        } else if self.arrow(pop, true).contains(p) {
            self.show(self.shown().add_months(1));
        } else if let Some(d) = self.day_at(pop, p) {
            self.ui.set_popup(None);
            self.ui.set_slot(self.id, None);
            out.push(on(d));
        }
    }
}

impl<M: 'static> Widget<M> for DatePicker<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.body;
        let w = self.ui.metrics().width("0000-00-00", s.size);
        Size::new(
            w + 2.0 * t.spacing.md + 24.0,
            s.line_height + 2.0 * t.spacing.sm,
        )
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let pop = self.popup(&b);
        let Some(on) = &self.on_select else {
            return false;
        };
        match event {
            Event::MousePressed(MouseButton::Left) => {
                if let Some(p) = self.ui.take_hit(self.id) {
                    self.click(&pop, p, on.as_ref(), out);
                    return true;
                }
                if self.is_open() {
                    self.ui.set_popup(None);
                    self.ui.set_slot(self.id, None);
                    return b.contains(cursor);
                }
                if b.contains(cursor) {
                    self.ui.set_focus(Some(self.id));
                    self.ui.set_popup(Some(self.id));
                    return true;
                }
                false
            }
            Event::KeyPressed(Key::Escape, _) if self.is_open() => {
                self.ui.set_popup(None);
                self.ui.set_slot(self.id, None);
                true
            }
            Event::KeyPressed(key, _) if self.ui.is_focused(Some(self.id)) => {
                let base = self.selected.or(self.today).unwrap_or(self.shown());
                let go = |days: i64| Date::from_days(base.to_days() + days);
                let next = match key {
                    Key::Enter => {
                        self.ui
                            .set_popup(if self.is_open() { None } else { Some(self.id) });
                        return true;
                    }
                    Key::Character(c) if c == " " => {
                        self.ui
                            .set_popup(if self.is_open() { None } else { Some(self.id) });
                        return true;
                    }
                    Key::Named(n) => match n.as_str() {
                        "ArrowLeft" => go(-1),
                        "ArrowRight" => go(1),
                        "ArrowUp" => go(-7),
                        "ArrowDown" => go(7),
                        "PageUp" => base.add_months(-1),
                        "PageDown" => base.add_months(1),
                        _ => return false,
                    },
                    _ => return false,
                };
                self.show(next);
                out.push(on(next));
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        self.ui
            .describe(Role::DatePicker, &self.placeholder, &b, |n| {
                n.id = Some(self.id);
                n.value = self.selected.map(|d| d.to_string());
                n.enabled = self.on_select.is_some();
            });
        let s = t.typography.body;
        let enabled = self.on_select.is_some();
        self.ui.register(self.id);
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        let text = |origin: Point, content: String, color: Color, weight: u16, size: f32| {
            DrawCommand::Text {
                family: FontFamily::Sans,
                weight,
                line_height: None,
                origin,
                content,
                size,
                color,
                max_width: None,
            }
        };
        let hover = enabled && self.ui.hovered(&b);
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: if hover || !enabled {
                p.surface
            } else {
                p.surface_raised
            },
            radius: t.radii.medium,
            border: Some(Border {
                color: if enabled { p.border_strong } else { p.border },
                width: 1.0,
            }),
            shadow: None,
        });
        let y = b.origin.y + (b.size.height - s.line_height) / 2.0;
        let (label, color) = match self.selected {
            Some(d) if enabled => (d.to_string(), p.text),
            Some(d) => (d.to_string(), p.text_disabled),
            None => (self.placeholder.clone(), p.text_secondary),
        };
        out.push(text(
            Point::new(b.origin.x + t.spacing.md, y),
            label,
            color,
            400,
            s.size,
        ));
        out.push(text(
            Point::new(b.origin.x + b.size.width - t.spacing.md - 10.0, y),
            "\u{25BE}".into(),
            p.text_secondary,
            400,
            s.size,
        ));
        if !self.is_open() {
            return;
        }
        let pop = self.popup(&b);
        let first = self.shown();
        let mut c = vec![DrawCommand::Quad {
            bounds: pop,
            fill: p.surface_raised,
            radius: t.radii.medium,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: Some(t.elevation.medium),
        }];
        let m = self.ui.metrics();
        let centred = |r: &Rectangle, s: &str, size: f32, weight: u16| {
            let w = m.width_styled(s, size, weight, FontFamily::Sans);
            Point::new(
                r.origin.x + (r.size.width - w) / 2.0,
                r.origin.y + (r.size.height - t.typography.body.line_height) / 2.0,
            )
        };
        let title = format!("{} {}", MONTHS[first.month as usize - 1], first.year);
        let head = Rectangle::new(
            Point::new(pop.origin.x, pop.origin.y + 8.0),
            Size::new(pop.size.width, HEAD - 4.0),
        );
        c.push(text(
            centred(&head, &title, s.size, 600),
            title,
            p.text,
            600,
            s.size,
        ));
        for (next, g) in [(false, "\u{2039}"), (true, "\u{203A}")] {
            let r = self.arrow(&pop, next);
            if r.contains(self.ui.cursor()) {
                c.push(DrawCommand::Quad {
                    bounds: r,
                    fill: p.hover,
                    radius: t.radii.small,
                    border: None,
                    shadow: None,
                });
            }
            c.push(text(
                centred(&r, g, s.size + 4.0, 500),
                g.into(),
                p.text,
                500,
                s.size + 4.0,
            ));
        }
        c.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(pop.origin.x + 8.0, pop.origin.y + 6.0 + HEAD),
                Size::new(pop.size.width - 16.0, 1.0),
            ),
            fill: p.border.with_alpha(0.7),
            radius: 0.0,
            border: None,
            shadow: None,
        });
        for (i, d) in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
            .iter()
            .enumerate()
        {
            let r = self.cell(&pop, 0, i as u32);
            let r = Rectangle::new(Point::new(r.origin.x, r.origin.y - CELL), r.size);
            let cs = t.typography.caption.size;
            c.push(text(
                centred(&r, d, cs, 500),
                (*d).into(),
                p.text_secondary,
                500,
                cs,
            ));
        }
        let hot = self.day_at(&pop, self.ui.cursor());
        for w in 0..6u32 {
            for wd in 0..7u32 {
                let n = (w * 7 + wd) as i64 - first.weekday() as i64;
                let d = Date::from_days(first.to_days() + n);
                if d.month != first.month {
                    continue;
                }
                let r = self.cell(&pop, w, wd);
                let r = Rectangle::new(
                    Point::new(r.origin.x + 2.0, r.origin.y + 2.0),
                    Size::new(CELL - 4.0, CELL - 4.0),
                );
                let sel = self.selected == Some(d);
                if sel || hot == Some(d) {
                    c.push(DrawCommand::Quad {
                        bounds: r,
                        fill: if sel {
                            p.accent
                        } else if hot == Some(d) {
                            p.hover
                        } else {
                            Color::TRANSPARENT
                        },
                        radius: t.radii.medium,
                        border: None,
                        shadow: None,
                    });
                }
                let label = d.day.to_string();
                let today = self.today == Some(d) && !sel;
                let weight = if sel || today { 600 } else { 400 };
                c.push(text(
                    centred(&r, &label, s.size, weight),
                    label,
                    if sel {
                        p.on_accent
                    } else if today {
                        p.accent
                    } else {
                        p.text
                    },
                    weight,
                    s.size,
                ));
            }
        }
        self.ui.overlay(Some(self.id), Some(pop), c);
    }
}
