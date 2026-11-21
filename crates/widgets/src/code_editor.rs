//! A code editor: line numbers, syntax colours, auto-indent, find, comment toggle.

use crate::{
    code_ops::{self, Dir, Sel},
    highlight::{tokenize, Lang, State, Token},
    text_edit::{
        apply_key, commit_edit, history_edit, line_range, primary, sanitize, word_bounds, Edit,
        KeyOutcome,
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
use std::{cell::RefCell, collections::HashMap, hash::Hasher, rc::Rc};

const BAR: f32 = 8.0;
const K_DRAG: u64 = 0x_c0de_0001 << 20;
const FIND_W: f32 = 304.0;
const FIND_H: f32 = 34.0;
const STATUS_H: f32 = 26.0;
const MAX_MATCHES: usize = 100_000;
/// Ui state slot for extra carets (the editor's own id holds the find state).
const K_MULTI: u64 = 0x_3c17;

/// A monospace editor for source code. Controlled like [`crate::TextEditor`]: you own the
/// string and receive the new one from `on_input`. Without `on_input` it is a read-only
/// viewer that still selects, copies and searches.
///
/// Enter keeps the indent (and opens a block after `{`, `(`, `[` or `:`), Tab and Shift+Tab
/// indent and dedent whole selections, brackets and quotes close themselves, `Ctrl+/`
/// toggles line comments for the language, and `Ctrl+F` opens find (Enter and Shift+Enter
/// step through matches, Escape closes). The bracket next to the caret is outlined together
/// with its match.
///
/// **Multiple cursors**: `Alt`+click adds a caret, `Alt+Shift+Up/Down` adds one above or below,
/// `Ctrl+Shift+D` selects the word under the caret and then each next occurrence, and
/// `Ctrl+Shift+L` selects them all. Typing, Backspace, Delete, Enter, Tab, the arrows, Home,
/// End, copy and paste then act at every caret; Escape (or a plain click) returns to one.
/// Undo reverts a multi-cursor edit as one step. Lines never wrap; the view scrolls both ways and
/// only lines in view are drawn, so a 100 000-line file costs a screenful.
pub struct CodeEditor<M> {
    ui: Ui,
    id: u64,
    value: String,
    lang: Lang,
    on_input: Option<Box<dyn Fn(String) -> M>>,
    width: Length,
    height: Length,
    numbers: bool,
    status: bool,
    tab: usize,
}

impl<M: 'static> CodeEditor<M> {
    /// An editor showing `value`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, value: impl Into<String>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            value: value.into(),
            lang: Lang::Plain,
            on_input: None,
            width: Length::Fill(1),
            height: Length::Fill(1),
            numbers: true,
            status: true,
            tab: 4,
        }
    }
    /// The language to colour. Default is plain text.
    pub fn lang(mut self, lang: Lang) -> Self {
        self.lang = lang;
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
    /// Show or hide the line-number gutter (default shown).
    pub fn line_numbers(mut self, on: bool) -> Self {
        self.numbers = on;
        self
    }
    /// Show or hide the status bar with line, column and language (default shown).
    pub fn status(mut self, on: bool) -> Self {
        self.status = on;
        self
    }
    /// Spaces per indent level and per tab stop (default 4).
    pub fn tab_width(mut self, n: usize) -> Self {
        self.tab = n.clamp(1, 8);
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    // ------------------------------------------------------------ geometry

    fn size(&self) -> f32 {
        self.ui.theme().typography.code.size
    }
    fn lh(&self) -> f32 {
        self.ui.theme().typography.code.line_height
    }
    /// Width of one monospace cell.
    fn cw(&self) -> f32 {
        let w = self
            .ui
            .metrics()
            .width_styled("0000000000", self.size(), 400, FontFamily::Mono)
            / 10.0;
        w.max(1.0)
    }
    fn status_h(&self) -> f32 {
        if self.status {
            STATUS_H
        } else {
            0.0
        }
    }
    fn gutter(&self, doc: &Doc) -> f32 {
        if !self.numbers {
            return 12.0;
        }
        let digits = doc.len().to_string().len().max(2) as f32;
        digits * self.cw() + 28.0
    }
    /// The area lines are drawn in (inside the border, above the status bar).
    fn view(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x + 1.0, b.origin.y + 1.0),
            Size::new(
                (b.size.width - 2.0).max(0.0),
                (b.size.height - 2.0 - self.status_h()).max(0.0),
            ),
        )
    }
    fn max_scroll_y(&self, b: &Rectangle, doc: &Doc) -> f32 {
        let v = self.view(b);
        (doc.len() as f32 * self.lh() + self.lh() - v.size.height).max(0.0)
    }
    fn max_scroll_x(&self, b: &Rectangle, doc: &Doc) -> f32 {
        let v = self.view(b);
        let widest = doc.widest(&self.value, self.tab) as f32 * self.cw();
        (widest + 24.0 - (v.size.width - self.gutter(doc))).max(0.0)
    }
    fn find_rect(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x + b.size.width - FIND_W - 16.0, b.origin.y + 8.0),
            Size::new(FIND_W, FIND_H),
        )
    }
    /// The three find-bar buttons: previous, next, close.
    fn find_buttons(&self, b: &Rectangle) -> [Rectangle; 3] {
        let f = self.find_rect(b);
        let x = f.origin.x + f.size.width - 4.0 - 26.0 * 3.0;
        [0, 1, 2].map(|i| {
            Rectangle::new(
                Point::new(x + 26.0 * i as f32, f.origin.y + 4.0),
                Size::new(26.0, FIND_H - 8.0),
            )
        })
    }
    /// Character index under `p`.
    fn index_at(&self, b: &Rectangle, doc: &Doc, st_x: f32, p: Point) -> usize {
        let v = self.view(b);
        let y = p.y - v.origin.y - self.lh() * 0.5 + self.ui.scroll_offset(self.id);
        let line = ((y / self.lh()).round().max(0.0) as usize).min(doc.len() - 1);
        let x = p.x - v.origin.x - self.gutter(doc) + st_x;
        let cell = (x / self.cw()).max(0.0);
        let col = col_at(doc.line(&self.value, line), cell, self.tab);
        doc.index_of(&self.value, line, col)
    }

    // ------------------------------------------------------------ editing

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
    /// The extra carets, if they still belong to the text on screen.
    fn multi(&self, doc: &Doc) -> Multi {
        let m = self.ui.state(self.id ^ K_MULTI, |m: &mut Multi| m.clone());
        if m.hash == doc.hash {
            m
        } else {
            Multi::default()
        }
    }
    /// Every selection, primary last in the input and tracked through sorting.
    fn all_sels(e: &Edit, mc: &Multi) -> (Vec<Sel>, usize) {
        let mut v = mc.extras.clone();
        v.push(Sel {
            anchor: e.anchor,
            caret: e.caret,
        });
        let keep = v.len() - 1;
        let at = code_ops::normalize(&mut v, keep);
        (v, at)
    }
    /// Make `sels` the selections, with the one at `keep` as the primary.
    fn set_sels(&self, e: &mut Edit, mc: &mut Multi, mut sels: Vec<Sel>, keep: usize) {
        if sels.is_empty() {
            return;
        }
        let k = keep.min(sels.len() - 1);
        let at = code_ops::normalize(&mut sels, k);
        let p = sels.remove(at);
        (e.anchor, e.caret) = (p.anchor, p.caret);
        e.goal_col = None;
        mc.extras = sels;
        mc.hash = doc_hash(&self.value);
    }
    /// Apply a text edit at every selection and commit it as one undo step.
    fn multi_edit(
        &self,
        mc: &mut Multi,
        e: &mut Edit,
        caret0: usize,
        out: &mut Vec<M>,
        op: impl FnOnce(&str, &[Sel]) -> (String, Vec<Sel>),
    ) {
        let (sels, _) = Self::all_sels(e, mc);
        let (new, carets) = op(&self.value, &sels);
        mc.hash = doc_hash(&new);
        let mut it = carets;
        if let Some(p) = it.pop() {
            (e.anchor, e.caret) = (p.anchor, p.caret);
        }
        mc.extras = it;
        e.goal_col = None;
        if new != self.value {
            // Typing one character at each of n carets is one burst for undo, like typing one.
            let (now, n) = (self.ui.now(), mc.extras.len() + 1);
            self.ui
                .with_history(self.id, |h| h.record_carets(&self.value, caret0, &new, now, n));
            if let Some(f) = &self.on_input {
                out.push(f(new));
            }
        }
    }
    /// Multi-cursor handling for one event. `None` lets the single-caret code run.
    #[allow(clippy::too_many_arguments)]
    fn multi_event(
        &self,
        event: &Event,
        e: &mut Edit,
        mc: &mut Multi,
        fs: &FindState,
        doc: &Doc,
        caret0: usize,
        b: &Rectangle,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> Option<bool> {
        let editable = self.on_input.is_some();
        let focused = self.ui.is_focused(Some(self.id));
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                let v = self.view(b);
                let on_bar = self.max_scroll_y(b, doc) > 0.0
                    && cursor.x >= v.origin.x + v.size.width - BAR - 6.0;
                let in_gutter = self.numbers && cursor.x < v.origin.x + self.gutter(doc);
                let on_find = fs.open && self.find_rect(b).contains(cursor);
                if self.ui.modifiers().alt && editable && !on_bar && !in_gutter && !on_find {
                    self.ui.set_focus(Some(self.id));
                    let i = self.index_at(b, doc, e.scroll_x, cursor);
                    let (mut sels, _) = Self::all_sels(e, mc);
                    sels.push(Sel::at(i));
                    let keep = sels.len() - 1;
                    self.set_sels(e, mc, sels, keep);
                    return Some(true);
                }
                mc.extras.clear();
                None
            }
            _ if !focused || fs.focus || !e.preedit.is_empty() => None,
            Event::TextInput(s) if editable && !mc.extras.is_empty() => {
                let clean = sanitize(s, true);
                if !clean.is_empty() {
                    self.multi_edit(mc, e, caret0, out, |t, sels| code_ops::insert(t, sels, &clean));
                }
                Some(true)
            }
            Event::KeyPressed(k, m) => {
                let cmd = primary(m);
                // Ctrl+Alt is AltGr on many layouts, so adding a caret is Alt+Shift+arrow.
                let add_caret = m.alt && m.shift && !m.ctrl && !m.logo;
                // Ways in.
                match k {
                    Key::Character(c) if cmd && m.shift && c.eq_ignore_ascii_case("d") => {
                        let (sels, keep) = Self::all_sels(e, mc);
                        let (mut next, changed) = code_ops::add_next_occurrence(&self.value, &sels, keep);
                        if changed {
                            let newest = next.len() - 1;
                            // A first press only selects the word: keep it primary.
                            let k = if next.len() == sels.len() { keep } else { newest };
                            self.set_sels(e, mc, std::mem::take(&mut next), k);
                        }
                        return Some(true);
                    }
                    Key::Character(c) if cmd && m.shift && c.eq_ignore_ascii_case("l") => {
                        let (sels, keep) = Self::all_sels(e, mc);
                        let all = code_ops::select_all_occurrences(&self.value, &sels, keep);
                        let last = all.len().saturating_sub(1);
                        self.set_sels(e, mc, all, last);
                        return Some(true);
                    }
                    Key::Named(n) if add_caret && (n == "ArrowUp" || n == "ArrowDown") => {
                        let (sels, _) = Self::all_sels(e, mc);
                        let next = code_ops::add_caret_vertical(&self.value, &sels, n == "ArrowDown");
                        let keep = next.len().saturating_sub(1);
                        // The new caret is the one that was not there before.
                        let pos = next.iter().position(|s| !sels.contains(s)).unwrap_or(keep);
                        self.set_sels(e, mc, next, pos);
                        return Some(true);
                    }
                    _ => {}
                }
                if mc.extras.is_empty() {
                    return None;
                }
                let (sels, _) = Self::all_sels(e, mc);
                let plain = !cmd && !m.alt;
                match k {
                    Key::Escape => {
                        mc.extras.clear();
                        Some(true)
                    }
                    Key::Backspace if editable && plain => {
                        self.multi_edit(mc, e, caret0, out, |t, s| code_ops::delete(t, s, false));
                        Some(true)
                    }
                    Key::Named(n) if editable && plain && n == "Delete" => {
                        self.multi_edit(mc, e, caret0, out, |t, s| code_ops::delete(t, s, true));
                        Some(true)
                    }
                    Key::Enter if editable && plain => {
                        self.multi_edit(mc, e, caret0, out, code_ops::newline);
                        Some(true)
                    }
                    Key::Tab if editable && plain => {
                        let unit = self.indent_unit();
                        self.multi_edit(mc, e, caret0, out, |t, s| code_ops::insert(t, s, &unit));
                        Some(true)
                    }
                    Key::Named(n) if plain => {
                        let dir = match n.as_str() {
                            "ArrowLeft" => Some(Dir::Left),
                            "ArrowRight" => Some(Dir::Right),
                            "ArrowUp" => Some(Dir::Up),
                            "ArrowDown" => Some(Dir::Down),
                            "Home" => Some(Dir::Home),
                            "End" => Some(Dir::End),
                            _ => None,
                        };
                        match dir {
                            Some(d) => {
                                let next = code_ops::step(&self.value, &sels, d, m.shift);
                                let last = next.len().saturating_sub(1);
                                self.set_sels(e, mc, next, last);
                                Some(true)
                            }
                            None => None,
                        }
                    }
                    Key::Character(c) if cmd && !m.alt && c.eq_ignore_ascii_case("v") && editable => {
                        if let Some(text) = self.ui.clipboard().get() {
                            let text = sanitize(&text, true);
                            let lines: Vec<String> = text.split('\n').map(str::to_string).collect();
                            if lines.len() == sels.len() {
                                // One line per caret.
                                self.multi_edit(mc, e, caret0, out, |t, s| {
                                    code_ops::replace_all(t, s, &|i, _| lines[i].clone())
                                });
                            } else {
                                self.multi_edit(mc, e, caret0, out, |t, s| code_ops::insert(t, s, &text));
                            }
                        }
                        Some(true)
                    }
                    Key::Character(c) if cmd && !m.alt && c.eq_ignore_ascii_case("c") => {
                        let parts: Vec<String> = sels
                            .iter()
                            .filter(|s| !s.is_empty())
                            .map(|s| {
                                let (a, z) = s.range();
                                self.value.chars().skip(a).take(z - a).collect()
                            })
                            .collect();
                        if !parts.is_empty() {
                            self.ui.clipboard().set(parts.join("\n"));
                        }
                        Some(true)
                    }
                    // Plain characters arrive as text; everything else (undo, comment
                    // toggle, find...) works on one caret, so drop the extras first.
                    Key::Character(_) if !cmd => None,
                    _ => {
                        mc.extras.clear();
                        None
                    }
                }
            }
            _ => None,
        }
    }
    fn indent_unit(&self) -> String {
        " ".repeat(self.tab)
    }
    /// Apply per-line edits `(line, at_col, remove, insert)` to lines `first..=last`,
    /// keeping the selection on the same text.
    fn edit_lines(
        &self,
        doc: &Doc,
        e: &mut Edit,
        (first, last): (usize, usize),
        op: impl Fn(&str) -> (usize, usize, String),
    ) -> String {
        let v = &self.value;
        // (char index of the edit, chars removed, chars inserted) per line.
        let mut ops: Vec<(usize, usize, usize)> = Vec::new();
        let mut rebuilt = String::new();
        let (lo, hi) = (doc.starts[first].0, doc.starts[last].1);
        for l in first..=last {
            let line = doc.line(v, l);
            let (at, rm, ins) = op(line);
            let bat = byte_col(line, at);
            let brm = bat + byte_col(&line[bat..], rm);
            rebuilt.push_str(&line[..bat]);
            rebuilt.push_str(&ins);
            rebuilt.push_str(&line[brm..]);
            if l < last {
                rebuilt.push('\n');
            }
            ops.push((doc.cstart[l] + at, rm, ins.chars().count()));
        }
        let map = |p: usize| -> usize {
            let mut q = p;
            for &(s, rm, ins) in &ops {
                if p >= s + rm {
                    q = q + ins - rm.min(q + ins);
                } else if p > s {
                    q = q + s + ins - p; // inside a removed run: park at its end
                }
            }
            q
        };
        e.anchor = map(e.anchor);
        e.caret = map(e.caret);
        format!("{}{}{}", &v[..lo], rebuilt, &v[hi..])
    }
    /// First and last line a block operation touches.
    fn block_lines(&self, doc: &Doc, e: &Edit) -> (usize, usize) {
        let (a, z) = e.range();
        let (la, _) = doc.line_col(a);
        let (mut lz, cz) = doc.line_col(z);
        if z > a && cz == 0 && lz > la {
            lz -= 1;
        }
        (la, lz)
    }
    fn toggle_comment(&self, doc: &Doc, e: &mut Edit) -> Option<String> {
        let marker = self.lang.line_comment()?;
        let (f, l) = self.block_lines(doc, e);
        let lines: Vec<&str> = (f..=l).map(|i| doc.line(&self.value, i)).collect();
        let all = lines
            .iter()
            .filter(|s| !s.trim().is_empty())
            .all(|s| s.trim_start().starts_with(marker));
        let base = lines
            .iter()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap_or(0);
        let mchars = marker.chars().count();
        Some(self.edit_lines(doc, e, (f, l), |line| {
            if line.trim().is_empty() {
                (0, 0, String::new())
            } else if all {
                let ind = line.chars().take_while(|c| c.is_whitespace()).count();
                let space = line[line
                    .char_indices()
                    .nth(ind + mchars)
                    .map_or(line.len(), |x| x.0)..]
                    .starts_with(' ') as usize;
                (ind, mchars + space, String::new())
            } else {
                (base, 0, format!("{marker} "))
            }
        }))
    }
    fn indent_lines(&self, doc: &Doc, e: &mut Edit, back: bool) -> String {
        let range = self.block_lines(doc, e);
        let unit = self.indent_unit();
        let tab = self.tab;
        self.edit_lines(doc, e, range, move |line| {
            if back {
                let n = if line.starts_with('\t') {
                    1
                } else {
                    line.chars().take(tab).take_while(|c| *c == ' ').count()
                };
                (0, n, String::new())
            } else if line.is_empty() {
                (0, 0, String::new())
            } else {
                (0, 0, unit.clone())
            }
        })
    }
    /// Enter: keep the indent, open a block after an opener.
    fn newline(&self, doc: &Doc, e: &mut Edit) -> String {
        let (line, col) = doc.line_col(e.range().0);
        let text = doc.line(&self.value, line);
        let before: String = text.chars().take(col).collect();
        let after: String = text.chars().skip(col).collect();
        let ind: String = text
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        let t = before.trim_end();
        let opens = t.ends_with(['{', '(', '[']) || (self.lang == Lang::Python && t.ends_with(':'));
        let closes = matches!(
            (t.chars().last(), after.chars().next()),
            (Some('{'), Some('}')) | (Some('('), Some(')')) | (Some('['), Some(']'))
        );
        let inner = if opens {
            format!("{ind}{}", self.indent_unit())
        } else {
            ind.clone()
        };
        if closes {
            let v = e.insert(&self.value, &format!("\n{inner}\n{ind}"));
            e.caret -= 1 + ind.chars().count();
            e.anchor = e.caret;
            v
        } else {
            e.insert(&self.value, &format!("\n{inner}"))
        }
    }
    /// A typed character, with bracket and quote pairing. `None` means "plain insert".
    fn typed(&self, e: &mut Edit, c: char) -> Option<String> {
        let close = match c {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            '"' => '"',
            '`' if self.lang != Lang::Plain => '`',
            ')' | ']' | '}' => c,
            _ => return None,
        };
        let chars: Vec<char> = self.value.chars().collect();
        let next = chars.get(e.caret).copied();
        let prev = e.caret.checked_sub(1).and_then(|i| chars.get(i)).copied();
        if matches!(c, ')' | ']' | '}') {
            // Typing a closer over the one already there steps past it.
            if !e.has_selection() && next == Some(c) {
                e.move_to(e.caret + 1, false);
                return Some(self.value.clone());
            }
            return None;
        }
        if e.has_selection() {
            let sel = e.selected(&self.value);
            let (a, _) = e.range();
            let v = e.insert(&self.value, &format!("{c}{sel}{close}"));
            e.anchor = a + 1;
            e.caret = a + 1 + sel.chars().count();
            return Some(v);
        }
        if c == close {
            if next == Some(c) {
                e.move_to(e.caret + 1, false);
                return Some(self.value.clone());
            }
            if prev.is_some_and(|p| p.is_alphanumeric() || p == '\\') {
                return None;
            }
        }
        let room = next.map_or(true, |n| {
            n.is_whitespace() || matches!(n, ')' | ']' | '}' | ',' | ';')
        });
        if !room {
            return None;
        }
        let v = e.insert(&self.value, &format!("{c}{close}"));
        e.caret -= 1;
        e.anchor = e.caret;
        Some(v)
    }
    /// Backspace between an empty pair removes both.
    fn backspace_pair(&self, e: &mut Edit) -> Option<String> {
        if e.has_selection() || e.caret == 0 {
            return None;
        }
        let chars: Vec<char> = self.value.chars().collect();
        let (p, n) = (chars.get(e.caret - 1)?, chars.get(e.caret)?);
        if !matches!(
            (p, n),
            ('(', ')') | ('[', ']') | ('{', '}') | ('"', '"') | ('`', '`')
        ) {
            return None;
        }
        e.anchor = e.caret - 1;
        let mut v = String::new();
        v.extend(chars[..e.caret - 1].iter());
        v.extend(chars[e.caret + 1..].iter());
        e.caret -= 1;
        e.anchor = e.caret;
        Some(v)
    }

    // ------------------------------------------------------------ find

    fn matches(&self, query: &str) -> Rc<Vec<(usize, usize)>> {
        find_all(self.id, &self.value, query)
    }
    /// Select the match after (or before) the selection and put it in view.
    fn step_match(&self, e: &mut Edit, query: &str, back: bool) {
        let ms = self.matches(query);
        if ms.is_empty() {
            return;
        }
        let (a, z) = e.range();
        let pick = if back {
            ms.iter().rev().find(|m| m.0 < a).or(ms.last())
        } else {
            ms.iter().find(|m| m.0 >= z).or(ms.first())
        };
        if let Some(&(s, l)) = pick {
            e.anchor = s;
            e.caret = s + l;
            e.goal_col = None;
        }
    }
}

impl<M: 'static> Widget<M> for CodeEditor<M> {
    fn style(&self) -> Style {
        util::sized(self.width, self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(320.0, 8.0 * self.lh() + self.status_h() + 2.0)
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
        let doc = Doc::get(self.id, &self.value);
        let mut e = self.ui.edit(self.id);
        e.clamp(doc.nchars);
        let caret0 = e.caret;
        let editable = self.on_input.is_some();
        let focused = self.ui.is_focused(Some(self.id));
        let pkey = util::key_of(&b);
        let mut fs: FindState = self.ui.state(self.id, |s: &mut FindState| s.clone());
        let fs0 = fs.clone();
        let view = self.view(&b);
        let mut mc = self.multi(&doc);
        let mc0 = mc.clone();
        let consumed = 'ev: {
            if let Some(handled) =
                self.multi_event(event, &mut e, &mut mc, &fs, &doc, caret0, &b, cursor, out)
            {
                break 'ev handled;
            }
            match event {
                Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                    self.ui.set_focus(Some(self.id));
                    if fs.open {
                        let [prev, next, close] = self.find_buttons(&b);
                        if close.contains(cursor) {
                            fs.open = false;
                            fs.focus = false;
                            break 'ev true;
                        }
                        if prev.contains(cursor) || next.contains(cursor) {
                            self.step_match(&mut e, &fs.query, prev.contains(cursor));
                            break 'ev true;
                        }
                        if self.find_rect(&b).contains(cursor) {
                            fs.focus = true;
                            fs.fresh = true;
                            break 'ev true;
                        }
                    }
                    fs.focus = false;
                    let bar = self.max_scroll_y(&b, &doc) > 0.0
                        && cursor.x >= view.origin.x + view.size.width - BAR - 6.0
                        && cursor.y < view.origin.y + view.size.height;
                    if bar {
                        let h = view.size.height;
                        let total = doc.len() as f32 * self.lh() + self.lh();
                        let th = (h * h / total).clamp(28.0, h);
                        let grab = th / 2.0;
                        self.ui
                            .set_slot(self.id ^ K_DRAG, Some((grab * 16.0) as i64));
                        let travel = (h - th).max(1.0);
                        let top = (cursor.y - view.origin.y - grab).clamp(0.0, travel);
                        self.ui
                            .set_scroll_offset(self.id, top / travel * self.max_scroll_y(&b, &doc));
                        break 'ev true;
                    }
                    self.ui.set_pressed(Some(pkey));
                    let i = self.index_at(&b, &doc, e.scroll_x, cursor);
                    let now = self.ui.now();
                    let near = (cursor.x - e.click_at.0).abs() <= 4.0
                        && (cursor.y - e.click_at.1).abs() <= 4.0;
                    e.clicks = if e.clicks > 0 && near && now.saturating_sub(e.click_ms) < 500 {
                        e.clicks % 3 + 1
                    } else {
                        1
                    };
                    (e.click_ms, e.click_at) = (now, (cursor.x, cursor.y));
                    let in_gutter = self.numbers && cursor.x < view.origin.x + self.gutter(&doc);
                    if in_gutter {
                        let (s, z) = line_range(&self.value, i);
                        let z = (z + 1).min(doc.nchars);
                        (e.anchor, e.caret) = (s, z);
                    } else {
                        match e.clicks {
                            2 => (e.anchor, e.caret) = word_bounds(&self.value, i),
                            3 => (e.anchor, e.caret) = line_range(&self.value, i),
                            _ => e.move_to(i, false),
                        }
                    }
                    e.goal_col = None;
                    e.preedit.clear();
                    true
                }
                Event::CursorMoved(p) if self.ui.slot(self.id ^ K_DRAG).is_some() => {
                    let g = self.ui.slot(self.id ^ K_DRAG).unwrap_or(0) as f32 / 16.0;
                    let h = view.size.height;
                    let total = doc.len() as f32 * self.lh() + self.lh();
                    let th = (h * h / total).clamp(28.0, h);
                    let travel = (h - th).max(1.0);
                    let top = (p.y - view.origin.y - g).clamp(0.0, travel);
                    self.ui
                        .set_scroll_offset(self.id, top / travel * self.max_scroll_y(&b, &doc));
                    return true;
                }
                Event::CursorMoved(p) if self.ui.pressed() == Some(pkey) => {
                    // Drag-select, and scroll when the pointer leaves the view.
                    let lh = self.lh();
                    let mut off = self.ui.scroll_offset(self.id);
                    if p.y < view.origin.y {
                        off -= lh;
                    } else if p.y > view.origin.y + view.size.height {
                        off += lh;
                    }
                    self.ui
                        .set_scroll_offset(self.id, off.clamp(0.0, self.max_scroll_y(&b, &doc)));
                    let i = self.index_at(&b, &doc, e.scroll_x, *p);
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
                Event::Scrolled { dx, dy } if b.contains(cursor) => {
                    let (oy, ox) = (self.ui.scroll_offset(self.id), e.scroll_x);
                    let (dx, dy) = (*dx, *dy);
                    self.ui.set_scroll_offset(
                        self.id,
                        (oy - dy).clamp(0.0, self.max_scroll_y(&b, &doc)),
                    );
                    e.scroll_x = (ox - dx).clamp(0.0, self.max_scroll_x(&b, &doc));
                    self.ui.scroll_offset(self.id) != oy || e.scroll_x != ox
                }
                Event::ImePreedit(s) if focused && editable => {
                    e.preedit = s.clone();
                    true
                }
                Event::TextInput(s) if focused && fs.focus => {
                    if fs.fresh {
                        fs.query.clear();
                        fs.fresh = false;
                    }
                    fs.query.push_str(&sanitize(s, false));
                    self.step_from_caret(&mut e, &fs.query);
                    true
                }
                Event::TextInput(s) if focused => {
                    e.preedit.clear();
                    let clean = sanitize(s, true);
                    if editable && !clean.is_empty() {
                        let mut it = clean.chars();
                        let v = match (it.next(), it.next()) {
                            (Some(c), None) => self.typed(&mut e, c),
                            _ => None,
                        };
                        match v {
                            Some(v) if v == self.value => {} // stepped over a closer
                            Some(v) => self.commit(v, caret0, out),
                            None => {
                                let v = e.insert(&self.value, &clean);
                                self.commit(v, caret0, out);
                            }
                        }
                    }
                    true
                }
                Event::KeyPressed(k, m) if focused && fs.focus => {
                    match k {
                        Key::Escape => {
                            fs.open = false;
                            fs.focus = false;
                        }
                        Key::Enter => {
                            fs.fresh = false;
                            self.step_match(&mut e, &fs.query, m.shift);
                        }
                        Key::Backspace => {
                            if fs.fresh {
                                fs.query.clear();
                            } else if m.alt || m.ctrl {
                                let t = fs.query.trim_end().len();
                                let cut = fs.query[..t].rfind(' ').map_or(0, |i| i + 1);
                                fs.query.truncate(cut);
                            } else {
                                fs.query.pop();
                            }
                            fs.fresh = false;
                            self.step_from_caret(&mut e, &fs.query);
                        }
                        Key::Character(c) if primary(m) && c.eq_ignore_ascii_case("v") => {
                            if let Some(t) = self.ui.clipboard().get() {
                                if fs.fresh {
                                    fs.query.clear();
                                }
                                fs.fresh = false;
                                fs.query.push_str(&sanitize(&t, false));
                                self.step_from_caret(&mut e, &fs.query);
                            }
                        }
                        Key::Character(c) if primary(m) && c.eq_ignore_ascii_case("f") => {
                            fs.fresh = true;
                        }
                        Key::Tab => {
                            fs.focus = false;
                        }
                        _ => {}
                    }
                    true
                }
                Event::KeyPressed(k, _) if focused && !e.preedit.is_empty() => {
                    !matches!(k, Key::Tab)
                }
                Event::KeyPressed(k, m) if focused => {
                    let (line, col) = doc.line_col(e.caret);
                    let page = (((view.size.height) / self.lh()) as usize).max(2) - 1;
                    let vertical = |e: &mut Edit, to: usize| {
                        let goal = e.goal_col.unwrap_or(col);
                        let to = to.min(doc.len() - 1);
                        e.move_to(doc.index_of(&self.value, to, goal), m.shift);
                        e.goal_col = Some(goal);
                    };
                    let cmd = primary(m);
                    let mut changed: Option<String> = None;
                    let handled = match k {
                        Key::Character(c) if cmd && c.eq_ignore_ascii_case("f") => {
                            fs.open = true;
                            fs.focus = true;
                            fs.fresh = true;
                            let sel = e.selected(&self.value);
                            if !sel.is_empty() && !sel.contains('\n') {
                                fs.query = sel;
                            }
                            true
                        }
                        Key::Character(c) if cmd && c == "/" && editable => {
                            changed = self.toggle_comment(&doc, &mut e);
                            true
                        }
                        Key::Character(c) if cmd && (c == "]" || c == "[") && editable => {
                            changed = Some(self.indent_lines(&doc, &mut e, c == "["));
                            true
                        }
                        Key::Character(c) if cmd && c.eq_ignore_ascii_case("d") && editable => {
                            // Duplicate the line (or selection) below it.
                            let (f, l) = self.block_lines(&doc, &e);
                            let (lo, hi) = (doc.starts[f].0, doc.starts[l].1);
                            let chunk = self.value[lo..hi].to_string();
                            let n = chunk.chars().count() + 1;
                            let mut v = self.value.clone();
                            v.insert_str(hi, &format!("\n{chunk}"));
                            e.caret += n;
                            e.anchor += n;
                            changed = Some(v);
                            true
                        }
                        Key::Character(_)
                            if editable
                                && history_edit(
                                    &self.ui,
                                    self.id,
                                    &self.value,
                                    &mut e,
                                    (k, m),
                                    &self.on_input,
                                    out,
                                ) =>
                        {
                            true
                        }
                        Key::Escape => {
                            if fs.open {
                                fs.open = false;
                            } else {
                                self.ui.set_focus(None);
                            }
                            true
                        }
                        Key::Tab if editable => {
                            let multi = e.has_selection()
                                && doc.line_col(e.range().0).0 != doc.line_col(e.range().1).0;
                            changed = Some(if m.shift || multi {
                                self.indent_lines(&doc, &mut e, m.shift)
                            } else {
                                let cells = cells(
                                    &doc.line(&self.value, line)
                                        [..byte_col(doc.line(&self.value, line), col)],
                                    self.tab,
                                );
                                let n = self.tab - cells % self.tab;
                                e.insert(&self.value, &" ".repeat(n))
                            });
                            true
                        }
                        Key::Tab => false,
                        Key::Enter if editable => {
                            changed = Some(self.newline(&doc, &mut e));
                            true
                        }
                        Key::Enter if fs.open => {
                            self.step_match(&mut e, &fs.query, m.shift);
                            true
                        }
                        Key::Backspace
                            if editable && self.backspace_pair(&mut e.clone()).is_some() =>
                        {
                            changed = self.backspace_pair(&mut e);
                            true
                        }
                        Key::Named(n) => match (n.as_str(), cmd) {
                            ("F3", _) if fs.open => {
                                self.step_match(&mut e, &fs.query, m.shift);
                                true
                            }
                            ("ArrowUp", _) => {
                                vertical(&mut e, line.saturating_sub(1));
                                true
                            }
                            ("ArrowDown", _) => {
                                vertical(&mut e, line + 1);
                                true
                            }
                            ("PageUp", _) => {
                                vertical(&mut e, line.saturating_sub(page));
                                true
                            }
                            ("PageDown", _) => {
                                vertical(&mut e, line + page);
                                true
                            }
                            ("Home", true) => {
                                e.move_to(0, m.shift);
                                true
                            }
                            ("End", true) => {
                                e.move_to(doc.nchars, m.shift);
                                true
                            }
                            ("Home", false) => {
                                // Smart home: first non-blank, then column 0.
                                let text = doc.line(&self.value, line);
                                let ind = text.chars().take_while(|c| c.is_whitespace()).count();
                                let to = if col == ind { 0 } else { ind };
                                e.move_to(doc.index_of(&self.value, line, to), m.shift);
                                e.goal_col = None;
                                true
                            }
                            ("End", false) => {
                                e.move_to(doc.index_of(&self.value, line, usize::MAX), m.shift);
                                e.goal_col = None;
                                true
                            }
                            _ => false,
                        },
                        _ => false,
                    };
                    if let Some(v) = changed {
                        e.goal_col = None;
                        if v != self.value {
                            self.commit(v, caret0, out);
                        }
                        true
                    } else if handled {
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
            }
        };
        if consumed {
            e.blink_epoch = self.ui.now();
        }
        if fs != fs0 {
            self.ui.state(self.id, |s: &mut FindState| *s = fs);
        }
        if mc != mc0 {
            self.ui.state(self.id ^ K_MULTI, |m: &mut Multi| *m = mc);
        }
        self.ui.set_edit(self.id, e);
        consumed
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let doc = Doc::get(self.id, &self.value);
        let focused = self.ui.is_focused(Some(self.id));
        let editable = self.on_input.is_some();
        self.ui.register(self.id);
        self.ui.describe(Role::TextEditor, "Code editor", &b, |n| {
            n.id = Some(self.id);
            n.value = Some(self.value.clone());
            n.enabled = editable;
        });
        let mut e = self.ui.edit(self.id);
        e.clamp(doc.nchars);
        let mut fs: FindState = self.ui.state(self.id, |s: &mut FindState| s.clone());
        let (lh, cw, size) = (self.lh(), self.cw(), self.size());
        let view = self.view(&b);
        let gutter = self.gutter(&doc);
        let (cl, cc) = doc.line_col(e.caret);
        let mc = self.multi(&doc);

        // Keep the caret in view, but only when it (or the text) moved: otherwise the
        // wheel could never scroll away from the caret.
        let marker = e.caret ^ (doc.nchars << 20);
        let mut off = self
            .ui
            .scroll_offset(self.id)
            .min(self.max_scroll_y(&b, &doc));
        let text_w = (view.size.width - gutter - 16.0).max(cw);
        let caret_cells = cells(
            &doc.line(&self.value, cl)[..byte_col(doc.line(&self.value, cl), cc)],
            self.tab,
        ) as f32
            * cw;
        if fs.last != marker {
            if focused {
                let vh = view.size.height;
                if (cl as f32) * lh < off {
                    off = cl as f32 * lh;
                }
                if (cl as f32 + 1.0) * lh + lh * 0.5 > off + vh {
                    off = (cl as f32 + 1.5) * lh - vh;
                }
                if caret_cells - e.scroll_x > text_w - 2.0 * cw {
                    e.scroll_x = caret_cells - text_w + 2.0 * cw;
                }
                if caret_cells < e.scroll_x {
                    e.scroll_x = (caret_cells - 4.0 * cw).max(0.0);
                }
            }
            fs.last = marker;
            self.ui.state(self.id, |s: &mut FindState| s.last = marker);
        }
        e.scroll_x = e.scroll_x.clamp(0.0, self.max_scroll_x(&b, &doc));
        off = off.clamp(0.0, self.max_scroll_y(&b, &doc));
        self.ui.set_scroll_offset(self.id, off);
        self.ui.set_edit(self.id, e.clone());

        // Frame.
        if focused {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        let hover = self.ui.hovered(&b);
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: p.surface,
            radius: t.radii.medium,
            border: Some(Border {
                color: mix(
                    p.border,
                    p.border_strong,
                    self.ui
                        .ease(Ui::anim_key(&b, 11), (focused || hover) as u8 as f32),
                ),
                width: 1.0,
            }),
            shadow: None,
        });
        out.push(DrawCommand::PushClip(view));
        let outer_clip = self.ui.clip();
        self.ui.set_clip(Some(view));
        let first = (off / lh).floor() as usize;
        let last = (((off + view.size.height) / lh).ceil() as usize + 1).min(doc.len());
        let ox = view.origin.x + gutter - e.scroll_x;
        let oy = view.origin.y + lh * 0.5 - off;
        let code_clip = Rectangle::new(
            Point::new(view.origin.x + gutter, view.origin.y),
            Size::new((view.size.width - gutter).max(0.0), view.size.height),
        );
        let (sa, sz) = e.range();
        let q = &fs.query;
        let ms = if fs.open && !q.is_empty() {
            self.matches(q)
        } else {
            Rc::new(Vec::new())
        };
        let quad = |r: Rectangle, c: Color, radius: f32| DrawCommand::Quad {
            bounds: r,
            fill: c,
            radius,
            border: None,
            shadow: None,
        };

        // Current line band (only while focused and not selecting).
        if focused && !e.has_selection() && cl >= first && cl < last {
            out.push(quad(
                Rectangle::new(
                    Point::new(view.origin.x, oy + cl as f32 * lh),
                    Size::new(view.size.width, lh),
                ),
                p.hover,
                0.0,
            ));
        }
        out.push(DrawCommand::PushClip(code_clip));
        let mut state = doc.state_at(&self.value, first, self.lang);
        // Indent guides, from the leading whitespace of each visible line.
        for i in first..last {
            let raw = doc.line(&self.value, i);
            let lead = cells(
                &raw[..raw.len() - raw.trim_start_matches([' ', '\t']).len()],
                self.tab,
            );
            if !raw.trim().is_empty() {
                for k in 0..lead / self.tab {
                    out.push(quad(
                        Rectangle::new(
                            Point::new(ox + (k * self.tab) as f32 * cw + 1.0, oy + i as f32 * lh),
                            Size::new(1.0, lh),
                        ),
                        p.border.with_alpha(0.55),
                        0.0,
                    ));
                }
            }
        }
        let visible_cells = (code_clip.size.width / cw) as usize + 8;
        let first_cell = (e.scroll_x / cw) as usize;
        for i in first..last {
            let raw = doc.line(&self.value, i);
            let y = oy + i as f32 * lh;
            let disp = expand_tabs(raw, self.tab);
            let (spans, next) = tokenize(self.lang, &disp, state);
            state = next;
            // Selection.
            let (ls, le) = (doc.cstart[i], doc.cstart[i] + raw.chars().count());
            if focused && e.has_selection() && sa <= le && sz > ls {
                let from = if sa > ls {
                    col_cells(raw, sa - ls, self.tab)
                } else {
                    0
                };
                let to = if sz > le {
                    col_cells(raw, usize::MAX, self.tab) + 1
                } else {
                    col_cells(raw, sz - ls, self.tab)
                };
                if to > from {
                    out.push(quad(
                        Rectangle::new(
                            Point::new(ox + from as f32 * cw, y),
                            Size::new((to - from) as f32 * cw, lh),
                        ),
                        p.selection,
                        0.0,
                    ));
                }
            }
            // The other cursors' selections.
            for x in mc.extras.iter().filter(|x| !x.is_empty()) {
                let (xa, xz) = x.range();
                if focused && xa <= le && xz > ls {
                    let from = if xa > ls {
                        col_cells(raw, xa - ls, self.tab)
                    } else {
                        0
                    };
                    let to = if xz > le {
                        col_cells(raw, usize::MAX, self.tab) + 1
                    } else {
                        col_cells(raw, xz - ls, self.tab)
                    };
                    if to > from {
                        out.push(quad(
                            Rectangle::new(
                                Point::new(ox + from as f32 * cw, y),
                                Size::new((to - from) as f32 * cw, lh),
                            ),
                            p.selection,
                            0.0,
                        ));
                    }
                }
            }
            // Search hits on this line.
            if !ms.is_empty() {
                let lo = ms.partition_point(|m| m.0 + m.1 <= ls);
                for &(s, l) in ms[lo..].iter().take_while(|m| m.0 <= le) {
                    let from = col_cells(raw, s.saturating_sub(ls), self.tab);
                    let to = col_cells(raw, (s + l).saturating_sub(ls).min(le - ls), self.tab);
                    let current = (s, s + l) == (sa, sz);
                    out.push(quad(
                        Rectangle::new(
                            Point::new(ox + from as f32 * cw, y + 1.0),
                            Size::new(((to - from) as f32 * cw).max(2.0), lh - 2.0),
                        ),
                        if current {
                            p.accent.with_alpha(0.38)
                        } else {
                            p.warning.with_alpha(0.28)
                        },
                        t.radii.small,
                    ));
                }
            }
            // Text, one run per token, skipping what is scrolled out of view.
            let mut at = 0usize;
            let run = |from: usize, to: usize, color: Color, out: &mut DrawList| {
                if to <= from {
                    return;
                }
                let c0 = cells(&disp[..from], self.tab);
                let n = cells(&disp[from..to], self.tab);
                if c0 + n < first_cell || c0 > first_cell + visible_cells {
                    return;
                }
                out.push(DrawCommand::Text {
                    family: FontFamily::Mono,
                    weight: 400,
                    line_height: Some(lh),
                    origin: Point::new(ox + c0 as f32 * cw, y),
                    content: disp[from..to].to_string(),
                    size,
                    color,
                    max_width: None,
                });
            };
            for s in &spans {
                run(at, s.start, p.text, out);
                run(s.start, s.end, token_color(&t, s.kind), out);
                at = s.end;
            }
            run(at, disp.len(), p.text, out);
        }
        if !e.preedit.is_empty() {
            let x = ox + caret_cells;
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
            let w = e.preedit.chars().count() as f32 * cw;
            out.push(quad(
                Rectangle::new(Point::new(x, y + lh - 2.0), Size::new(w, 1.0)),
                p.text,
                0.0,
            ));
        }
        let blink = (self.ui.now().saturating_sub(e.blink_epoch) / 530) % 2 == 0;
        if focused && editable && blink && !fs.focus {
            out.push(quad(
                Rectangle::new(
                    Point::new(ox + caret_cells, oy + cl as f32 * lh),
                    Size::new(1.5, lh),
                ),
                p.text,
                0.0,
            ));
        }
        if focused && editable && blink && !fs.focus {
            for x in &mc.extras {
                let (l, c) = doc.line_col(x.caret);
                if l < first || l >= last {
                    continue;
                }
                let raw = doc.line(&self.value, l);
                let at = cells(&raw[..byte_col(raw, c)], self.tab) as f32 * cw;
                out.push(quad(
                    Rectangle::new(Point::new(ox + at, oy + l as f32 * lh), Size::new(1.5, lh)),
                    p.text,
                    0.0,
                ));
            }
        }
        // The bracket beside the caret and its match.
        if focused {
            let line = doc.line(&self.value, cl);
            let caret_byte = doc.starts[cl.min(doc.len() - 1)].0 + byte_col(line, cc);
            let mark = |byte: usize, good: bool, out: &mut DrawList| {
                let l = doc.starts.partition_point(|s| s.0 <= byte).saturating_sub(1);
                if l < first || l >= last {
                    return;
                }
                let raw = doc.line(&self.value, l);
                let col = self.value[doc.starts[l].0..byte].chars().count();
                let at = cells(&raw[..byte_col(raw, col)], self.tab) as f32 * cw;
                let ink = if good { p.accent } else { p.danger };
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(ox + at, oy + l as f32 * lh + 1.0),
                        Size::new(cw, lh - 2.0),
                    ),
                    fill: ink.with_alpha(0.2),
                    radius: t.radii.small,
                    border: Some(Border {
                        color: ink.with_alpha(0.8),
                        width: 1.0,
                    }),
                    shadow: None,
                });
            };
            match code_ops::bracket_pair(&self.value, caret_byte) {
                Some(Ok((a, z))) => {
                    mark(a, true, out);
                    mark(z, true, out);
                }
                Some(Err(a)) => mark(a, false, out),
                None => {}
            }
        }
        out.push(DrawCommand::PopClip);

        // Gutter.
        if self.numbers {
            out.push(quad(
                Rectangle::new(view.origin, Size::new(gutter, view.size.height)),
                p.surface,
                0.0,
            ));
            out.push(quad(
                Rectangle::new(
                    Point::new(view.origin.x + gutter - 1.0, view.origin.y),
                    Size::new(1.0, view.size.height),
                ),
                p.border.with_alpha(0.6),
                0.0,
            ));
            for i in first..last {
                let s = (i + 1).to_string();
                let w = s.len() as f32 * cw;
                let active = focused && i >= doc.line_col(sa).0 && i <= doc.line_col(sz).0;
                out.push(DrawCommand::Text {
                    family: FontFamily::Mono,
                    weight: 400,
                    line_height: Some(lh),
                    origin: Point::new(view.origin.x + gutter - 14.0 - w, oy + i as f32 * lh),
                    content: s,
                    size,
                    color: if active {
                        p.text
                    } else {
                        mix(p.text_secondary, p.surface, 0.3)
                    },
                    max_width: None,
                });
            }
        }
        self.ui.set_clip(outer_clip);
        out.push(DrawCommand::PopClip);

        // Scrollbars.
        let live = self.ui.slot(self.id ^ K_DRAG).is_some();
        let max_y = self.max_scroll_y(&b, &doc);
        if max_y > 0.0 {
            let h = view.size.height;
            let total = doc.len() as f32 * lh + lh;
            let th = (h * h / total).clamp(28.0, h);
            let y = view.origin.y + (h - th) * off / max_y;
            let r = Rectangle::new(
                Point::new(view.origin.x + view.size.width - BAR - 2.0, y),
                Size::new(BAR, th),
            );
            out.push(quad(
                r,
                if live || self.ui.hovered(&r) {
                    p.border_strong
                } else {
                    p.border_strong.with_alpha(0.5)
                },
                BAR / 2.0,
            ));
        }
        let max_x = self.max_scroll_x(&b, &doc);
        if max_x > 0.0 {
            let w = view.size.width - gutter;
            let total = w + max_x;
            let tw = (w * w / total).clamp(28.0, w);
            let x = view.origin.x + gutter + (w - tw) * e.scroll_x / max_x;
            out.push(quad(
                Rectangle::new(
                    Point::new(x, view.origin.y + view.size.height - BAR - 2.0),
                    Size::new(tw, BAR),
                ),
                p.border_strong.with_alpha(0.5),
                BAR / 2.0,
            ));
        }

        // Status bar.
        if self.status {
            let r = Rectangle::new(
                Point::new(view.origin.x, view.origin.y + view.size.height),
                Size::new(view.size.width, STATUS_H),
            );
            out.push(quad(r, p.surface_raised, 0.0));
            out.push(quad(
                Rectangle::new(r.origin, Size::new(r.size.width, 1.0)),
                p.border.with_alpha(0.7),
                0.0,
            ));
            let small = t.typography.caption;
            let say = |x: f32, s: String, color: Color, out: &mut DrawList| {
                out.push(DrawCommand::Text {
                    family: FontFamily::Sans,
                    weight: 400,
                    line_height: Some(r.size.height),
                    origin: Point::new(x, r.origin.y + 1.0),
                    content: s,
                    size: small.size,
                    color,
                    max_width: None,
                });
            };
            let sel = sz - sa;
            let mut pos = if sel > 0 {
                format!("Ln {}, Col {}  ·  {} selected", cl + 1, cc + 1, sel)
            } else {
                format!("Ln {}, Col {}", cl + 1, cc + 1)
            };
            if !mc.extras.is_empty() {
                pos.push_str(&format!("  ·  {} cursors", mc.extras.len() + 1));
            }
            say(r.origin.x + 12.0, pos, p.text_secondary, out);
            let name = format!("{} lines  ·  {}", doc.len(), lang_name(self.lang));
            let w = self.ui.metrics().width(&name, small.size);
            say(
                r.origin.x + r.size.width - w - 12.0,
                name,
                p.text_secondary,
                out,
            );
        }

        // Find bar.
        if fs.open {
            let f = self.find_rect(&b);
            out.push(DrawCommand::Quad {
                bounds: f,
                fill: p.surface_raised,
                radius: t.radii.medium,
                border: Some(Border {
                    color: if fs.focus { p.accent } else { p.border_strong },
                    width: 1.0,
                }),
                shadow: Some(t.elevation.medium),
            });
            let none = !fs.query.is_empty() && ms.is_empty();
            let body = t.typography.body;
            let mut label = fs.query.clone();
            if label.is_empty() {
                label = "Find".into();
            }
            out.push(DrawCommand::PushClip(Rectangle::new(
                Point::new(f.origin.x + 10.0, f.origin.y),
                Size::new(FIND_W - 10.0 - 26.0 * 3.0 - 70.0, FIND_H),
            )));
            out.push(DrawCommand::Text {
                family: FontFamily::Sans,
                weight: 400,
                line_height: Some(FIND_H),
                origin: Point::new(f.origin.x + 10.0, f.origin.y),
                content: label,
                size: body.size,
                color: if fs.query.is_empty() {
                    p.text_secondary.with_alpha(0.7)
                } else if none {
                    p.danger
                } else {
                    p.text
                },
                max_width: None,
            });
            out.push(DrawCommand::PopClip);
            let count = if fs.query.is_empty() {
                String::new()
            } else if ms.is_empty() {
                "No results".to_string()
            } else {
                match ms.iter().position(|m| (m.0, m.0 + m.1) == (sa, sz)) {
                    Some(i) => format!("{} of {}", i + 1, ms.len()),
                    None => format!("{} found", ms.len()),
                }
            };
            let cwid = self.ui.metrics().width(&count, t.typography.caption.size);
            let bx = self.find_buttons(&b);
            out.push(DrawCommand::Text {
                family: FontFamily::Sans,
                weight: 400,
                line_height: Some(FIND_H),
                origin: Point::new(bx[0].origin.x - cwid - 8.0, f.origin.y),
                content: count,
                size: t.typography.caption.size,
                color: p.text_secondary,
                max_width: None,
            });
            for (r, glyph) in bx.iter().zip(["\u{2191}", "\u{2193}", "\u{00d7}"]) {
                if self.ui.hovered(r) {
                    out.push(quad(*r, p.hover, t.radii.small));
                }
                let gw = self.ui.metrics().width(glyph, body.size);
                out.push(DrawCommand::Text {
                    family: FontFamily::Sans,
                    weight: 400,
                    line_height: Some(r.size.height),
                    origin: Point::new(r.origin.x + (r.size.width - gw) / 2.0, r.origin.y),
                    content: glyph.into(),
                    size: body.size,
                    color: p.text_secondary,
                    max_width: None,
                });
            }
        }
    }
}

impl<M: 'static> CodeEditor<M> {
    /// After the query changed, select the first match at or after the selection start.
    fn step_from_caret(&self, e: &mut Edit, query: &str) {
        let ms = self.matches(query);
        let from = e.range().0;
        if let Some(&(s, l)) = ms.iter().find(|m| m.0 >= from).or(ms.first()) {
            e.anchor = s;
            e.caret = s + l;
        }
    }
}

fn lang_name(l: Lang) -> &'static str {
    match l {
        Lang::Plain => "Plain text",
        Lang::Rust => "Rust",
        Lang::Python => "Python",
        Lang::JavaScript => "JavaScript",
        Lang::Json => "JSON",
        Lang::Toml => "TOML",
        Lang::Shell => "Shell",
        Lang::C => "C / C++",
        Lang::Sql => "SQL",
        Lang::Go => "Go",
        Lang::Java => "Java",
        Lang::Kotlin => "Kotlin",
        Lang::Swift => "Swift",
        Lang::Ruby => "Ruby",
        Lang::Yaml => "YAML",
        Lang::Css => "CSS",
    }
}

/// Colours for token kinds, derived from the palette so every theme and the forced-colours
/// look stay legible.
pub(crate) fn token_color(t: &alloy_ui_theme::Theme, k: Token) -> Color {
    let p = &t.palette;
    match k {
        Token::Plain => p.text,
        Token::Keyword => p.accent,
        Token::Type => mix(p.accent, p.text, 0.5),
        Token::Function => mix(p.text, p.accent, 0.22),
        Token::String => p.success,
        Token::Number => p.warning,
        Token::Comment => p.text_secondary,
        Token::Punct => mix(p.text_secondary, p.text, 0.45),
        Token::Attr => mix(p.success, p.accent, 0.5),
    }
}

// -------------------------------------------------------------- text helpers

/// Display cells one character takes: 2 for East Asian wide, 0 for combining marks.
fn cell_width(c: char) -> usize {
    match c as u32 {
        0x0300..=0x036F | 0x200B..=0x200F | 0xFE00..=0xFE0F => 0,
        0x1100..=0x115F
        | 0x2E80..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}
/// Cells `s` spans when it starts at column 0, expanding tabs to `tab` stops.
fn cells(s: &str, tab: usize) -> usize {
    let mut n = 0;
    for c in s.chars() {
        n += if c == '\t' {
            tab - n % tab
        } else {
            cell_width(c)
        };
    }
    n
}
/// Cells before character column `col` of `line` (clamped to its end).
fn col_cells(line: &str, col: usize, tab: usize) -> usize {
    cells(&line[..byte_col(line, col)], tab)
}
fn byte_col(line: &str, col: usize) -> usize {
    line.char_indices().nth(col).map_or(line.len(), |x| x.0)
}
/// Character column whose left edge is nearest `cell`.
fn col_at(line: &str, cell: f32, tab: usize) -> usize {
    let mut n = 0usize;
    for (i, c) in line.chars().enumerate() {
        let w = if c == '\t' {
            tab - n % tab
        } else {
            cell_width(c)
        };
        if cell < n as f32 + w as f32 / 2.0 {
            return i;
        }
        n += w;
    }
    line.chars().count()
}
fn expand_tabs(line: &str, tab: usize) -> std::borrow::Cow<'_, str> {
    if !line.contains('\t') {
        return line.into();
    }
    let mut s = String::with_capacity(line.len() + 8);
    let mut n = 0;
    for c in line.chars() {
        if c == '\t' {
            let k = tab - n % tab;
            s.extend(std::iter::repeat(' ').take(k));
            n += k;
        } else {
            s.push(c);
            n += cell_width(c);
        }
    }
    s.into()
}

// -------------------------------------------------------------- document index

/// Find-bar state, remembered per editor.
#[derive(Clone, Default, PartialEq)]
struct FindState {
    open: bool,
    focus: bool,
    /// The next typed character replaces the query (after Ctrl+F).
    fresh: bool,
    query: String,
    /// Marker of the caret/text the view last scrolled for.
    last: usize,
}

/// Line index of a text: byte and char starts of every line, cached between frames.
struct Doc {
    /// Hash of the text this was built from (see [`doc_hash`]).
    hash: u64,
    /// `(byte start, byte end)` of each line, newline excluded.
    starts: Vec<(usize, usize)>,
    cstart: Vec<usize>,
    nchars: usize,
    states: RefCell<Vec<State>>,
    widest: RefCell<Option<usize>>,
}

thread_local! {
    static DOCS: RefCell<HashMap<u64, (u64, Rc<Doc>)>> = RefCell::new(HashMap::new());
    #[allow(clippy::type_complexity)]
    static FINDS: RefCell<HashMap<u64, (u64, String, Rc<Vec<(usize, usize)>>)>> = RefCell::new(HashMap::new());
}

fn hash_of(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write(s.as_bytes());
    h.finish()
}

/// What identifies a text for cached line indexes and multi-cursor validity.
fn doc_hash(s: &str) -> u64 {
    hash_of(s) ^ s.len() as u64
}

/// Extra carets and selections beyond the primary one in the shared [`Edit`]. Valid only for
/// the text they were made against: any other change (undo, an app-side edit) drops them.
#[derive(Clone, Default, PartialEq)]
struct Multi {
    extras: Vec<Sel>,
    hash: u64,
}

impl Doc {
    fn get(id: u64, text: &str) -> Rc<Doc> {
        let h = doc_hash(text);
        DOCS.with(|d| {
            let mut d = d.borrow_mut();
            if let Some((hh, doc)) = d.get(&id) {
                if *hh == h {
                    return doc.clone();
                }
            }
            let mut built = Doc::build(text);
            built.hash = h;
            let doc = Rc::new(built);
            d.insert(id, (h, doc.clone()));
            doc
        })
    }
    fn build(text: &str) -> Doc {
        let mut starts = Vec::new();
        let mut cstart = Vec::new();
        let (mut b, mut c) = (0usize, 0usize);
        for line in text.split('\n') {
            starts.push((b, b + line.len()));
            cstart.push(c);
            b += line.len() + 1;
            c += line.chars().count() + 1;
        }
        Doc {
            hash: 0,
            starts,
            cstart,
            nchars: text.chars().count(),
            states: RefCell::new(vec![State::Normal]),
            widest: RefCell::new(None),
        }
    }
    fn len(&self) -> usize {
        self.starts.len()
    }
    fn line<'a>(&self, text: &'a str, i: usize) -> &'a str {
        let (a, z) = self.starts[i.min(self.len() - 1)];
        text[a..z].trim_end_matches('\r')
    }
    fn line_col(&self, idx: usize) -> (usize, usize) {
        let i = self.cstart.partition_point(|&s| s <= idx).saturating_sub(1);
        (i, idx - self.cstart[i])
    }
    fn index_of(&self, text: &str, line: usize, col: usize) -> usize {
        let line = line.min(self.len() - 1);
        self.cstart[line] + col.min(self.line(text, line).chars().count())
    }
    /// The lexer state at the start of `line`, computed (and cached) from the lines above.
    fn state_at(&self, text: &str, line: usize, lang: Lang) -> State {
        if lang == Lang::Plain {
            return State::Normal;
        }
        let mut st = self.states.borrow_mut();
        while st.len() <= line {
            let i = st.len() - 1;
            let next = tokenize(lang, self.line(text, i), st[i]).1;
            st.push(next);
        }
        st[line]
    }
    /// Widest line in cells (sampled lazily; capped so a minified megabyte line stays cheap).
    fn widest(&self, text: &str, tab: usize) -> usize {
        *self.widest.borrow_mut().get_or_insert_with(|| {
            self.starts
                .iter()
                .map(|&(a, z)| {
                    if z - a > 20_000 {
                        z - a
                    } else {
                        cells(&text[a..z], tab)
                    }
                })
                .max()
                .unwrap_or(0)
        })
    }
}

/// Every occurrence of `query` as `(char start, char length)`. Smart case: a query with no
/// capitals ignores case (ASCII).
fn find_all(id: u64, text: &str, query: &str) -> Rc<Vec<(usize, usize)>> {
    let key = hash_of(text) ^ text.len() as u64;
    if let Some(hit) = FINDS.with(|f| {
        f.borrow()
            .get(&id)
            .filter(|(h, q, _)| *h == key && q == query)
            .map(|x| x.2.clone())
    }) {
        return hit;
    }
    let mut res = Vec::new();
    if !query.is_empty() {
        let fold = !query.chars().any(char::is_uppercase);
        let (hay, needle) = if fold {
            (text.to_ascii_lowercase(), query.to_ascii_lowercase())
        } else {
            (text.to_string(), query.to_string())
        };
        let qlen = query.chars().count();
        let (mut bytes, mut chars) = (0usize, 0usize);
        for (b, _) in hay.match_indices(&needle) {
            chars += text[bytes..b].chars().count();
            bytes = b;
            res.push((chars, qlen));
            if res.len() >= MAX_MATCHES {
                break;
            }
        }
    }
    let res = Rc::new(res);
    FINDS.with(|f| {
        f.borrow_mut()
            .insert(id, (key, query.to_string(), res.clone()))
    });
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_expand_tabs_and_wide_chars() {
        assert_eq!(cells("a\tb", 4), 5);
        assert_eq!(cells("\t\t", 4), 8);
        assert_eq!(cells("日本", 4), 4);
        assert_eq!(col_at("abc", 1.4, 4), 1);
        assert_eq!(col_at("abc", 99.0, 4), 3);
    }
    #[test]
    fn doc_index_round_trips() {
        let t = "ab\n\ncde";
        let d = Doc::build(t);
        assert_eq!(d.len(), 3);
        for i in 0..=t.chars().count() {
            let (l, c) = d.line_col(i);
            assert_eq!(d.index_of(t, l, c), i);
        }
        assert_eq!(d.line(t, 2), "cde");
    }
    #[test]
    fn find_is_smart_case_and_counts_chars() {
        let m = find_all(1, "Foo foo FOO é foo", "foo");
        assert_eq!(&m[..], &[(0, 3), (4, 3), (8, 3), (14, 3)]);
        let m = find_all(1, "Foo foo FOO", "Foo");
        assert_eq!(&m[..], &[(0, 3)]);
    }
    #[test]
    fn lexer_state_is_cached_per_line() {
        let t = "a /* x\ny\nz */ b";
        let d = Doc::build(t);
        assert_eq!(d.state_at(t, 2, Lang::Rust), State::Block);
        assert_eq!(d.state_at(t, 0, Lang::Rust), State::Normal);
    }
}
