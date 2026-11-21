//! Multi-line text editor field.

use crate::a11y::Role;
use crate::{
    text_edit::{
        apply_key, commit_edit, history_edit, line_range, nearest_boundary, sanitize, word_bounds,
        Edit, KeyOutcome,
    },
    ui::Ui,
    util::{self, focus_ring},
};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    Modifiers, MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// A scrolling multi-line text field. Controlled like [`crate::TextInput`]:
/// you own the string (lines joined by `\n`) and get the new one from
/// `on_input`. Enter inserts a newline; Tab is left to focus navigation.
/// Long lines wrap at word boundaries; `.wrap(false)` scrolls them sideways.
pub struct TextEditor<M> {
    ui: Ui,
    id: u64,
    value: String,
    placeholder: String,
    on_input: Option<Box<dyn Fn(String) -> M>>,
    width: Length,
    height: Length,
    wrap: bool,
}

impl<M: 'static> TextEditor<M> {
    /// An editor showing `value`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, value: impl Into<String>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            value: value.into(),
            placeholder: String::new(),
            on_input: None,
            width: Length::Fill(1),
            height: Length::Fill(1),
            wrap: true,
        }
    }
    /// Hint shown while empty.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// Called with the new text on every edit; without it the editor is read-only.
    pub fn on_input(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.on_input = Some(Box::new(f));
        self
    }
    /// Width sizing (default fills).
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Height sizing (default fills).
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// Wrap long lines (default) or scroll them sideways.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn pad(&self) -> f32 {
        self.ui.theme().spacing.md
    }
    fn size(&self) -> f32 {
        self.ui.theme().typography.code.size
    }
    fn line_h(&self) -> f32 {
        self.ui.theme().typography.code.line_height
    }
    fn width_of(&self, s: &str) -> f32 {
        self.ui
            .metrics()
            .width_styled(s, self.size(), 400, FontFamily::Mono)
    }
    /// The visual rows of the text for a field of bounds `b`.
    fn vis(&self, b: &Rectangle) -> Vis {
        let w = self
            .wrap
            .then(|| (b.size.width - 2.0 * self.pad()).max(1.0));
        Vis::new(&self.value, w, &|s| self.width_of(s))
    }
    fn max_scroll(&self, b: &Rectangle, lines: usize) -> f32 {
        (lines as f32 * self.line_h() - (b.size.height - 2.0 * self.pad())).max(0.0)
    }
    /// Character index under `p`.
    fn index_at(&self, b: &Rectangle, lines: &Vis, e_scroll_x: f32, p: Point) -> usize {
        let y = p.y - b.origin.y - self.pad() + self.ui.scroll_offset(self.id);
        let line = (y / self.line_h()).floor().max(0.0) as usize;
        let line = line.min(lines.len() - 1);
        let rel = p.x - b.origin.x - self.pad() + e_scroll_x;
        let size = self.size();
        let m = self.ui.metrics();
        let col = nearest_boundary(&lines.rows[line], rel, |s| m.width(s, size));
        lines.index_of(line, col)
    }
    /// Emit `new` as an edit, remembering the old state for undo.
    fn commit(&self, new: String, caret_before: usize, out: &mut Vec<M>) {
        commit_edit(
            &self.ui,
            self.id,
            &self.value,
            new,
            caret_before,
            &self.on_input,
            out,
        );
    }
    /// Undo/redo shortcut; returns true when the key was one.
    fn history(&self, e: &mut Edit, k: &Key, m: &Modifiers, out: &mut Vec<M>) -> bool {
        history_edit(
            &self.ui,
            self.id,
            &self.value,
            e,
            (k, m),
            &self.on_input,
            out,
        )
    }
}

impl<M: 'static> Widget<M> for TextEditor<M> {
    fn style(&self) -> Style {
        util::sized(self.width, self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        Size::new(240.0, 6.0 * self.line_h() + 2.0 * t.spacing.md)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let lines = self.vis(&b);
        let mut e = self.ui.edit(self.id);
        e.clamp(self.value.chars().count());
        let caret0 = e.caret;
        let editable = self.on_input.is_some();
        let focused = self.ui.is_focused(Some(self.id));
        let key = util::key_of(&b);
        let consumed = match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
                self.ui.set_pressed(Some(key));
                let i = self.index_at(&b, &lines, e.scroll_x, cursor);
                let now = self.ui.now();
                let near = (cursor.x - e.click_at.0).abs() <= 4.0
                    && (cursor.y - e.click_at.1).abs() <= 4.0;
                e.clicks = if e.clicks > 0 && near && now.saturating_sub(e.click_ms) < 500 {
                    e.clicks % 3 + 1
                } else {
                    1
                };
                (e.click_ms, e.click_at) = (now, (cursor.x, cursor.y));
                match e.clicks {
                    2 => {
                        let (s, z) = word_bounds(&self.value, i);
                        (e.anchor, e.caret) = (s, z);
                    }
                    3 => {
                        (e.anchor, e.caret) = line_range(&self.value, i);
                    }
                    _ => e.move_to(i, false),
                }
                e.goal_col = None;
                e.preedit.clear();
                true
            }
            Event::CursorMoved(p) if self.ui.pressed() == Some(key) => {
                let i = self.index_at(&b, &lines, e.scroll_x, *p);
                e.move_to(i, true);
                true
            }
            Event::MouseReleased(MouseButton::Left) => self.ui.pressed() == Some(key),
            Event::Scrolled { dy, .. } if b.contains(cursor) => {
                let max = self.max_scroll(&b, lines.len());
                let off = self.ui.scroll_offset(self.id);
                self.ui
                    .set_scroll_offset(self.id, (off - dy).clamp(0.0, max));
                (off - dy).clamp(0.0, max) != off
            }
            Event::ImePreedit(s) if focused => {
                e.preedit = s.clone();
                true
            }
            Event::TextInput(s) if focused => {
                e.preedit.clear();
                let clean = sanitize(s, true);
                if editable && !clean.is_empty() {
                    let v = e.insert(&self.value, &clean);
                    self.commit(v, caret0, out);
                }
                true
            }
            // While the IME composes it owns the keyboard (see TextInput).
            Event::KeyPressed(k, _) if focused && !e.preedit.is_empty() => !matches!(k, Key::Tab),
            Event::KeyPressed(k, m) if focused => {
                let (line, col) = lines.line_col(e.caret);
                let vertical = |e: &mut Edit, to_line: usize| {
                    let goal = e.goal_col.unwrap_or(col);
                    e.move_to(lines.index_of(to_line, goal), m.shift);
                    e.goal_col = Some(goal);
                };
                let page = (((b.size.height - 2.0 * self.pad()) / self.line_h()) as usize).max(1);
                let handled = match k {
                    Key::Tab => return false,
                    Key::Character(_) if editable && self.history(&mut e, k, m, out) => true,
                    Key::Escape => {
                        self.ui.set_focus(None);
                        true
                    }
                    Key::Enter if editable => {
                        let v = e.insert(&self.value, "\n");
                        self.commit(v, caret0, out);
                        true
                    }
                    Key::Named(n) => match n.as_str() {
                        "ArrowUp" => {
                            vertical(&mut e, line.saturating_sub(1));
                            true
                        }
                        "ArrowDown" => {
                            vertical(&mut e, line + 1);
                            true
                        }
                        "PageUp" => {
                            vertical(&mut e, line.saturating_sub(page));
                            true
                        }
                        "PageDown" => {
                            vertical(&mut e, line + page);
                            true
                        }
                        "Home" => {
                            let to = if m.ctrl || m.logo {
                                0
                            } else {
                                lines.index_of(line, 0)
                            };
                            e.move_to(to, m.shift);
                            e.goal_col = None;
                            true
                        }
                        "End" => {
                            let to = if m.ctrl || m.logo {
                                self.value.chars().count()
                            } else {
                                lines.index_of(line, usize::MAX)
                            };
                            e.move_to(to, m.shift);
                            e.goal_col = None;
                            true
                        }
                        _ => false,
                    },
                    _ => false,
                };
                if handled {
                    true
                } else {
                    match apply_key(
                        &mut e,
                        &self.value,
                        k,
                        m,
                        &*self.ui.clipboard(),
                        editable,
                        true,
                    ) {
                        KeyOutcome::Ignored => false,
                        KeyOutcome::Handled => {
                            e.goal_col = None;
                            true
                        }
                        KeyOutcome::Changed(v) => {
                            e.goal_col = None;
                            self.commit(v, caret0, out);
                            true
                        }
                    }
                }
            }
            _ => false,
        };
        if consumed {
            e.blink_epoch = self.ui.now();
        }
        self.ui.set_edit(self.id, e);
        consumed
    }
    fn animating(&self) -> bool {
        self.on_input.is_some() && self.ui.is_focused(Some(self.id))
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        self.ui
            .describe(Role::TextEditor, &self.placeholder, &b, |n| {
                n.id = Some(self.id);
                n.value = Some(self.value.clone());
                n.enabled = self.on_input.is_some();
            });
        let focused = self.ui.is_focused(Some(self.id));
        let editable = self.on_input.is_some();
        self.ui.register(self.id);
        let lines = self.vis(&b);
        let mut e = self.ui.edit(self.id);
        e.clamp(self.value.chars().count());
        let (pad, lh, size) = (self.pad(), self.line_h(), self.size());
        let (cl, cc) = lines.line_col(e.caret);
        let col_x = |line: usize, col: usize| -> f32 {
            let s: String = lines.rows[line].chars().take(col).collect();
            self.width_of(&s)
        };

        // Keep the caret in view, vertically and horizontally.
        let view_h = (b.size.height - 2.0 * pad).max(lh);
        let inner_w = (b.size.width - 2.0 * pad).max(0.0);
        let mut off = self
            .ui
            .scroll_offset(self.id)
            .min(self.max_scroll(&b, lines.len()));
        if focused {
            if (cl as f32) * lh < off {
                off = cl as f32 * lh;
            }
            if (cl as f32 + 1.0) * lh > off + view_h {
                off = (cl as f32 + 1.0) * lh - view_h;
            }
        }
        self.ui.set_scroll_offset(self.id, off);
        let caret_x = col_x(cl, cc) + self.width_of(&e.preedit);
        if caret_x - e.scroll_x > inner_w {
            e.scroll_x = caret_x - inner_w;
        }
        if caret_x < e.scroll_x {
            e.scroll_x = caret_x;
        }
        e.scroll_x = if self.wrap { 0.0 } else { e.scroll_x.max(0.0) };
        self.ui.set_edit(self.id, e.clone());

        if focused {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: if editable {
                p.surface_raised
            } else {
                p.surface
            },
            radius: t.radii.medium,
            border: Some(Border {
                color: crate::util::mix(
                    p.border,
                    p.border_strong,
                    self.ui.ease(
                        Ui::anim_key(&b, 7),
                        (focused || self.ui.hovered(&b)) as u8 as f32,
                    ),
                ),
                width: 1.0,
            }),
            shadow: None,
        });
        out.push(DrawCommand::PushClip(Rectangle::new(
            Point::new(b.origin.x + pad, b.origin.y + pad),
            Size::new(inner_w, view_h),
        )));
        let ox = b.origin.x + pad - e.scroll_x;
        let oy = b.origin.y + pad - off;
        if self.value.is_empty() && e.preedit.is_empty() {
            out.push(DrawCommand::Text {
                family: FontFamily::Mono,
                weight: 400,
                line_height: Some(lh),
                origin: Point::new(ox, oy),
                content: self.placeholder.clone(),
                size,
                color: p.text_secondary,
                max_width: None,
            });
        }
        let first = (off / lh).floor() as usize;
        let last = (((off + view_h) / lh).ceil() as usize).min(lines.len());
        let (sa, sz) = e.range();
        for (i, line) in lines.rows.iter().enumerate().take(last).skip(first) {
            let y = oy + i as f32 * lh;
            if focused && e.has_selection() {
                let (st, n) = (lines.starts[i], line.chars().count());
                if sa <= st + n && sz >= st {
                    let from = col_x(i, sa.saturating_sub(st));
                    let to = if sz > st + n {
                        // A hard break shows a small stub for the newline.
                        col_x(i, n) + if lines.hard[i] { size * 0.4 } else { 0.0 }
                    } else {
                        col_x(i, sz - st)
                    };
                    if to > from {
                        out.push(DrawCommand::Quad {
                            bounds: Rectangle::new(
                                Point::new(ox + from, y),
                                Size::new(to - from, lh),
                            ),
                            fill: p.selection,
                            radius: 0.0,
                            border: None,
                            shadow: None,
                        });
                    }
                }
            }
            if !line.is_empty() {
                out.push(DrawCommand::Text {
                    family: FontFamily::Mono,
                    weight: 400,
                    line_height: Some(lh),
                    origin: Point::new(ox, y),
                    content: line.clone(),
                    size,
                    color: if editable { p.text } else { p.text_secondary },
                    max_width: None,
                });
            }
        }
        if !e.preedit.is_empty() {
            let x = ox + col_x(cl, cc);
            let w = self.width_of(&e.preedit);
            let y = oy + cl as f32 * lh;
            out.push(DrawCommand::Text {
                family: FontFamily::Mono,
                weight: 400,
                line_height: Some(lh),
                origin: Point::new(x, y),
                content: e.preedit.clone(),
                size,
                color: p.text,
                max_width: None,
            });
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(x, y + lh - 2.0), Size::new(w, 1.0)),
                fill: p.text,
                radius: 0.0,
                border: None,
                shadow: None,
            });
        }
        let blink_on = (self.ui.now().saturating_sub(e.blink_epoch) / 530) % 2 == 0;
        if focused && editable && blink_on {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(ox + caret_x, oy + cl as f32 * lh),
                    Size::new(1.5, lh),
                ),
                fill: p.text,
                radius: 0.0,
                border: None,
                shadow: None,
            });
        }
        out.push(DrawCommand::PopClip);
    }
}

/// The text laid out in visual rows. A soft row (a wrap) continues straight
/// into the next in character indices; a hard row is followed by a `\n`.
struct Vis {
    rows: Vec<String>,
    starts: Vec<usize>,
    hard: Vec<bool>,
}

impl Vis {
    fn new(value: &str, width: Option<f32>, w: &dyn Fn(&str) -> f32) -> Self {
        let mut rows: Vec<String> = Vec::new();
        let mut hard = Vec::new();
        for para in value.split('\n') {
            let mut row = String::new();
            for word in para.split_inclusive(' ') {
                let Some(max) = width else {
                    row.push_str(word);
                    continue;
                };
                if !row.is_empty() && w(format!("{row}{word}").trim_end()) > max {
                    rows.push(std::mem::take(&mut row));
                    hard.push(false);
                }
                if w(word.trim_end()) > max {
                    for ch in word.chars() {
                        if !row.is_empty() && w(&format!("{row}{ch}")) > max {
                            rows.push(std::mem::take(&mut row));
                            hard.push(false);
                        }
                        row.push(ch);
                    }
                } else {
                    row.push_str(word);
                }
            }
            rows.push(row);
            hard.push(true);
        }
        let mut starts = Vec::with_capacity(rows.len());
        let mut at = 0;
        for (r, h) in rows.iter().zip(&hard) {
            starts.push(at);
            at += r.chars().count() + *h as usize;
        }
        Self { rows, starts, hard }
    }
    fn len(&self) -> usize {
        self.rows.len()
    }
    /// The `(row, column)` of character index `idx`.
    fn line_col(&self, idx: usize) -> (usize, usize) {
        let i = self.starts.partition_point(|&s| s <= idx).saturating_sub(1);
        (i, idx - self.starts[i])
    }
    /// The character index of `(row, col)`. On a wrapped row the column stops
    /// before the break so the caret stays on that row.
    fn index_of(&self, row: usize, col: usize) -> usize {
        let row = row.min(self.len() - 1);
        let n = self.rows[row].chars().count();
        let max = if self.hard[row] {
            n
        } else {
            n.saturating_sub(1)
        };
        self.starts[row] + col.min(max)
    }
}

#[cfg(test)]
mod tests {
    use super::Vis;

    #[test]
    fn row_math_round_trips() {
        let v = Vis::new("ab\n\ncde", None, &|s| s.len() as f32);
        assert_eq!(v.rows, ["ab", "", "cde"]);
        for i in 0..=7 {
            let (r, c) = v.line_col(i);
            assert_eq!(v.index_of(r, c), i);
        }
        assert_eq!(v.index_of(0, 50), 2, "column clamps to row length");
        assert_eq!(v.index_of(9, 0), 4, "row clamps to the last");
    }

    #[test]
    fn soft_wraps_keep_every_character_and_stay_on_their_row() {
        let v = Vis::new("aa bb cc dd", Some(5.0), &|s| s.len() as f32);
        assert_eq!(v.rows.concat(), "aa bb cc dd");
        assert_eq!(v.rows, ["aa bb ", "cc dd"]);
        assert_eq!(v.line_col(6), (1, 0), "wrap point belongs to the next row");
        assert_eq!(
            v.index_of(0, 99),
            5,
            "End on a soft row stops before the break"
        );
        let long = Vis::new("abcdefgh", Some(3.0), &|s| s.len() as f32);
        assert_eq!(long.rows, ["abc", "def", "gh"]);
    }
}
