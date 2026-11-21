//! Kanban board: columns of cards you drag between.

use crate::badge::Tone;
use crate::util::{self, label_cmd, mix};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

/// One card. `id` must be unique across the whole board and stable between
/// rebuilds; the board reports moves by id.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// Stable identity.
    pub id: u64,
    /// First line, semibold.
    pub title: String,
    /// Optional second line, dim.
    pub detail: Option<String>,
    /// Optional pill at the bottom, e.g. `("Blocked", Tone::Danger)`.
    pub tag: Option<(String, Tone)>,
}

impl Card {
    /// A card titled `title`.
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            detail: None,
            tag: None,
        }
    }
    /// A dim second line.
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
    /// A status pill.
    pub fn tag(mut self, label: impl Into<String>, tone: Tone) -> Self {
        self.tag = Some((label.into(), tone));
        self
    }
}

/// A named column of cards.
#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    /// Header text.
    pub title: String,
    /// Cards top to bottom.
    pub cards: Vec<Card>,
}

impl Lane {
    /// A lane of `cards`.
    pub fn new(title: impl Into<String>, cards: impl Into<Vec<Card>>) -> Self {
        Self {
            title: title.into(),
            cards: cards.into(),
        }
    }
}

/// A finished drag (or an Alt+arrow keyboard move): card `card` leaves lane
/// `from` and should be inserted at `index` of lane `to` (indices count the
/// target lane *without* the card, so `Vec::insert(index, card)` is correct
/// after removing it from `from`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    /// The card's id.
    pub card: u64,
    /// Source lane index.
    pub from: usize,
    /// Target lane index.
    pub to: usize,
    /// Insert position within the target lane after removal.
    pub index: usize,
}

impl Move {
    /// Apply this move to `lanes` (what an `update` function usually does).
    pub fn apply(&self, lanes: &mut [Lane]) {
        let Some(at) = lanes
            .get(self.from)
            .and_then(|l| l.cards.iter().position(|c| c.id == self.card))
        else {
            return;
        };
        if self.to >= lanes.len() {
            return;
        }
        let card = lanes[self.from].cards.remove(at);
        let to = &mut lanes[self.to].cards;
        to.insert(self.index.min(to.len()), card);
    }
}

type OnMove<M> = Rc<dyn Fn(Move) -> M>;
type OnOpen<M> = Rc<dyn Fn(u64) -> M>;

/// Columns of cards with drag-and-drop between and within them. The board
/// does not own the data: it reports a [`Move`] and your `update` applies it
/// (see [`Move::apply`]). While dragging, the card follows the pointer, the
/// target lane lights up and a placeholder shows where it will land. Keyboard:
/// focus the board, arrows move the selection, **Alt+arrows** move the selected
/// card, Enter opens it.
pub struct Kanban<M> {
    ui: Ui,
    id: u64,
    lanes: Vec<Lane>,
    height: Length,
    on_move: Option<OnMove<M>>,
    on_open: Option<OnOpen<M>>,
}

const GAP: f32 = 12.0;
const HEAD: f32 = 40.0;
const PAD: f32 = 8.0;
const CARD_GAP: f32 = 8.0;
const DRAG_START: f32 = 4.0;

const K_PRESS: u64 = 0x4B41_0001;
const K_GX: u64 = 0x4B41_0002;
const K_GY: u64 = 0x4B41_0003;
const K_DRAG: u64 = 0x4B41_0004;
const K_SEL: u64 = 0x4B41_0005;
const K_PX: u64 = 0x4B41_0006;
const K_PY: u64 = 0x4B41_0007;

impl<M: 'static> Kanban<M> {
    /// A board of `lanes`. `id` keeps drag and selection state across rebuilds.
    pub fn new(ui: &Ui, id: u64, lanes: impl Into<Vec<Lane>>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            lanes: lanes.into(),
            height: Length::Fill(1),
            on_move: None,
            on_open: None,
        }
    }
    /// Send a message when a card is dropped somewhere new.
    pub fn on_move(mut self, f: impl Fn(Move) -> M + 'static) -> Self {
        self.on_move = Some(Rc::new(f));
        self
    }
    /// Send the card's id when it is clicked (pressed and released without dragging).
    pub fn on_open(mut self, f: impl Fn(u64) -> M + 'static) -> Self {
        self.on_open = Some(Rc::new(f));
        self
    }
    /// Board height (default: fill).
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn slot(&self, k: u64) -> Option<i64> {
        self.ui.slot(self.id ^ k)
    }
    fn set(&self, k: u64, v: Option<i64>) {
        self.ui.set_slot(self.id ^ k, v);
    }
    fn dragging(&self) -> Option<u64> {
        self.slot(K_DRAG).map(|v| v as u64)
    }
    fn selected(&self) -> Option<u64> {
        self.slot(K_SEL).map(|v| v as u64)
    }
    fn find(&self, card: u64) -> Option<(usize, usize)> {
        self.lanes
            .iter()
            .enumerate()
            .find_map(|(l, lane)| lane.cards.iter().position(|c| c.id == card).map(|i| (l, i)))
    }

    fn lane_w(&self, b: &Rectangle) -> f32 {
        let n = self.lanes.len().max(1) as f32;
        ((b.size.width - GAP * (n - 1.0)) / n).max(120.0)
    }
    fn lane_rect(&self, b: &Rectangle, i: usize) -> Rectangle {
        let w = self.lane_w(b);
        Rectangle::new(
            Point::new(b.origin.x + i as f32 * (w + GAP), b.origin.y),
            Size::new(w, b.size.height),
        )
    }
    fn card_h(&self, c: &Card) -> f32 {
        let t = self.ui.theme();
        let mut h = 2.0 * 10.0 + t.typography.label.line_height;
        if c.detail.is_some() {
            h += t.typography.caption.line_height + 2.0;
        }
        if c.tag.is_some() {
            h += t.typography.caption.line_height + 10.0;
        }
        h
    }
    fn max_scroll(&self, b: &Rectangle, lane: usize, extra: f32) -> f32 {
        let content: f32 = self.lanes[lane]
            .cards
            .iter()
            .map(|c| self.card_h(c) + CARD_GAP)
            .sum::<f32>()
            + extra;
        (content + HEAD + PAD - b.size.height).max(0.0)
    }
    fn scroll(&self, lane: usize) -> f32 {
        self.ui.scroll_offset(self.id ^ (0x5C00 + lane as u64))
    }

    /// Card rectangles of `lane`, skipping `skip`, with a gap of `gap_h` opened
    /// at `gap_at`. Returns the rectangles (in lane order, without the skipped
    /// card) and the gap rectangle.
    fn place(
        &self,
        b: &Rectangle,
        lane: usize,
        skip: Option<u64>,
        gap: Option<(usize, f32)>,
    ) -> (Vec<(u64, Rectangle)>, Option<Rectangle>) {
        let r = self.lane_rect(b, lane);
        let mut y = r.origin.y + HEAD + PAD - self.scroll(lane);
        let mut cards = Vec::new();
        let mut hole = None;
        let mut idx = 0;
        let width = r.size.width - 2.0 * PAD;
        for c in self.lanes[lane].cards.iter().filter(|c| Some(c.id) != skip) {
            if let Some((at, h)) = gap {
                if at == idx {
                    hole = Some(Rectangle::new(
                        Point::new(r.origin.x + PAD, y),
                        Size::new(width, h),
                    ));
                    y += h + CARD_GAP;
                }
            }
            let h = self.card_h(c);
            cards.push((
                c.id,
                Rectangle::new(Point::new(r.origin.x + PAD, y), Size::new(width, h)),
            ));
            y += h + CARD_GAP;
            idx += 1;
        }
        if let Some((at, h)) = gap {
            if hole.is_none() && at >= idx {
                hole = Some(Rectangle::new(
                    Point::new(r.origin.x + PAD, y),
                    Size::new(width, h),
                ));
            }
        }
        (cards, hole)
    }

    fn lane_at(&self, b: &Rectangle, p: Point) -> Option<usize> {
        (0..self.lanes.len()).find(|&i| self.lane_rect(b, i).contains(p))
    }
    fn card_at(&self, b: &Rectangle, p: Point) -> Option<u64> {
        let l = self.lane_at(b, p)?;
        if p.y < b.origin.y + HEAD {
            return None;
        }
        self.place(b, l, None, None)
            .0
            .into_iter()
            .find(|(_, r)| r.contains(p))
            .map(|(id, _)| id)
    }
    /// Lane and insert index (cards excluding `card`) under `p`.
    fn drop_target(&self, b: &Rectangle, card: u64, p: Point) -> Option<(usize, usize)> {
        let lane = self.lane_at(b, p).or_else(|| {
            // Past the outer edges, snap to the nearest lane.
            (!self.lanes.is_empty()).then(|| {
                if p.x < b.origin.x {
                    0
                } else {
                    self.lanes.len() - 1
                }
            })
        })?;
        let (cards, _) = self.place(b, lane, Some(card), None);
        let index = cards
            .iter()
            .filter(|(_, r)| r.origin.y + r.size.height / 2.0 < p.y)
            .count();
        Some((lane, index))
    }

    fn key_move(&self, out: &mut Vec<M>, dl: i32, di: i32, alt: bool) -> bool {
        let Some(sel) = self.selected() else {
            // First arrow press selects the first card.
            if let Some(c) = self.lanes.iter().flat_map(|l| l.cards.first()).next() {
                self.set(K_SEL, Some(c.id as i64));
                return true;
            }
            return false;
        };
        let Some((l, i)) = self.find(sel) else {
            return false;
        };
        if alt {
            let Some(f) = self.on_move.clone() else {
                return false;
            };
            let to = (l as i32 + dl).clamp(0, self.lanes.len() as i32 - 1) as usize;
            let len_after = if to == l {
                self.lanes[l].cards.len() - 1
            } else {
                self.lanes[to].cards.len()
            };
            let index = if to == l {
                (i as i32 + di).clamp(0, len_after as i32) as usize
            } else {
                i.min(len_after)
            };
            if (to, index) == (l, i) {
                return true;
            }
            out.push(f(Move {
                card: sel,
                from: l,
                to,
                index,
            }));
            return true;
        }
        let nl = (l as i32 + dl).clamp(0, self.lanes.len() as i32 - 1) as usize;
        let target = if dl != 0 {
            let cards = &self.lanes[nl].cards;
            cards
                .get(i.min(cards.len().saturating_sub(1)))
                .map(|c| c.id)
        } else {
            let cards = &self.lanes[l].cards;
            let ni = (i as i32 + di).clamp(0, cards.len() as i32 - 1) as usize;
            cards.get(ni).map(|c| c.id)
        };
        if let Some(t) = target {
            self.set(K_SEL, Some(t as i64));
        }
        true
    }
}

impl<M: 'static> Widget<M> for Kanban<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(self.lanes.len() as f32 * 220.0, 320.0)
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
                if let Some(card) = self.card_at(&b, cursor) {
                    let l = self.lane_at(&b, cursor).unwrap_or(0);
                    let rect = self
                        .place(&b, l, None, None)
                        .0
                        .into_iter()
                        .find(|(id, _)| *id == card)
                        .map(|(_, r)| r)
                        .unwrap_or_default();
                    self.set(K_PRESS, Some(card as i64));
                    self.set(K_SEL, Some(card as i64));
                    self.set(K_GX, Some(((cursor.x - rect.origin.x) * 8.0) as i64));
                    self.set(K_GY, Some(((cursor.y - rect.origin.y) * 8.0) as i64));
                    self.set(K_PX, Some((cursor.x * 8.0) as i64));
                    self.set(K_PY, Some((cursor.y * 8.0) as i64));
                    return true;
                }
                false
            }
            Event::CursorMoved(p) => {
                let Some(card) = self.slot(K_PRESS) else {
                    return false;
                };
                if self.dragging().is_none() {
                    let (sx, sy) = (
                        self.slot(K_PX).unwrap_or(0) as f32 / 8.0,
                        self.slot(K_PY).unwrap_or(0) as f32 / 8.0,
                    );
                    if (p.x - sx).hypot(p.y - sy) >= DRAG_START {
                        self.set(K_DRAG, Some(card));
                    }
                }
                true
            }
            Event::MouseReleased(MouseButton::Left) => {
                let Some(card) = self.slot(K_PRESS).map(|v| v as u64) else {
                    return false;
                };
                let was_drag = self.dragging().is_some();
                for k in [K_PRESS, K_DRAG, K_GX, K_GY, K_PX, K_PY] {
                    self.set(k, None);
                }
                if was_drag {
                    if let (Some(f), Some((from, _)), Some((to, index))) = (
                        self.on_move.clone(),
                        self.find(card),
                        self.drop_target(&b, card, cursor),
                    ) {
                        let at = self.find(card).map_or(0, |(_, i)| i);
                        if (from, at) != (to, index) {
                            out.push(f(Move {
                                card,
                                from,
                                to,
                                index,
                            }));
                        }
                    }
                } else if let Some(f) = self.on_open.clone() {
                    if self.card_at(&b, cursor) == Some(card) {
                        out.push(f(card));
                    }
                }
                true
            }
            Event::Scrolled { dy, .. } if b.contains(cursor) => {
                let Some(l) = self.lane_at(&b, cursor) else {
                    return false;
                };
                let max = self.max_scroll(&b, l, 0.0);
                let key = self.id ^ (0x5C00 + l as u64);
                let cur = self.ui.scroll_offset(key);
                let next = (cur - dy).clamp(0.0, max);
                self.ui.set_scroll_offset(key, next);
                next != cur
            }
            Event::KeyPressed(Key::Named(n), m) if self.ui.is_focused(Some(self.id)) => {
                let (dl, di) = match n.as_str() {
                    "ArrowLeft" => (-1, 0),
                    "ArrowRight" => (1, 0),
                    "ArrowUp" => (0, -1),
                    "ArrowDown" => (0, 1),
                    _ => return false,
                };
                self.key_move(out, dl, di, m.alt)
            }
            Event::KeyPressed(Key::Enter, _) if self.ui.is_focused(Some(self.id)) => {
                if let (Some(f), Some(sel)) = (self.on_open.clone(), self.selected()) {
                    out.push(f(sel));
                    return true;
                }
                false
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
        let b = layout.bounds;
        let cur = self.ui.cursor();
        let drag = self.dragging();
        let focused = self.ui.is_focused(Some(self.id));
        let target = drag.and_then(|c| self.drop_target(&b, c, cur));
        let dragged = drag.and_then(|c| {
            let (l, i) = self.find(c)?;
            Some(self.lanes[l].cards[i].clone())
        });
        let hover_card = if drag.is_none() {
            self.card_at(&b, cur)
        } else {
            None
        };
        self.ui.describe(
            Role::Label,
            &format!(
                "Kanban board, {}",
                self.lanes
                    .iter()
                    .map(|l| format!("{} {}", l.title, l.cards.len()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            &b,
            |_| {},
        );

        for (li, lane) in self.lanes.iter().enumerate() {
            let r = self.lane_rect(&b, li);
            let over = target.is_some_and(|(l, _)| l == li) && drag.is_some();
            let e = self.ui.ease(
                0x4B4E_0000 ^ ((self.id & 0xFFFF) << 8) ^ li as u64,
                over as u8 as f32,
            );
            out.push(DrawCommand::Quad {
                bounds: r,
                fill: mix(p.surface, p.accent.with_alpha(0.07), e),
                radius: t.radii.large,
                border: Some(Border {
                    color: mix(p.border, p.accent.with_alpha(0.55), e),
                    width: 1.0,
                }),
                shadow: None,
            });
            // Header: title and count.
            let ty = r.origin.y + (HEAD - lab.line_height) / 2.0;
            out.push(label_cmd(
                Point::new(r.origin.x + 12.0, ty),
                lane.title.clone(),
                &lab,
                600,
                p.text,
            ));
            let count = lane.cards.len().to_string();
            let tw = m.width_styled(&lane.title, lab.size, 600, alloy_ui_core::FontFamily::Sans);
            out.push(label_cmd(
                Point::new(
                    r.origin.x + 12.0 + tw + 8.0,
                    ty + (lab.line_height - cap.line_height) / 2.0,
                ),
                count,
                &cap,
                400,
                p.text_secondary,
            ));
            out.push(DrawCommand::PushClip(Rectangle::new(
                Point::new(r.origin.x, r.origin.y + HEAD),
                Size::new(r.size.width, r.size.height - HEAD),
            )));
            let gap = target
                .filter(|(l, _)| *l == li)
                .zip(dragged.as_ref())
                .map(|((_, i), c)| (i, self.card_h(c)));
            let (cards, hole) = self.place(&b, li, drag, gap);
            for (id, cr) in &cards {
                let Some(card) = lane.cards.iter().find(|c| c.id == *id) else {
                    continue;
                };
                self.draw_card(out, card, *cr, hover_card == Some(*id), false);
                if focused && self.selected() == Some(*id) && drag.is_none() {
                    out.push(util::focus_ring(*cr, t.radii.medium, p.focus_ring));
                }
            }
            if let Some(h) = hole {
                out.push(DrawCommand::Quad {
                    bounds: h,
                    fill: p.accent.with_alpha(0.10),
                    radius: t.radii.medium,
                    border: Some(Border {
                        color: p.accent.with_alpha(0.5),
                        width: 1.0,
                    }),
                    shadow: None,
                });
            }
            if lane.cards.is_empty() && drag.is_none() {
                let s = "No cards";
                let w = m.width_styled(s, cap.size, 400, alloy_ui_core::FontFamily::Sans);
                out.push(label_cmd(
                    Point::new(
                        r.origin.x + (r.size.width - w) / 2.0,
                        r.origin.y + HEAD + 12.0,
                    ),
                    s,
                    &cap,
                    400,
                    p.text_secondary,
                ));
            }
            out.push(DrawCommand::PopClip);
        }

        // The card in hand follows the pointer, above everything.
        if let Some(card) = dragged {
            let w = self.lane_w(&b) - 2.0 * PAD;
            let h = self.card_h(&card);
            let gx = self.slot(K_GX).unwrap_or(0) as f32 / 8.0;
            let gy = self.slot(K_GY).unwrap_or(0) as f32 / 8.0;
            let rect = Rectangle::new(Point::new(cur.x - gx, cur.y - gy), Size::new(w, h));
            let mut tmp = DrawList::default();
            self.draw_card(&mut tmp, &card, rect, true, true);
            self.ui.overlay(None, None, tmp.0);
        }
    }
}

impl<M: 'static> Kanban<M> {
    fn draw_card(&self, out: &mut DrawList, c: &Card, r: Rectangle, hot: bool, lifted: bool) {
        let t = self.ui.theme();
        let p = &t.palette;
        let cap = t.typography.caption;
        let lab = t.typography.label;
        let selected = self.selected() == Some(c.id);
        out.push(DrawCommand::Quad {
            bounds: r,
            fill: p.surface_raised,
            radius: t.radii.medium,
            border: Some(Border {
                color: if lifted || selected {
                    p.accent.with_alpha(0.8)
                } else if hot {
                    p.border_strong
                } else {
                    p.border
                },
                width: 1.0,
            }),
            shadow: lifted.then_some(t.elevation.high),
        });
        out.push(DrawCommand::PushClip(r));
        let mut y = r.origin.y + 10.0;
        out.push(label_cmd(
            Point::new(r.origin.x + 12.0, y),
            c.title.clone(),
            &lab,
            600,
            p.text,
        ));
        y += lab.line_height;
        if let Some(d) = &c.detail {
            y += 2.0;
            out.push(label_cmd(
                Point::new(r.origin.x + 12.0, y),
                d.clone(),
                &cap,
                400,
                p.text_secondary,
            ));
            y += cap.line_height;
        }
        if let Some((label, tone)) = &c.tag {
            y += 10.0;
            let ink = tone.color(&t);
            let w = self.ui.metrics().width_styled(
                label,
                cap.size,
                600,
                alloy_ui_core::FontFamily::Sans,
            ) + 14.0;
            let pill = Rectangle::new(
                Point::new(r.origin.x + 12.0, y - 2.0),
                Size::new(w, cap.line_height + 2.0),
            );
            out.push(DrawCommand::Quad {
                bounds: pill,
                fill: ink.with_alpha(0.12),
                radius: pill.size.height / 2.0,
                border: None,
                shadow: None,
            });
            out.push(label_cmd(
                Point::new(pill.origin.x + 7.0, y - 1.0),
                label.clone(),
                &cap,
                600,
                ink,
            ));
        }
        out.push(DrawCommand::PopClip);
    }
}
