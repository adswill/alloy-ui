//! Caret and selection editing, independent of drawing so it can be tested.

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
    fn word_moves() {
        assert_eq!(word_left("foo bar", 7), 4);
        assert_eq!(word_left("foo bar", 4), 0);
        assert_eq!(word_right("foo bar", 0), 3);
        assert_eq!(word_right("foo bar", 3), 7);
    }
}
