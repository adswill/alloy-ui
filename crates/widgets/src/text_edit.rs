//! Caret and selection editing, independent of drawing so it can be tested.

use crate::ui::Clipboard;
use alloy_ui_core::{Key, Modifiers};

/// Caret and selection state of one text field, in `char` indices.
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
    /// Delete the selection, or one character before (`forward == false`)
    /// or after the caret. Returns the new value, or `None` if nothing changed.
    pub fn delete(&mut self, value: &str, forward: bool) -> Option<String> {
        let len = value.chars().count();
        if !self.has_selection() {
            if forward && self.caret < len {
                self.anchor = self.caret + 1;
            } else if !forward && self.caret > 0 {
                self.anchor = self.caret - 1;
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
    let word = m.alt || m.ctrl;
    let changed = |v: Option<String>| v.map_or(Handled, Changed);
    match key {
        Key::Backspace if editable => {
            if !e.has_selection() && word {
                e.anchor = word_left(value, e.caret);
            }
            changed(e.delete(value, false))
        }
        Key::Named(n) => match n.as_str() {
            "ArrowLeft" => {
                let to = if e.has_selection() && !m.shift {
                    e.range().0
                } else if m.logo {
                    0
                } else if word {
                    word_left(value, e.caret)
                } else {
                    e.caret.saturating_sub(1)
                };
                e.move_to(to, m.shift);
                Handled
            }
            "ArrowRight" => {
                let to = if e.has_selection() && !m.shift {
                    e.range().1
                } else if m.logo {
                    len
                } else if word {
                    word_right(value, e.caret)
                } else {
                    (e.caret + 1).min(len)
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
        Key::Character(c) if m.ctrl || m.logo => match c.to_lowercase().as_str() {
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

/// Strip control characters (keeping newlines when `multiline`).
pub(crate) fn sanitize(text: &str, multiline: bool) -> String {
    text.chars()
        .filter(|c| !c.is_control() || (multiline && *c == '\n'))
        .collect()
}

/// Split `value` into lines (a trailing newline yields a final empty line).
pub fn split_lines(value: &str) -> Vec<String> {
    value.split('\n').map(str::to_owned).collect()
}

/// The `(line, column)` of character index `idx`.
pub fn line_col(lines: &[String], idx: usize) -> (usize, usize) {
    let mut start = 0;
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count();
        if idx <= start + n {
            return (i, idx - start);
        }
        start += n + 1;
    }
    let last = lines.len().saturating_sub(1);
    (last, lines.get(last).map_or(0, |l| l.chars().count()))
}

/// The character index of `(line, col)`, clamping both into range.
pub fn index_of(lines: &[String], line: usize, col: usize) -> usize {
    let line = line.min(lines.len().saturating_sub(1));
    let start: usize = lines[..line].iter().map(|l| l.chars().count() + 1).sum();
    start + col.min(lines[line].chars().count())
}

/// Index of the start of the word before `from`.
pub fn word_left(value: &str, from: usize) -> usize {
    let c: Vec<char> = value.chars().collect();
    let mut i = from.min(c.len());
    while i > 0 && !c[i - 1].is_alphanumeric() {
        i -= 1;
    }
    while i > 0 && c[i - 1].is_alphanumeric() {
        i -= 1;
    }
    i
}

/// Index of the end of the word after `from`.
pub fn word_right(value: &str, from: usize) -> usize {
    let c: Vec<char> = value.chars().collect();
    let mut i = from.min(c.len());
    while i < c.len() && !c[i].is_alphanumeric() {
        i += 1;
    }
    while i < c.len() && c[i].is_alphanumeric() {
        i += 1;
    }
    i
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
    fn line_math_round_trips() {
        let lines = split_lines("ab\n\ncde");
        assert_eq!(lines, ["ab", "", "cde"]);
        assert_eq!(line_col(&lines, 0), (0, 0));
        assert_eq!(line_col(&lines, 2), (0, 2));
        assert_eq!(line_col(&lines, 3), (1, 0));
        assert_eq!(line_col(&lines, 6), (2, 2));
        assert_eq!(line_col(&lines, 99), (2, 3));
        for i in 0..=7 {
            let (l, c) = line_col(&lines, i);
            assert_eq!(index_of(&lines, l, c), i);
        }
        assert_eq!(index_of(&lines, 0, 50), 2, "column clamps to line length");
        assert_eq!(index_of(&lines, 9, 0), 4, "line clamps to the last");
    }
    #[test]
    fn word_moves() {
        assert_eq!(word_left("foo bar", 7), 4);
        assert_eq!(word_left("foo bar", 4), 0);
        assert_eq!(word_right("foo bar", 0), 3);
        assert_eq!(word_right("foo bar", 3), 7);
    }
}
