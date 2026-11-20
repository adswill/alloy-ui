//! Caret and selection editing, independent of drawing so it can be tested.

use crate::ui::Clipboard;
use alloy_ui_core::{Key, Modifiers};
use unicode_segmentation::UnicodeSegmentation;

/// Caret and selection state of one text field. Indices count `char`s but
/// always sit on grapheme-cluster boundaries (see `prev_grapheme`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Edit {
    /// Caret position.
    pub caret: usize,
    /// Selection anchor; equals `caret` when nothing is selected.
    pub anchor: usize,
    /// IME composition text currently shown at the caret.
    pub preedit: String,
    /// Horizontal scroll of the text inside the field.
    pub scroll_x: f32,
    /// Column the caret tries to keep when moving up and down (multi-line fields).
    pub goal_col: Option<usize>,
    /// Time of the last edit or caret move; the caret blink restarts from it.
    pub blink_epoch: u64,
    /// Consecutive mouse presses at about the same spot (1 single, 2 double, 3 triple).
    pub clicks: u8,
    /// Time of the last mouse press, for multi-click detection.
    pub click_ms: u64,
    /// Position of the last mouse press.
    pub click_at: (f32, f32),
}

impl Edit {
    /// Selected range as `(start, end)`, start <= end.
    pub fn range(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }
    /// True when text is selected.
    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }
    /// Keep indices within `len` characters.
    pub fn clamp(&mut self, len: usize) {
        self.caret = self.caret.min(len);
        self.anchor = self.anchor.min(len);
    }
    /// Move the caret to `to`; keep the anchor when `extend`.
    pub fn move_to(&mut self, to: usize, extend: bool) {
        self.caret = to;
        if !extend {
            self.anchor = to;
        }
    }
    /// Select everything.
    pub fn select_all(&mut self, len: usize) {
        self.anchor = 0;
        self.caret = len;
    }
    /// The selected text of `value`.
    pub fn selected(&self, value: &str) -> String {
        let (a, b) = self.range();
        value.chars().skip(a).take(b - a).collect()
    }
    /// Replace the selection (or insert at the caret) with `text`; returns the new value.
    pub fn insert(&mut self, value: &str, text: &str) -> String {
        let (a, b) = self.range();
        let mut out: String = value.chars().take(a).collect();
        out.push_str(text);
        out.extend(value.chars().skip(b));
        self.caret = a + text.chars().count();
        self.anchor = self.caret;
        out
    }
    /// Delete the selection, or one grapheme cluster before (`forward == false`)
    /// or after the caret. Returns the new value, or `None` if nothing changed.
    pub fn delete(&mut self, value: &str, forward: bool) -> Option<String> {
        let len = value.chars().count();
        if !self.has_selection() {
            if forward && self.caret < len {
                self.anchor = next_grapheme(value, self.caret);
            } else if !forward && self.caret > 0 {
                self.anchor = prev_grapheme(value, self.caret);
            } else {
                return None;
            }
        }
        Some(self.insert(value, ""))
    }
}

/// What [`apply_key`] did with a key press.
#[derive(Debug, PartialEq)]
pub(crate) enum KeyOutcome {
    /// Not an editing key; let someone else have it.
    Ignored,
    /// Caret or selection moved (or a copy happened); the value is unchanged.
    Handled,
    /// The value changed to this string.
    Changed(String),
}

/// Editing keys shared by [`crate::TextInput`] and [`crate::TextEditor`]:
/// arrows with shift/word/line modifiers, Backspace, Delete and the
/// select-all / copy / cut / paste shortcuts. Enter, Escape, Tab and
/// (for multi-line fields) Home/End and vertical movement are the caller's.
pub(crate) fn apply_key(
    e: &mut Edit,
    value: &str,
    key: &Key,
    m: &Modifiers,
    clipboard: &dyn Clipboard,
    editable: bool,
    multiline: bool,
) -> KeyOutcome {
    use KeyOutcome::*;
    let len = value.chars().count();
    let mac = is_mac();
    let word = word_mod(m);
    let rtl = line_is_rtl(value, e.caret);
    let changed = |v: Option<String>| v.map_or(Handled, Changed);
    match key {
        Key::Backspace if editable => {
            if !e.has_selection() && word {
                e.anchor = word_left(value, e.caret);
            }
            changed(e.delete(value, false))
        }
        Key::Named(n) => match n.as_str() {
            "ArrowLeft" | "ArrowRight" => {
                let left = n == "ArrowLeft";
                // Plain arrows are visual: in a right-to-left line Left goes
                // logically forward. Word and line moves stay logical.
                let back = left != rtl;
                let to = if e.has_selection() && !m.shift {
                    if back {
                        e.range().0
                    } else {
                        e.range().1
                    }
                } else if m.logo {
                    if left {
                        0
                    } else {
                        len
                    }
                } else if word {
                    if left {
                        word_left(value, e.caret)
                    } else {
                        word_right(value, e.caret)
                    }
                } else if back {
                    prev_grapheme(value, e.caret)
                } else {
                    next_grapheme(value, e.caret)
                };
                e.move_to(to, m.shift);
                Handled
            }
            "Home" if !multiline => {
                e.move_to(0, m.shift);
                Handled
            }
            "End" if !multiline => {
                e.move_to(len, m.shift);
                Handled
            }
            "Delete" if editable => {
                if !e.has_selection() && word {
                    e.anchor = word_right(value, e.caret);
                }
                changed(e.delete(value, true))
            }
            _ => Ignored,
        },
        // Cocoa's Emacs bindings: Ctrl+A / Ctrl+E are line start / end.
        Key::Character(c) if mac && m.ctrl && !m.logo && matches!(c.as_str(), "a" | "e") => {
            let (start, end) = line_range(value, e.caret);
            e.move_to(if c == "a" { start } else { end }, m.shift);
            Handled
        }
        Key::Character(c) if primary(m) => match c.to_lowercase().as_str() {
            "a" => {
                e.select_all(len);
                Handled
            }
            "c" => {
                if e.has_selection() {
                    clipboard.set(e.selected(value));
                }
                Handled
            }
            "x" if editable => {
                if e.has_selection() {
                    clipboard.set(e.selected(value));
                    return changed(e.delete(value, false));
                }
                Handled
            }
            "v" if editable => match clipboard.get() {
                Some(text) => Changed(e.insert(value, &sanitize(&text, multiline))),
                None => Handled,
            },
            _ => Ignored,
        },
        _ => Ignored,
    }
}

/// True when compiled for macOS, whose text shortcuts differ from the rest.
pub(crate) fn is_mac() -> bool {
    cfg!(target_os = "macos")
}

/// The shortcut modifier: Cmd on macOS, Ctrl elsewhere. Super off macOS is
/// the window manager's, and Ctrl+Alt there is AltGr, so neither counts.
pub(crate) fn primary(m: &Modifiers) -> bool {
    if is_mac() {
        m.logo && !m.ctrl
    } else {
        m.ctrl && !m.alt && !m.logo
    }
}

/// The word-wise movement modifier: Option on macOS, Ctrl elsewhere.
pub(crate) fn word_mod(m: &Modifiers) -> bool {
    if is_mac() {
        m.alt && !m.ctrl && !m.logo
    } else {
        m.ctrl && !m.alt && !m.logo
    }
}

/// Strip control characters (keeping newlines when `multiline`). CRLF and a
/// lone CR (classic Mac line ending) both count as one newline.
pub(crate) fn sanitize(text: &str, multiline: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let c = if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            '\n'
        } else {
            c
        };
        if !c.is_control() || (multiline && c == '\n') {
            out.push(c);
        }
    }
    out
}

/// Undo/redo stack of one text field. Runs of single-character typing within
/// a second collapse into one step, so Ctrl+Z removes a burst, not a letter.
#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<(String, usize)>,
    redo: Vec<(String, usize)>,
    last_ms: u64,
    typing: bool,
}

impl History {
    const LIMIT: usize = 200;

    /// Note that `before` (with the caret at `caret`) became `after` at time `now`.
    pub(crate) fn record(&mut self, before: &str, caret: usize, after: &str, now: u64) {
        let typing = after.chars().count() == before.chars().count() + 1;
        let merge = typing
            && self.typing
            && !self.undo.is_empty()
            && now.saturating_sub(self.last_ms) < 1000;
        if !merge {
            self.undo.push((before.to_owned(), caret));
            if self.undo.len() > Self::LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.typing = typing;
        self.last_ms = now;
    }
    fn undo(&mut self, current: &str, caret: usize) -> Option<(String, usize)> {
        let prev = self.undo.pop()?;
        self.redo.push((current.to_owned(), caret));
        self.typing = false;
        Some(prev)
    }
    fn redo(&mut self, current: &str, caret: usize) -> Option<(String, usize)> {
        let next = self.redo.pop()?;
        self.undo.push((current.to_owned(), caret));
        self.typing = false;
        Some(next)
    }
}

/// Ctrl/Cmd+Z undoes, Shift+Ctrl/Cmd+Z and Ctrl+Y redo. Returns the value to
/// switch to, with the caret already placed.
pub(crate) fn history_key(
    e: &mut Edit,
    h: &mut History,
    value: &str,
    key: &Key,
    m: &Modifiers,
) -> Option<String> {
    let Key::Character(c) = key else {
        return None;
    };
    if !primary(m) {
        return None;
    }
    let step = match c.to_lowercase().as_str() {
        "z" if m.shift => h.redo(value, e.caret),
        "z" => h.undo(value, e.caret),
        "y" if !is_mac() => h.redo(value, e.caret),
        _ => return None,
    };
    // A key that is undo/redo but has nothing to do still counts as handled.
    let (v, caret) = step.unwrap_or_else(|| (value.to_owned(), e.caret));
    e.caret = caret.min(v.chars().count());
    e.anchor = e.caret;
    Some(v)
}

/// Byte offset of char index `idx` (the end when past it).
fn byte_of(value: &str, idx: usize) -> usize {
    value
        .char_indices()
        .nth(idx)
        .map_or(value.len(), |(b, _)| b)
}

/// The grapheme-cluster boundary before `idx` (0 at the start). Clusters are
/// what a user sees as one character: ZWJ emoji, flags, base plus combining
/// marks, Hangul syllables.
pub fn prev_grapheme(value: &str, idx: usize) -> usize {
    let b = byte_of(value, idx);
    match value[..b].graphemes(true).next_back() {
        Some(g) => idx - g.chars().count(),
        None => 0,
    }
}

/// The grapheme-cluster boundary after `idx` (unchanged at the end).
pub fn next_grapheme(value: &str, idx: usize) -> usize {
    let b = byte_of(value, idx);
    match value[b..].graphemes(true).next() {
        Some(g) => idx + g.chars().count(),
        None => idx,
    }
}

/// Char range `(start, end)` of the line containing char index `idx`.
pub(crate) fn line_range(value: &str, idx: usize) -> (usize, usize) {
    let b = byte_of(value, idx);
    let start = value[..b].rfind('\n').map_or(0, |i| i + 1);
    let end = value[b..].find('\n').map_or(value.len(), |i| b + i);
    (value[..start].chars().count(), value[..end].chars().count())
}

/// True when the first strong letter of the line holding `idx` is from a
/// right-to-left script (Hebrew, Arabic, Syriac, Thaana, N'Ko and friends).
fn line_is_rtl(value: &str, idx: usize) -> bool {
    let b = byte_of(value, idx);
    let start = value[..b].rfind('\n').map_or(0, |i| i + 1);
    let end = value[b..].find('\n').map_or(value.len(), |i| b + i);
    value[start..end]
        .chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| {
            matches!(c as u32,
                0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF
                | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
        })
}

/// UAX #29 word segments as `(start, end, is_word)` in char indices;
/// `is_word` is false for spaces and punctuation.
fn word_segments(value: &str) -> Vec<(usize, usize, bool)> {
    let mut at = 0;
    value
        .split_word_bounds()
        .map(|seg| {
            let n = seg.chars().count();
            let r = (at, at + n, seg.chars().any(char::is_alphanumeric));
            at += n;
            r
        })
        .collect()
}

/// Index of the start of the word before `from`. Words follow UAX #29, so
/// `don't` and `foo_bar` are one word and each CJK ideograph is its own.
pub fn word_left(value: &str, from: usize) -> usize {
    word_segments(value)
        .into_iter()
        .rev()
        .find(|&(start, _, word)| word && start < from)
        .map_or(0, |(start, _, _)| start)
}

/// Index of the end of the word after `from`.
pub fn word_right(value: &str, from: usize) -> usize {
    word_segments(value)
        .into_iter()
        .find(|&(_, end, word)| word && end > from)
        .map_or_else(|| value.chars().count(), |(_, end, _)| end)
}

/// The word, or the run of spaces or punctuation, under char index `idx`,
/// as a `(start, end)` range. Used for double-click selection.
pub fn word_bounds(value: &str, idx: usize) -> (usize, usize) {
    let segs = word_segments(value);
    segs.iter()
        .find(|&&(s, e, _)| s <= idx && idx < e)
        .or(segs.last())
        .map_or((0, 0), |&(s, e, _)| (s, e))
}

/// Char index in `s` (on a grapheme boundary) whose x position, measured
/// with `width`, is nearest `x`. Binary search over boundaries, so a click
/// costs O(log n) measurements instead of one per character.
pub(crate) fn nearest_boundary(s: &str, x: f32, width: impl Fn(&str) -> f32) -> usize {
    if x <= 0.0 {
        return 0;
    }
    // (char index, byte offset) of every boundary, the end included.
    let mut cuts = Vec::new();
    let (mut ci, mut bi) = (0, 0);
    for g in s.graphemes(true) {
        cuts.push((ci, bi));
        ci += g.chars().count();
        bi += g.len();
    }
    cuts.push((ci, bi));
    let at = |k: usize| width(&s[..cuts[k].1]);
    // Smallest boundary whose prefix is at least `x` wide.
    let (mut lo, mut hi) = (0, cuts.len() - 1);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if at(mid) >= x {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    if lo > 0 && (x - at(lo - 1)) < (at(lo) - x) {
        lo -= 1;
    }
    cuts[lo].0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_replaces_selection() {
        let mut e = Edit {
            caret: 5,
            anchor: 0,
            ..Default::default()
        };
        let v = e.insert("hello world", "bye");
        assert_eq!(v, "bye world");
        assert_eq!((e.caret, e.anchor), (3, 3));
    }
    #[test]
    fn backspace_and_delete() {
        let mut e = Edit {
            caret: 3,
            anchor: 3,
            ..Default::default()
        };
        assert_eq!(e.delete("abcdef", false).unwrap(), "abdef");
        assert_eq!(e.caret, 2);
        assert_eq!(e.delete("abdef", true).unwrap(), "abef");
        let mut start = Edit::default();
        assert!(start.delete("abc", false).is_none());
    }
    #[test]
    fn multibyte_safe() {
        let mut e = Edit {
            caret: 1,
            anchor: 1,
            ..Default::default()
        };
        assert_eq!(e.insert("é😀", "x"), "éx😀");
        assert_eq!(e.selected("éx😀"), "");
        e.select_all(3);
        assert_eq!(e.selected("éx😀"), "éx😀");
    }
    #[test]
    fn history_undo_redo_and_typing_coalesce() {
        let mut h = History::default();
        let mut e = Edit::default();
        h.record("", 0, "a", 100);
        h.record("a", 1, "ab", 200); // merged with the first
        h.record("ab", 2, "ab ", 300);
        h.record("ab ", 3, "", 5000); // a deletion is its own step
        let ctrl = Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let z = Key::Character("z".into());
        assert_eq!(
            history_key(&mut e, &mut h, "", &z, &ctrl).as_deref(),
            Some("ab ")
        );
        assert_eq!(e.caret, 3);
        assert_eq!(
            history_key(&mut e, &mut h, "ab ", &z, &ctrl).as_deref(),
            Some("")
        );
        let shift = Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        assert_eq!(
            history_key(&mut e, &mut h, "", &z, &shift).as_deref(),
            Some("ab ")
        );
        let y = Key::Character("y".into());
        assert_eq!(
            history_key(&mut e, &mut h, "ab ", &y, &ctrl).as_deref(),
            Some("")
        );
        // nothing left to redo: still "handled", value unchanged
        assert_eq!(
            history_key(&mut e, &mut h, "", &y, &ctrl).as_deref(),
            Some("")
        );
        // a new edit clears redo
        h.record("", 0, "q", 9000);
        assert!(h.redo.is_empty());
    }
    #[test]
    fn word_moves() {
        assert_eq!(word_left("foo bar", 7), 4);
        assert_eq!(word_left("foo bar", 4), 0);
        assert_eq!(word_right("foo bar", 0), 3);
        assert_eq!(word_right("foo bar", 3), 7);
    }
    #[test]
    fn graphemes_words_and_boundaries() {
        let flag = "a\u{1f1ef}\u{1f1f5}b";
        assert_eq!(next_grapheme(flag, 1), 3);
        assert_eq!(prev_grapheme(flag, 3), 1);
        assert_eq!(word_right("we don't know", 3), 8);
        assert_eq!(word_left("we don't know", 8), 3);
        assert_eq!(word_bounds("alpha beta", 7), (6, 10));
        assert_eq!(sanitize("a\rb\r\nc", true), "a\nb\nc");
        let w = |s: &str| s.chars().count() as f32 * 10.0;
        assert_eq!(nearest_boundary("abcd", 24.0, w), 2);
        assert_eq!(nearest_boundary("abcd", 99.0, w), 4);
        assert_eq!(nearest_boundary("a\u{1f1ef}\u{1f1f5}b", 15.0, w), 1);
    }
}
