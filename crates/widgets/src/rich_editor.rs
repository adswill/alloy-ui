//! A rich-text editor: bold, italic, underline, code, links, headings, lists and quotes.

use crate::{
    rich_text::{BlockKind, Mark, Marks, RichText},
    text_edit::{
        apply_key, line_range, nearest_boundary, next_grapheme, prev_grapheme, primary, sanitize,
        word_bounds, word_left, word_right, Edit, KeyOutcome,
    },
    ui::Ui,
    util::{self, focus_ring, mix},
    Role,
};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

const TOOLBAR_H: f32 = 42.0;
const PAD_X: f32 = 28.0;
const PAD_Y: f32 = 20.0;
const MAX_W: f32 = 760.0;
const LINK_W: f32 = 340.0;
const LINK_H: f32 = 36.0;
const BAR: f32 = 8.0;
const K_DRAG: u64 = 0x_41c4_0001 << 20;

/// A WYSIWYG editor over a [`RichText`] you own. Edits arrive as a whole new document through
/// `on_change`; `RichText::to_markdown` and `from_markdown` save and load it.
///
/// Select text and use the toolbar or `Ctrl/Cmd+B`, `I`, `U`, `E` (code) and `K` (link).
/// Typing `- `, `1. `, `# `, `## ` or `> ` at the start of a paragraph turns it into a list item,
/// heading or quote; Enter on an empty item ends the list. Ctrl/Cmd-click opens a link
/// (a plain click when the editor is read-only).
pub struct RichEditor<M> {
    ui: Ui,
    id: u64,
    doc: RichText,
    placeholder: String,
    on_change: Option<Rc<dyn Fn(RichText) -> M>>,
    on_link: Option<Rc<dyn Fn(String) -> M>>,
    toolbar: bool,
    width: Length,
    height: Length,
}

impl<M: 'static> RichEditor<M> {
    /// An editor showing `doc`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, doc: &RichText) -> Self {
        Self {
            ui: ui.clone(),
            id,
            doc: doc.clone(),
            placeholder: String::new(),
            on_change: None,
            on_link: None,
            toolbar: true,
            width: Length::Fill(1),
            height: Length::Fill(1),
        }
    }
    /// Called with the new document on every edit; without it the editor is read-only.
    pub fn on_change(mut self, f: impl Fn(RichText) -> M + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
    /// Called with the target of an opened link.
    pub fn on_link(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.on_link = Some(Rc::new(f));
        self
    }
    /// Hint shown while the document is empty.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// Show or hide the formatting toolbar (default shown when editable).
    pub fn toolbar(mut self, on: bool) -> Self {
        self.toolbar = on;
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
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn editable(&self) -> bool {
        self.on_change.is_some()
    }
    fn tool_h(&self) -> f32 {
        if self.toolbar && self.editable() {
            TOOLBAR_H
        } else {
            0.0
        }
    }
    fn base(&self) -> f32 {
        (self.ui.theme().typography.body.size + 1.0).round()
    }
    /// The scrolling area: the widget minus its toolbar and border.
    fn view(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x + 1.0, b.origin.y + 1.0 + self.tool_h()),
            Size::new(
                (b.size.width - 2.0).max(0.0),
                (b.size.height - 2.0 - self.tool_h()).max(0.0),
            ),
        )
    }
    fn content_w(&self, b: &Rectangle) -> f32 {
        (b.size.width - 2.0 * PAD_X).clamp(80.0, MAX_W)
    }
    fn content_x(&self, b: &Rectangle) -> f32 {
        b.origin.x + ((b.size.width - self.content_w(b)) / 2.0).max(PAD_X.min(b.size.width / 4.0))
    }
    fn laid(&self, b: &Rectangle) -> Rc<Laid> {
        Laid::get(self.id, &self.doc, self.content_w(b), self.base(), &self.ui)
    }
    fn max_scroll(&self, b: &Rectangle, l: &Laid) -> f32 {
        (l.height + 2.0 * PAD_Y - self.view(b).size.height).max(0.0)
    }
    fn offset(&self, b: &Rectangle, l: &Laid) -> f32 {
        self.ui
            .scroll_offset(self.id)
            .clamp(0.0, self.max_scroll(b, l))
    }
    /// Top-left of the document in window coordinates.
    fn origin(&self, b: &Rectangle, l: &Laid) -> Point {
        Point::new(
            self.content_x(b),
            self.view(b).origin.y + PAD_Y - self.offset(b, l),
        )
    }
    /// Flat index under `p`.
    fn index_at(&self, b: &Rectangle, l: &Laid, p: Point) -> usize {
        let o = self.origin(b, l);
        l.index_at(&self.ui, p.x - o.x, p.y - o.y)
    }

    // ------------------------------------------------------------ toolbar

    fn tools(&self, b: &Rectangle) -> Vec<(Tool, Rectangle)> {
        use Tool::*;
        let mut x = b.origin.x + 10.0;
        let y = b.origin.y + 7.0;
        let mut out = Vec::new();
        let groups: [&[(Tool, f32)]; 4] = [
            &[
                (Bold, 30.0),
                (Italic, 30.0),
                (Underline, 30.0),
                (Code, 36.0),
            ],
            &[(Link, 44.0)],
            &[(Head(1), 32.0), (Head(2), 32.0)],
            &[(Bullet, 30.0), (Numbered, 30.0), (Quote, 30.0)],
        ];
        for (gi, g) in groups.iter().enumerate() {
            if gi > 0 {
                x += 11.0;
            }
            for &(t, w) in *g {
                out.push((t, Rectangle::new(Point::new(x, y), Size::new(w, 28.0))));
                x += w + 2.0;
            }
        }
        out
    }
    fn separators(&self, b: &Rectangle) -> Vec<f32> {
        let t = self.tools(b);
        [Tool::Code, Tool::Link, Tool::Head(2)]
            .iter()
            .filter_map(|k| t.iter().find(|x| x.0 == *k))
            .map(|(_, r)| r.origin.x + r.size.width + 5.5)
            .collect()
    }
    fn link_rect(&self, b: &Rectangle) -> Rectangle {
        let t = self.tools(b);
        let link = t
            .iter()
            .find(|x| x.0 == Tool::Link)
            .map_or(b.origin.x + 100.0, |x| x.1.origin.x);
        Rectangle::new(
            Point::new(
                link.min(b.origin.x + b.size.width - LINK_W - 12.0)
                    .max(b.origin.x + 8.0),
                b.origin.y + TOOLBAR_H + 4.0,
            ),
            Size::new(LINK_W, LINK_H),
        )
    }
    fn is_active(&self, t: Tool, e: &Edit, st: &RichState) -> bool {
        let (a, z) = e.range();
        let doc = &self.doc;
        match t {
            Tool::Bold | Tool::Italic | Tool::Underline | Tool::Code => {
                let m = t.mark().unwrap_or(Mark::Bold);
                if a == z {
                    match st.pending {
                        Some((at, marks)) if at == a => marks.has(m),
                        _ => doc.style_at(a).0.has(m),
                    }
                } else {
                    doc.all_marked(a, z, m)
                }
            }
            Tool::Link => doc.link_at(a).is_some() || (a != z && doc.link_at(z).is_some()),
            Tool::Head(l) => doc.kind_of(a, z) == Some(BlockKind::Heading(l)),
            Tool::Bullet => doc.kind_of(a, z) == Some(BlockKind::Bullet),
            Tool::Numbered => doc.kind_of(a, z) == Some(BlockKind::Numbered),
            Tool::Quote => doc.kind_of(a, z) == Some(BlockKind::Quote),
        }
    }

    // ------------------------------------------------------------ editing

    /// Commit `new` as the next document, with undo bookkeeping. `typing` edits within a second
    /// of each other share one undo step.
    fn commit(&self, st: &mut RichState, e: &Edit, new: RichText, typing: bool, out: &mut Vec<M>) {
        let Some(f) = &self.on_change else { return };
        let now = self.ui.now();
        let coalesce = typing && st.last_typing && now.saturating_sub(st.last_ms) < 1000;
        if !coalesce {
            st.undo.push(Snap {
                doc: self.doc.clone(),
                caret: st.caret_before,
                anchor: st.anchor_before,
            });
            if st.undo.len() > 200 {
                st.undo.remove(0);
            }
        }
        st.redo.clear();
        st.last_typing = typing;
        st.last_ms = now;
        let _ = e;
        out.push(f(new));
    }
    fn undo(&self, st: &mut RichState, e: &mut Edit, redo: bool, out: &mut Vec<M>) {
        let Some(f) = &self.on_change else { return };
        let snap = if redo { st.redo.pop() } else { st.undo.pop() };
        if let Some(s) = snap {
            let cur = Snap {
                doc: self.doc.clone(),
                caret: e.caret,
                anchor: e.anchor,
            };
            if redo {
                st.undo.push(cur);
            } else {
                st.redo.push(cur);
            }
            e.caret = s.caret.min(s.doc.len());
            e.anchor = s.anchor.min(s.doc.len());
            st.last_typing = false;
            out.push(f(s.doc));
        }
    }
    /// Replace the selection with `text`, typed in the current style.
    fn type_text(&self, st: &mut RichState, e: &mut Edit, text: &str, out: &mut Vec<M>) {
        let (a, z) = e.range();
        let (marks, link) = match st.pending {
            Some((at, m)) if at == a && a == z => (m, None),
            _ => self.doc.style_at(a),
        };
        let mut d = self.doc.clone();
        d.replace(a, z, text, marks, link.as_deref());
        e.caret = a + text.chars().count();
        e.anchor = e.caret;
        st.pending = None;
        self.commit(st, e, d, !text.is_empty() && !text.contains('\n'), out);
    }
    fn toggle(&self, st: &mut RichState, e: &mut Edit, t: Tool, out: &mut Vec<M>) {
        let (a, z) = e.range();
        let mut d = self.doc.clone();
        match t {
            Tool::Bold | Tool::Italic | Tool::Underline | Tool::Code => {
                let m = t.mark().unwrap_or(Mark::Bold);
                if a == z {
                    let cur = match st.pending {
                        Some((at, p)) if at == a => p,
                        _ => self.doc.style_at(a).0,
                    };
                    let mut next = cur;
                    next.set(m, !cur.has(m));
                    st.pending = Some((a, next));
                    return;
                }
                d.toggle_mark(a, z, m);
            }
            Tool::Head(l) => d.toggle_kind(a, z, BlockKind::Heading(l)),
            Tool::Bullet => d.toggle_kind(a, z, BlockKind::Bullet),
            Tool::Numbered => d.toggle_kind(a, z, BlockKind::Numbered),
            Tool::Quote => d.toggle_kind(a, z, BlockKind::Quote),
            Tool::Link => {
                // Open the link bar on the selection, or on the link under the caret.
                let (la, lz, url) = match self.doc.link_at(a) {
                    Some((s, z2, u)) if a == z => (s, z2, u),
                    _ => (a, z, String::new()),
                };
                if la != lz {
                    st.link = Some(LinkBar {
                        text: url,
                        range: (la, lz),
                        fresh: true,
                    });
                }
                return;
            }
        }
        self.commit(st, e, d, false, out);
    }
    /// Move vertically by `dy` visual lines, keeping the goal x.
    fn vertical(&self, b: &Rectangle, l: &Laid, e: &mut Edit, dy: isize, extend: bool) {
        let (li, x) = l.caret_pos(&self.ui, e.caret);
        let goal = e.goal_col.map_or(x, |g| g as f32 / 16.0);
        let to = (li as isize + dy).clamp(0, l.lines.len() as isize - 1) as usize;
        let to_idx = if to == li && dy != 0 {
            if dy < 0 {
                0
            } else {
                self.doc.len()
            }
        } else {
            l.index_in_line(&self.ui, to, goal)
        };
        e.move_to(to_idx, extend);
        e.goal_col = Some((goal * 16.0) as usize);
        let _ = b;
    }
}

impl<M: 'static> Widget<M> for RichEditor<M> {
    fn style(&self) -> Style {
        util::sized(self.width, self.height)
    }
    fn measure(&self, _a: Size) -> Size {
        Size::new(360.0, 220.0 + self.tool_h())
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
        let l = self.laid(&b);
        let mut e = self.ui.edit(self.id);
        e.clamp(self.doc.len());
        let mut st: RichState = self.ui.state(self.id, |s: &mut RichState| s.clone());
        st.caret_before = e.caret;
        st.anchor_before = e.anchor;
        let focused = self.ui.is_focused(Some(self.id));
        let editable = self.editable();
        let view = self.view(&b);
        let pkey = util::key_of(&b);
        let consumed = 'ev: {
            match event {
                Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                    self.ui.set_focus(Some(self.id));
                    if editable && self.tool_h() > 0.0 && cursor.y < b.origin.y + TOOLBAR_H {
                        if let Some((t, _)) =
                            self.tools(&b).into_iter().find(|x| x.1.contains(cursor))
                        {
                            self.toggle(&mut st, &mut e, t, out);
                        }
                        break 'ev true;
                    }
                    if st.link.is_some() {
                        if self.link_rect(&b).contains(cursor) {
                            break 'ev true;
                        }
                        st.link = None;
                    }
                    // Scrollbar.
                    let max = self.max_scroll(&b, &l);
                    if max > 0.0 && cursor.x >= view.origin.x + view.size.width - BAR - 6.0 {
                        let h = view.size.height;
                        let th = (h * h / (l.height + 2.0 * PAD_Y)).clamp(28.0, h);
                        self.ui
                            .set_slot(self.id ^ K_DRAG, Some((th / 2.0 * 16.0) as i64));
                        let top = (cursor.y - view.origin.y - th / 2.0).clamp(0.0, h - th);
                        self.ui
                            .set_scroll_offset(self.id, top / (h - th).max(1.0) * max);
                        break 'ev true;
                    }
                    let i = self.index_at(&b, &l, cursor);
                    let open = {
                        let m = self.ui.modifiers();
                        (m.ctrl || m.logo) || !editable
                    };
                    if open {
                        if let (Some((_, _, url)), Some(f)) = (self.doc.link_at(i), &self.on_link) {
                            // Only when the click is on the linked text itself.
                            let o = self.origin(&b, &l);
                            if l.hits_link(&self.ui, cursor.x - o.x, cursor.y - o.y) {
                                out.push(f(url));
                                break 'ev true;
                            }
                        }
                    }
                    self.ui.set_pressed(Some(pkey));
                    let now = self.ui.now();
                    let near = (cursor.x - e.click_at.0).abs() <= 4.0
                        && (cursor.y - e.click_at.1).abs() <= 4.0;
                    e.clicks = if e.clicks > 0 && near && now.saturating_sub(e.click_ms) < 500 {
                        e.clicks % 3 + 1
                    } else {
                        1
                    };
                    (e.click_ms, e.click_at) = (now, (cursor.x, cursor.y));
                    let plain = self.doc.plain();
                    match e.clicks {
                        2 => (e.anchor, e.caret) = word_bounds(&plain, i),
                        3 => (e.anchor, e.caret) = line_range(&plain, i),
                        _ => e.move_to(i, self.ui.modifiers().shift),
                    }
                    e.goal_col = None;
                    e.preedit.clear();
                    st.pending = None;
                    true
                }
                Event::CursorMoved(p) if self.ui.slot(self.id ^ K_DRAG).is_some() => {
                    let h = view.size.height;
                    let th = (h * h / (l.height + 2.0 * PAD_Y)).clamp(28.0, h);
                    let top = (p.y - view.origin.y - th / 2.0).clamp(0.0, h - th);
                    self.ui.set_scroll_offset(
                        self.id,
                        top / (h - th).max(1.0) * self.max_scroll(&b, &l),
                    );
                    return true;
                }
                Event::CursorMoved(p) if self.ui.pressed() == Some(pkey) => {
                    let mut off = self.offset(&b, &l);
                    if p.y < view.origin.y {
                        off -= 24.0;
                    } else if p.y > view.origin.y + view.size.height {
                        off += 24.0;
                    }
                    self.ui
                        .set_scroll_offset(self.id, off.clamp(0.0, self.max_scroll(&b, &l)));
                    let i = self.index_at(&b, &l, *p);
                    e.move_to(i, true);
                    true
                }
                Event::MouseReleased(MouseButton::Left) => {
                    if self.ui.slot(self.id ^ K_DRAG).is_some() {
                        self.ui.set_slot(self.id ^ K_DRAG, None);
                        return true;
                    }
                    self.ui.pressed() == Some(pkey)
                }
                Event::Scrolled { dy, .. } if b.contains(cursor) => {
                    let before = self.offset(&b, &l);
                    self.ui.set_scroll_offset(
                        self.id,
                        (before - dy).clamp(0.0, self.max_scroll(&b, &l)),
                    );
                    self.offset(&b, &l) != before
                }
                Event::ImePreedit(s) if focused && editable => {
                    e.preedit = s.clone();
                    true
                }
                Event::TextInput(s) if focused && st.link.is_some() => {
                    let lb = st.link.as_mut().expect("checked");
                    if lb.fresh {
                        lb.text.clear();
                        lb.fresh = false;
                    }
                    lb.text.push_str(&sanitize(s, false));
                    true
                }
                Event::TextInput(s) if focused => {
                    e.preedit.clear();
                    let clean = sanitize(s, true);
                    if editable && !clean.is_empty() {
                        if clean == " " && self.markdown_shortcut(&mut st, &mut e, out) {
                            break 'ev true;
                        }
                        self.type_text(&mut st, &mut e, &clean, out);
                    }
                    true
                }
                Event::KeyPressed(k, m) if focused && st.link.is_some() => {
                    match k {
                        Key::Escape => st.link = None,
                        Key::Enter => {
                            let lb = st.link.take().expect("checked");
                            let url = lb.text.trim().to_string();
                            let mut d = self.doc.clone();
                            d.set_link(
                                lb.range.0,
                                lb.range.1,
                                (!url.is_empty()).then_some(url.as_str()),
                            );
                            self.commit(&mut st, &e, d, false, out);
                        }
                        Key::Backspace => {
                            let lb = st.link.as_mut().expect("checked");
                            if lb.fresh {
                                lb.text.clear();
                            } else {
                                lb.text.pop();
                            }
                            lb.fresh = false;
                        }
                        Key::Character(c) if primary(m) && c.eq_ignore_ascii_case("v") => {
                            if let Some(t) = self.ui.clipboard().get() {
                                let lb = st.link.as_mut().expect("checked");
                                if lb.fresh {
                                    lb.text.clear();
                                }
                                lb.fresh = false;
                                lb.text.push_str(sanitize(&t, false).trim());
                            }
                        }
                        _ => {}
                    }
                    true
                }
                Event::KeyPressed(k, _) if focused && !e.preedit.is_empty() => {
                    !matches!(k, Key::Tab)
                }
                Event::KeyPressed(k, m) if focused => {
                    let cmd = primary(m);
                    let plain = self.doc.plain();
                    let (a, z) = e.range();
                    let page = (view.size.height / 24.0) as isize;
                    let handled = match k {
                        Key::Character(c) if cmd && editable => {
                            let c = c.to_lowercase();
                            match c.as_str() {
                                "b" => self.toggle(&mut st, &mut e, Tool::Bold, out),
                                "i" => self.toggle(&mut st, &mut e, Tool::Italic, out),
                                "u" => self.toggle(&mut st, &mut e, Tool::Underline, out),
                                "e" => self.toggle(&mut st, &mut e, Tool::Code, out),
                                "k" => self.toggle(&mut st, &mut e, Tool::Link, out),
                                "z" => self.undo(&mut st, &mut e, m.shift, out),
                                "y" => self.undo(&mut st, &mut e, true, out),
                                "8" | "*" if m.shift => {
                                    self.toggle(&mut st, &mut e, Tool::Bullet, out)
                                }
                                "7" | "&" if m.shift => {
                                    self.toggle(&mut st, &mut e, Tool::Numbered, out)
                                }
                                "1" | "!" => self.toggle(&mut st, &mut e, Tool::Head(1), out),
                                "2" | "@" => self.toggle(&mut st, &mut e, Tool::Head(2), out),
                                "x" => {
                                    if a != z {
                                        self.ui.clipboard().set(self.doc.slice(a, z));
                                        self.type_text(&mut st, &mut e, "", out);
                                    }
                                }
                                "v" => {
                                    if let Some(t) = self.ui.clipboard().get() {
                                        let t = sanitize(&t, true);
                                        if !t.is_empty() {
                                            self.type_text(&mut st, &mut e, &t, out);
                                        }
                                    }
                                }
                                _ => {
                                    if let KeyOutcome::Ignored = apply_key(
                                        &mut e,
                                        &plain,
                                        k,
                                        m,
                                        &*self.ui.clipboard(),
                                        false,
                                        true,
                                    ) {
                                        break 'ev false;
                                    }
                                }
                            }
                            true
                        }
                        Key::Escape => {
                            self.ui.set_focus(None);
                            true
                        }
                        Key::Tab => false,
                        Key::Enter if editable => {
                            let empty_item = a == z
                                && self
                                    .doc
                                    .kind_of(a, a)
                                    .is_some_and(|k| k != BlockKind::Paragraph)
                                && self.doc.blocks[self.doc.at(a).0].is_empty();
                            if empty_item {
                                let mut d = self.doc.clone();
                                d.set_kind(a, a, BlockKind::Paragraph);
                                self.commit(&mut st, &e, d, false, out);
                            } else {
                                self.type_text(&mut st, &mut e, "\n", out);
                            }
                            true
                        }
                        Key::Backspace if editable => {
                            let (bi, off) = self.doc.at(a);
                            if a != z {
                                self.type_text(&mut st, &mut e, "", out);
                            } else if off == 0 && self.doc.blocks[bi].kind != BlockKind::Paragraph {
                                let mut d = self.doc.clone();
                                d.set_kind(a, a, BlockKind::Paragraph);
                                self.commit(&mut st, &e, d, false, out);
                            } else if a > 0 {
                                e.anchor = if m.alt || m.ctrl {
                                    word_left(&plain, a)
                                } else {
                                    prev_grapheme(&plain, a)
                                };
                                self.type_text(&mut st, &mut e, "", out);
                            }
                            true
                        }
                        Key::Named(n) => match n.as_str() {
                            "Delete" if editable => {
                                if a == z && a < self.doc.len() {
                                    e.anchor = if m.alt || m.ctrl {
                                        word_right(&plain, a)
                                    } else {
                                        next_grapheme(&plain, a)
                                    };
                                }
                                self.type_text(&mut st, &mut e, "", out);
                                true
                            }
                            "ArrowUp" => {
                                self.vertical(&b, &l, &mut e, -1, m.shift);
                                true
                            }
                            "ArrowDown" => {
                                self.vertical(&b, &l, &mut e, 1, m.shift);
                                true
                            }
                            "PageUp" => {
                                self.vertical(&b, &l, &mut e, -page, m.shift);
                                true
                            }
                            "PageDown" => {
                                self.vertical(&b, &l, &mut e, page, m.shift);
                                true
                            }
                            "Home" | "End" => {
                                let home = n == "Home";
                                let to = if cmd {
                                    if home {
                                        0
                                    } else {
                                        self.doc.len()
                                    }
                                } else {
                                    let (li, _) = l.caret_pos(&self.ui, e.caret);
                                    let line = &l.lines[li];
                                    if home {
                                        line.start
                                    } else {
                                        l.line_end(li)
                                    }
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
                        match apply_key(&mut e, &plain, k, m, &*self.ui.clipboard(), false, true) {
                            KeyOutcome::Ignored => false,
                            _ => {
                                e.goal_col = None;
                                true
                            }
                        }
                    }
                }
                _ => false,
            }
        };
        if consumed {
            e.blink_epoch = self.ui.now();
            if e.caret != st.caret_before {
                // Moving the caret drops a pending style that was meant for the old spot.
                if !matches!(st.pending, Some((at, _)) if at == e.caret) {
                    st.pending = st.pending.filter(|p| p.0 == e.caret);
                }
            }
        }
        self.ui.state(self.id, |s: &mut RichState| *s = st);
        self.ui.set_edit(self.id, e);
        consumed
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let l = self.laid(&b);
        let focused = self.ui.is_focused(Some(self.id));
        let editable = self.editable();
        self.ui.register(self.id);
        self.ui
            .describe(Role::TextEditor, "Rich text editor", &b, |n| {
                n.id = Some(self.id);
                n.value = Some(self.doc.plain());
                n.enabled = editable;
            });
        let mut e = self.ui.edit(self.id);
        e.clamp(self.doc.len());
        let mut st: RichState = self.ui.state(self.id, |s: &mut RichState| s.clone());
        let view = self.view(&b);
        let (a, z) = e.range();

        // Keep the caret in view when it (or the text) moved, not on every frame.
        let marker = e.caret ^ (self.doc.len() << 20) ^ (l.lines.len() << 40);
        let mut off = self.offset(&b, &l);
        if st.last != marker {
            if focused {
                let (li, _) = l.caret_pos(&self.ui, e.caret);
                let line = &l.lines[li];
                if line.y < off {
                    off = line.y;
                }
                if line.y + line.h + 2.0 * PAD_Y > off + view.size.height {
                    off = line.y + line.h + 2.0 * PAD_Y - view.size.height;
                }
            }
            st.last = marker;
        }
        off = off.clamp(0.0, self.max_scroll(&b, &l));
        self.ui.set_scroll_offset(self.id, off);

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
                        Ui::anim_key(&b, 17),
                        (focused || self.ui.hovered(&b)) as u8 as f32,
                    ),
                ),
                width: 1.0,
            }),
            shadow: None,
        });
        let quad = |r: Rectangle, c: Color, radius: f32| DrawCommand::Quad {
            bounds: r,
            fill: c,
            radius,
            border: None,
            shadow: None,
        };
        let o = self.origin(&b, &l);
        out.push(DrawCommand::PushClip(view));
        let outer = self.ui.clip();
        self.ui.set_clip(Some(view));

        if self.doc.is_empty() && !self.placeholder.is_empty() {
            out.push(DrawCommand::Text {
                family: FontFamily::Sans,
                weight: 400,
                line_height: Some(l.lh),
                origin: o,
                content: self.placeholder.clone(),
                size: self.base(),
                color: p.text_secondary.with_alpha(0.75),
                max_width: None,
            });
        }

        // Block furniture: quote bars.
        let mut quote: Option<(usize, f32, f32)> = None; // (block, top, bottom)
        let flush_quote = |q: &mut Option<(usize, f32, f32)>, out: &mut DrawList| {
            if let Some((_, top, bot)) = q.take() {
                out.push(quad(
                    Rectangle::new(Point::new(o.x + 1.0, o.y + top), Size::new(3.0, bot - top)),
                    p.border_strong,
                    1.5,
                ));
            }
        };
        let vis_lo = off - 40.0;
        let vis_hi = off + view.size.height + 40.0;
        for line in &l.lines {
            if let Some((blk, _, _)) = quote {
                if blk != line.block {
                    flush_quote(&mut quote, out);
                }
            }
            if self.doc.blocks[line.block].kind == BlockKind::Quote {
                match &mut quote {
                    Some(q) => q.2 = line.y + line.h,
                    None => quote = Some((line.block, line.y, line.y + line.h)),
                }
            }
            if line.y + line.h < vis_lo || line.y > vis_hi {
                continue;
            }
            let y = o.y + line.y;
            let x0 = o.x + line.x0;
            // Selection.
            if focused && a != z && a <= line.end_incl() && z > line.start {
                let from = l.x_at(&self.ui, line, a.max(line.start));
                let to_idx = z.min(line.end);
                let mut to = l.x_at(&self.ui, line, to_idx);
                if z > line.end && l.is_block_last(line) {
                    to += 5.0;
                }
                if to > from {
                    out.push(quad(
                        Rectangle::new(Point::new(x0 + from, y), Size::new(to - from, line.h)),
                        p.selection,
                        0.0,
                    ));
                }
            }
            // Text, code chips and underlines.
            for f in &line.frags {
                let (size, weight, family) = frag_style(f.marks, line, l.base);
                let fx = x0 + f.x;
                if f.marks.code {
                    out.push(quad(
                        Rectangle::new(
                            Point::new(fx - 2.0, y + (line.h - size * 1.45) / 2.0),
                            Size::new(f.w + 4.0, size * 1.45),
                        ),
                        mix(p.surface_raised, p.border, 0.45),
                        t.radii.small + 1.0,
                    ));
                }
                let ink = if f.link {
                    p.accent
                } else if self.doc.blocks[line.block].kind == BlockKind::Quote {
                    p.text_secondary
                } else {
                    p.text
                };
                // Renderers drop leading spaces, so draw them as an offset instead.
                let body = f.text.trim_start_matches(' ');
                let lead = f.text.len() - body.len();
                let dx = if lead > 0 {
                    measure(&*self.ui.metrics(), &f.text[..lead], size, weight, family)
                } else {
                    0.0
                };
                out.push(DrawCommand::Text {
                    family,
                    weight,
                    line_height: Some(line.h),
                    origin: Point::new(fx + dx, y),
                    content: body.to_string(),
                    size,
                    color: ink,
                    max_width: None,
                });
                if f.marks.underline || f.link {
                    let uy = y + (line.h + size) / 2.0 + 1.0;
                    out.push(quad(
                        Rectangle::new(Point::new(fx, uy), Size::new(f.w, 1.0)),
                        ink.with_alpha(if f.link { 0.55 } else { 0.9 }),
                        0.0,
                    ));
                }
            }
            // List markers sit on a block's first line.
            if line.first {
                let blk = &self.doc.blocks[line.block];
                match blk.kind {
                    BlockKind::Bullet => out.push(quad(
                        Rectangle::new(
                            Point::new(o.x + 9.0, y + line.h / 2.0 - 2.5),
                            Size::new(5.0, 5.0),
                        ),
                        p.text_secondary,
                        2.5,
                    )),
                    BlockKind::Numbered => {
                        let label = format!("{}.", l.numbers[line.block]);
                        let w =
                            self.ui
                                .metrics()
                                .width_styled(&label, l.base, 400, FontFamily::Sans);
                        out.push(DrawCommand::Text {
                            family: FontFamily::Sans,
                            weight: 400,
                            line_height: Some(line.h),
                            origin: Point::new(o.x + INDENT - 8.0 - w, y),
                            content: label,
                            size: l.base,
                            color: p.text_secondary,
                            max_width: None,
                        });
                    }
                    _ => {}
                }
            }
        }
        flush_quote(&mut quote, out);

        // Caret and IME text.
        let (li, cx) = l.caret_pos(&self.ui, e.caret);
        let line = &l.lines[li];
        if !e.preedit.is_empty() {
            let w = self
                .ui
                .metrics()
                .width_styled(&e.preedit, l.base, 400, FontFamily::Sans);
            let (x, y) = (o.x + line.x0 + cx, o.y + line.y);
            out.push(DrawCommand::Text {
                family: FontFamily::Sans,
                weight: 400,
                line_height: Some(line.h),
                origin: Point::new(x, y),
                content: e.preedit.clone(),
                size: l.base,
                color: p.text,
                max_width: None,
            });
            out.push(quad(
                Rectangle::new(Point::new(x, y + line.h - 3.0), Size::new(w, 1.0)),
                p.text,
                0.0,
            ));
        }
        let blink = (self.ui.now().saturating_sub(e.blink_epoch) / 530) % 2 == 0;
        if focused && editable && blink && a == z && st.link.is_none() {
            out.push(quad(
                Rectangle::new(
                    Point::new(o.x + line.x0 + cx, o.y + line.y + 2.0),
                    Size::new(1.5, line.h - 4.0),
                ),
                p.text,
                0.0,
            ));
        }
        self.ui.set_clip(outer);
        out.push(DrawCommand::PopClip);

        // Scrollbar.
        let max = self.max_scroll(&b, &l);
        if max > 0.0 {
            let h = view.size.height;
            let th = (h * h / (l.height + 2.0 * PAD_Y)).clamp(28.0, h);
            let r = Rectangle::new(
                Point::new(
                    view.origin.x + view.size.width - BAR - 2.0,
                    view.origin.y + (h - th) * off / max,
                ),
                Size::new(BAR, th),
            );
            let live = self.ui.slot(self.id ^ K_DRAG).is_some() || self.ui.hovered(&r);
            out.push(quad(
                r,
                if live {
                    p.border_strong
                } else {
                    p.border_strong.with_alpha(0.5)
                },
                BAR / 2.0,
            ));
        }

        // Toolbar.
        if self.tool_h() > 0.0 {
            let bar = Rectangle::new(
                Point::new(b.origin.x + 1.0, b.origin.y + 1.0),
                Size::new(b.size.width - 2.0, TOOLBAR_H - 1.0),
            );
            out.push(quad(bar, p.surface_raised, t.radii.medium));
            out.push(quad(
                Rectangle::new(
                    Point::new(b.origin.x + 1.0, b.origin.y + 8.0),
                    Size::new(b.size.width - 2.0, TOOLBAR_H - 8.0),
                ),
                p.surface_raised,
                0.0,
            ));
            out.push(quad(
                Rectangle::new(
                    Point::new(b.origin.x + 1.0, b.origin.y + TOOLBAR_H),
                    Size::new(b.size.width - 2.0, 1.0),
                ),
                p.border,
                0.0,
            ));
            for x in self.separators(&b) {
                out.push(quad(
                    Rectangle::new(Point::new(x, b.origin.y + 12.0), Size::new(1.0, 18.0)),
                    p.border,
                    0.0,
                ));
            }
            for (tool, r) in self.tools(&b) {
                let on = self.is_active(tool, &e, &st);
                let hot = self.ui.hovered(&r);
                if on {
                    out.push(quad(r, p.selection, t.radii.medium));
                } else if hot {
                    out.push(quad(r, p.hover, t.radii.medium));
                }
                draw_tool_glyph(
                    &self.ui,
                    tool,
                    r,
                    if on { p.text } else { p.text_secondary },
                    out,
                );
            }
            let words = format!("{} words", self.doc.words());
            let cap = t.typography.caption;
            let w = self.ui.metrics().width(&words, cap.size);
            out.push(DrawCommand::Text {
                family: FontFamily::Sans,
                weight: 400,
                line_height: Some(TOOLBAR_H - 1.0),
                origin: Point::new(b.origin.x + b.size.width - w - 14.0, b.origin.y),
                content: words,
                size: cap.size,
                color: p.text_secondary,
                max_width: None,
            });
        }

        // Link bar.
        if let Some(lb) = &st.link {
            let r = self.link_rect(&b);
            out.push(DrawCommand::Quad {
                bounds: r,
                fill: p.surface_raised,
                radius: t.radii.medium,
                border: Some(Border {
                    color: p.accent,
                    width: 1.0,
                }),
                shadow: Some(t.elevation.medium),
            });
            let body = t.typography.body;
            out.push(DrawCommand::PushClip(Rectangle::new(
                Point::new(r.origin.x + 10.0, r.origin.y),
                Size::new(LINK_W - 70.0, LINK_H),
            )));
            out.push(DrawCommand::Text {
                family: FontFamily::Sans,
                weight: 400,
                line_height: Some(LINK_H),
                origin: Point::new(r.origin.x + 10.0, r.origin.y),
                content: if lb.text.is_empty() {
                    "Paste or type a link".into()
                } else {
                    lb.text.clone()
                },
                size: body.size,
                color: if lb.text.is_empty() {
                    p.text_secondary.with_alpha(0.75)
                } else {
                    p.text
                },
                max_width: None,
            });
            out.push(DrawCommand::PopClip);
            let hint = if lb.text.is_empty() && self.doc.link_at(lb.range.0).is_some() {
                "Enter removes"
            } else {
                "Enter applies"
            };
            let cap = t.typography.caption;
            let w = self.ui.metrics().width(hint, cap.size);
            out.push(DrawCommand::Text {
                family: FontFamily::Sans,
                weight: 400,
                line_height: Some(LINK_H),
                origin: Point::new(r.origin.x + LINK_W - w - 12.0, r.origin.y),
                content: hint.into(),
                size: cap.size,
                color: p.text_secondary,
                max_width: None,
            });
        }
        self.ui.state(self.id, |s: &mut RichState| s.last = st.last);
    }
}

// -------------------------------------------------------------- state and tools

#[derive(Clone)]
struct Snap {
    doc: RichText,
    caret: usize,
    anchor: usize,
}
#[derive(Clone, Default)]
struct LinkBar {
    text: String,
    range: (usize, usize),
    fresh: bool,
}
#[derive(Clone, Default)]
struct RichState {
    /// Marks to apply to the next typed text, set by a toolbar toggle at a bare caret.
    pending: Option<(usize, Marks)>,
    link: Option<LinkBar>,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last_typing: bool,
    last_ms: u64,
    last: usize,
    caret_before: usize,
    anchor_before: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Bold,
    Italic,
    Underline,
    Code,
    Link,
    Head(u8),
    Bullet,
    Numbered,
    Quote,
}
impl Tool {
    fn mark(self) -> Option<Mark> {
        match self {
            Tool::Bold => Some(Mark::Bold),
            Tool::Italic => Some(Mark::Italic),
            Tool::Underline => Some(Mark::Underline),
            Tool::Code => Some(Mark::Code),
            _ => None,
        }
    }
}

fn draw_tool_glyph(ui: &Ui, tool: Tool, r: Rectangle, ink: Color, out: &mut DrawList) {
    let t = ui.theme();
    let (cx, cy) = (
        r.origin.x + r.size.width / 2.0,
        r.origin.y + r.size.height / 2.0,
    );
    let text = |s: &str, size: f32, weight: u16, family: FontFamily, out: &mut DrawList| {
        let w = ui.metrics().width_styled(s, size, weight, family);
        out.push(DrawCommand::Text {
            family,
            weight,
            line_height: Some(r.size.height),
            origin: Point::new(cx - w / 2.0, r.origin.y),
            content: s.into(),
            size,
            color: ink,
            max_width: None,
        });
    };
    let s = t.typography.body.size;
    match tool {
        Tool::Bold => text("B", s, 800, FontFamily::Sans, out),
        Tool::Italic => {
            // A slanted "I" with serifs: the renderer has no italic face yet.
            let line = |a: (f32, f32), b: (f32, f32), out: &mut DrawList| {
                out.push(DrawCommand::Line {
                    from: Point::new(a.0, a.1),
                    to: Point::new(b.0, b.1),
                    width: 1.6,
                    color: ink,
                });
            };
            line((cx + 2.2, cy - 6.0), (cx - 2.2, cy + 6.0), out);
            line((cx + 0.2, cy - 6.0), (cx + 4.4, cy - 6.0), out);
            line((cx - 4.4, cy + 6.0), (cx - 0.2, cy + 6.0), out);
        }
        Tool::Underline => {
            text("U", s, 600, FontFamily::Sans, out);
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(cx - 5.0, cy + 7.0), Size::new(10.0, 1.5)),
                fill: ink,
                radius: 0.0,
                border: None,
                shadow: None,
            });
        }
        Tool::Code => text("</>", s - 1.0, 500, FontFamily::Mono, out),
        Tool::Link => text("Link", s - 1.0, 600, FontFamily::Sans, out),
        Tool::Head(n) => text(&format!("H{n}"), s - 1.0, 700, FontFamily::Sans, out),
        Tool::Bullet => {
            for i in 0..3 {
                let y = cy - 5.5 + i as f32 * 5.5;
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(Point::new(cx - 7.0, y - 1.25), Size::new(2.5, 2.5)),
                    fill: ink,
                    radius: 1.25,
                    border: None,
                    shadow: None,
                });
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(Point::new(cx - 2.5, y - 0.75), Size::new(9.5, 1.5)),
                    fill: ink,
                    radius: 0.75,
                    border: None,
                    shadow: None,
                });
            }
        }
        Tool::Numbered => {
            for i in 0..3 {
                let y = cy - 5.5 + i as f32 * 5.5;
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(Point::new(cx - 7.0, y - 1.25), Size::new(2.5, 2.5)),
                    fill: ink.with_alpha(0.6),
                    radius: 0.5,
                    border: None,
                    shadow: None,
                });
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(Point::new(cx - 2.5, y - 0.75), Size::new(9.5, 1.5)),
                    fill: ink,
                    radius: 0.75,
                    border: None,
                    shadow: None,
                });
            }
        }
        Tool::Quote => {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(cx - 6.0, cy - 7.0), Size::new(2.5, 14.0)),
                fill: ink,
                radius: 1.25,
                border: None,
                shadow: None,
            });
            for i in 0..2 {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(cx - 1.5, cy - 4.5 + i as f32 * 6.0),
                        Size::new(8.0 - i as f32 * 2.5, 1.5),
                    ),
                    fill: ink,
                    radius: 0.75,
                    border: None,
                    shadow: None,
                });
            }
        }
    }
}

impl<M: 'static> RichEditor<M> {
    /// `- `, `* `, `1. `, `#`..`###` and `>` before a space turn a paragraph into that block.
    fn markdown_shortcut(&self, st: &mut RichState, e: &mut Edit, out: &mut Vec<M>) -> bool {
        let (a, z) = e.range();
        if a != z {
            return false;
        }
        let (bi, off) = self.doc.at(a);
        let blk = &self.doc.blocks[bi];
        if blk.kind != BlockKind::Paragraph || off == 0 || off > 4 {
            return false;
        }
        let before: String = blk.text().chars().take(off).collect();
        let kind = match before.as_str() {
            "-" | "*" => BlockKind::Bullet,
            "#" => BlockKind::Heading(1),
            "##" => BlockKind::Heading(2),
            "###" => BlockKind::Heading(3),
            ">" => BlockKind::Quote,
            s if s.ends_with('.')
                && s.len() > 1
                && s[..s.len() - 1].chars().all(|c| c.is_ascii_digit()) =>
            {
                BlockKind::Numbered
            }
            _ => return false,
        };
        let start = self.doc.block_start(bi);
        let mut d = self.doc.clone();
        d.replace(start, start + off, "", Marks::default(), None);
        d.set_kind(start, start, kind);
        e.caret = start;
        e.anchor = start;
        self.commit(st, e, d, false, out);
        true
    }
}

// -------------------------------------------------------------- layout

const INDENT: f32 = 28.0;
const QUOTE_INDENT: f32 = 18.0;

/// A run of text on a line with one style.
struct Frag {
    start: usize,
    n: usize,
    x: f32,
    w: f32,
    text: String,
    marks: Marks,
    link: bool,
}
struct Line {
    block: usize,
    y: f32,
    h: f32,
    x0: f32,
    start: usize,
    end: usize,
    frags: Vec<Frag>,
    first: bool,
    /// Heading size multiplier and weight.
    scale: f32,
    weight: u16,
}
impl Line {
    fn end_incl(&self) -> usize {
        self.end
    }
}
struct Laid {
    lines: Vec<Line>,
    height: f32,
    numbers: Vec<usize>,
    base: f32,
    lh: f32,
}

thread_local! {
    static LAYOUTS: RefCell<HashMap<u64, (u64, Rc<Laid>)>> = RefCell::new(HashMap::new());
}

fn frag_style(marks: Marks, line: &Line, base: f32) -> (f32, u16, FontFamily) {
    let size = base * line.scale;
    let mut weight = line.weight;
    if marks.bold {
        weight = weight.max(700);
    } else if marks.italic && weight < 500 {
        weight = 500; // stands in for italic until a bundled italic face exists
    }
    if marks.code {
        (
            size * 0.92,
            if marks.bold { 700 } else { 400 },
            FontFamily::Mono,
        )
    } else {
        (size, weight, FontFamily::Sans)
    }
}

impl Laid {
    fn get(id: u64, doc: &RichText, width: f32, base: f32, ui: &Ui) -> Rc<Laid> {
        let key = doc.fingerprint() ^ (width.to_bits() as u64) << 7 ^ (base.to_bits() as u64) << 21;
        LAYOUTS.with(|c| {
            let mut c = c.borrow_mut();
            if let Some((k, l)) = c.get(&id) {
                if *k == key {
                    return l.clone();
                }
            }
            let l = Rc::new(Laid::build(doc, width, base, ui));
            c.insert(id, (key, l.clone()));
            l
        })
    }
    fn build(doc: &RichText, width: f32, base: f32, ui: &Ui) -> Laid {
        let m = ui.metrics();
        let lh = (base * 1.6).round();
        let mut lines: Vec<Line> = Vec::new();
        let mut numbers = vec![0; doc.blocks.len()];
        let (mut y, mut flat, mut n) = (0.0f32, 0usize, 0usize);
        let mut prev: Option<BlockKind> = None;
        for (bi, b) in doc.blocks.iter().enumerate() {
            n = if b.kind == BlockKind::Numbered {
                n + 1
            } else {
                0
            };
            numbers[bi] = n;
            let (scale, weight, lscale) = match b.kind {
                BlockKind::Heading(1) => (1.8, 700, 1.3),
                BlockKind::Heading(2) => (1.4, 700, 1.35),
                BlockKind::Heading(_) => (1.18, 650, 1.4),
                _ => (1.0, 400, 1.0),
            };
            let h = if lscale == 1.0 {
                lh
            } else {
                (base * scale * lscale).round()
            };
            y += match (prev, b.kind) {
                (None, _) => 0.0,
                (Some(BlockKind::Bullet), BlockKind::Bullet)
                | (Some(BlockKind::Numbered), BlockKind::Numbered) => 3.0,
                (_, BlockKind::Heading(_)) => 18.0,
                (Some(BlockKind::Heading(_)), _) => 6.0,
                _ => 10.0,
            };
            prev = Some(b.kind);
            let x0 = match b.kind {
                BlockKind::Bullet | BlockKind::Numbered => INDENT,
                BlockKind::Quote => QUOTE_INDENT,
                _ => 0.0,
            };
            let avail = (width - x0).max(40.0);
            let mut line = Line {
                block: bi,
                y,
                h,
                x0,
                start: flat,
                end: flat,
                frags: Vec::new(),
                first: true,
                scale,
                weight,
            };
            let mut x = 0.0f32;
            let mut pos = flat;
            let probe = Line {
                block: bi,
                y,
                h,
                x0,
                start: 0,
                end: 0,
                first: false,
                scale,
                weight,
                frags: Vec::new(),
            };
            for r in &b.runs {
                let (size, w, fam) = frag_style(r.marks, &probe, base);
                let width_of = |s: &str| measure(&*m, s, size, w, fam);
                for word in r.text.split_inclusive(' ') {
                    let n_chars = word.chars().count();
                    let ww = width_of(word);
                    let trimmed = if word.ends_with(' ') {
                        width_of(word.trim_end())
                    } else {
                        ww
                    };
                    if x > 0.0 && x + trimmed > avail {
                        y += h;
                        let mut next = Line {
                            block: bi,
                            y,
                            h,
                            x0,
                            start: pos,
                            end: pos,
                            frags: Vec::new(),
                            first: false,
                            scale,
                            weight,
                        };
                        std::mem::swap(&mut next, &mut line);
                        next.end = pos;
                        lines.push(next);
                        x = 0.0;
                    }
                    // A word wider than the line breaks by character.
                    if trimmed > avail && x == 0.0 {
                        let mut chunk = String::new();
                        for ch in word.chars() {
                            let mut t = chunk.clone();
                            t.push(ch);
                            if !chunk.is_empty() && width_of(&t) > avail {
                                let cw = width_of(&chunk);
                                push_frag(&mut line, &mut pos, &mut x, &chunk, cw, r);
                                y += h;
                                let mut next = Line {
                                    block: bi,
                                    y,
                                    h,
                                    x0,
                                    start: pos,
                                    end: pos,
                                    frags: Vec::new(),
                                    first: false,
                                    scale,
                                    weight,
                                };
                                std::mem::swap(&mut next, &mut line);
                                next.end = pos;
                                lines.push(next);
                                x = 0.0;
                                chunk.clear();
                            }
                            chunk.push(ch);
                        }
                        let cw = width_of(&chunk);
                        push_frag(&mut line, &mut pos, &mut x, &chunk, cw, r);
                        continue;
                    }
                    let _ = n_chars;
                    push_frag(&mut line, &mut pos, &mut x, word, ww, r);
                }
            }
            line.end = pos;
            lines.push(line);
            y += h;
            flat = pos + 1;
        }
        Laid {
            lines,
            height: y,
            numbers,
            base,
            lh,
        }
    }
    fn is_block_last(&self, line: &Line) -> bool {
        let i = self
            .lines
            .iter()
            .position(|l| std::ptr::eq(l, line))
            .unwrap_or(0);
        self.lines
            .get(i + 1)
            .map_or(true, |n| n.block != line.block)
    }
    /// The last caret position on line `i` (before the trailing space of a soft wrap).
    fn line_end(&self, i: usize) -> usize {
        let l = &self.lines[i];
        if self.is_block_last(l) {
            l.end
        } else {
            l.end.saturating_sub(1).max(l.start)
        }
    }
    /// Which line the caret at flat `idx` is on, and its x within the line.
    fn caret_pos(&self, ui: &Ui, idx: usize) -> (usize, f32) {
        let mut found = self.lines.len() - 1;
        for (i, l) in self.lines.iter().enumerate() {
            if idx >= l.start && (idx < l.end || (idx == l.end && self.is_block_last(l))) {
                found = i;
                break;
            }
        }
        (found, self.x_at(ui, &self.lines[found], idx))
    }
    /// x of flat position `idx` within `line` (clamped to the line).
    fn x_at(&self, ui: &Ui, line: &Line, idx: usize) -> f32 {
        let m = ui.metrics();
        let mut x = 0.0;
        for f in &line.frags {
            if idx <= f.start {
                return f.x;
            }
            if idx < f.start + f.n {
                let prefix: String = f.text.chars().take(idx - f.start).collect();
                let (size, w, fam) = frag_style(f.marks, line, self.base);
                return f.x + measure(&*m, &prefix, size, w, fam);
            }
            x = f.x + f.w;
        }
        x
    }
    /// Flat index nearest x on line `i`.
    fn index_in_line(&self, ui: &Ui, i: usize, x: f32) -> usize {
        let line = &self.lines[i];
        let m = ui.metrics();
        for f in &line.frags {
            if x < f.x + f.w {
                if x <= f.x {
                    return f.start;
                }
                let (size, w, fam) = frag_style(f.marks, line, self.base);
                let col = nearest_boundary(&f.text, x - f.x, |s| measure(&*m, s, size, w, fam));
                return f.start + col;
            }
        }
        self.line_end(i)
    }
    fn line_at(&self, y: f32) -> usize {
        self.lines
            .iter()
            .position(|l| y < l.y + l.h + 2.0)
            .unwrap_or(self.lines.len() - 1)
    }
    fn index_at(&self, ui: &Ui, x: f32, y: f32) -> usize {
        let i = self.line_at(y);
        self.index_in_line(ui, i, x - self.lines[i].x0)
    }
    fn hits_link(&self, ui: &Ui, x: f32, y: f32) -> bool {
        let i = self.line_at(y);
        let line = &self.lines[i];
        let x = x - line.x0;
        let _ = ui;
        y >= line.y
            && y < line.y + line.h
            && line
                .frags
                .iter()
                .any(|f| f.link && x >= f.x && x < f.x + f.w)
    }
}

/// Width of `s` including trailing spaces, which shapers often drop when measuring.
fn measure(m: &dyn crate::TextMetrics, s: &str, size: f32, weight: u16, fam: FontFamily) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    if s.ends_with(' ') || s.starts_with(' ') {
        m.width_styled(&format!("|{s}|"), size, weight, fam)
            - m.width_styled("||", size, weight, fam)
    } else {
        m.width_styled(s, size, weight, fam)
    }
}

fn push_frag(
    line: &mut Line,
    pos: &mut usize,
    x: &mut f32,
    text: &str,
    w: f32,
    run: &crate::rich_text::Run,
) {
    let n = text.chars().count();
    match line.frags.last_mut() {
        Some(l)
            if l.marks == run.marks && l.link == run.link.is_some() && l.start + l.n == *pos =>
        {
            l.text.push_str(text);
            l.n += n;
            l.w += w;
        }
        _ => line.frags.push(Frag {
            start: *pos,
            n,
            x: *x,
            w,
            text: text.to_string(),
            marks: run.marks,
            link: run.link.is_some(),
        }),
    }
    *pos += n;
    *x += w;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ui;
    use alloy_ui_theme::Theme;

    #[test]
    fn layout_wraps_and_positions_the_caret() {
        let ui = Ui::new(Theme::light());
        let d = RichText::from_markdown("alpha beta gamma delta epsilon zeta eta theta\n\n- item");
        let l = Laid::build(&d, 120.0, 15.0, &ui);
        assert!(
            l.lines.len() > 3,
            "wrapped into several lines: {}",
            l.lines.len()
        );
        // Every character belongs to exactly one line start..end chain.
        for w in l.lines.windows(2) {
            if w[0].block == w[1].block {
                assert_eq!(w[0].end, w[1].start, "soft wraps are contiguous");
            }
        }
        let (li, x) = l.caret_pos(&ui, 0);
        assert_eq!((li, x), (0, 0.0));
        let (li, _) = l.caret_pos(&ui, d.len());
        assert_eq!(li, l.lines.len() - 1);
        // A click at the far right of line 0 lands before its trailing space.
        let idx = l.index_in_line(&ui, 0, 1.0e4);
        assert_eq!(idx, l.line_end(0));
        assert!(l.height > 0.0);
    }
}
