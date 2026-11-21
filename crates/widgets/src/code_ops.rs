//! Pure text operations behind the code editor's multi-cursor and bracket matching. All
//! positions are `char` indices except in [`match_bracket`], which works in bytes so it can
//! scan a window of a huge file without copying it.

use crate::text_edit::{next_grapheme, prev_grapheme, word_bounds};

/// One selection: `anchor` is where it started, `caret` where it ends (and blinks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sel {
    pub anchor: usize,
    pub caret: usize,
}

impl Sel {
    pub fn at(i: usize) -> Self {
        Self {
            anchor: i,
            caret: i,
        }
    }
    pub fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.caret), self.anchor.max(self.caret))
    }
    pub fn is_empty(&self) -> bool {
        self.anchor == self.caret
    }
}

/// Sort by position and merge selections that overlap or touch. Returns where the selection
/// that held `keep` (an index into the input) ended up.
pub(crate) fn normalize(v: &mut Vec<Sel>, keep: usize) -> usize {
    let mut tagged: Vec<(Sel, bool)> = v.iter().enumerate().map(|(i, s)| (*s, i == keep)).collect();
    tagged.sort_by_key(|(s, _)| (s.range().0, s.range().1));
    let mut out: Vec<(Sel, bool)> = Vec::with_capacity(tagged.len());
    for (s, k) in tagged {
        if let Some((last, lk)) = out.last_mut() {
            let (la, lz) = last.range();
            let (a, z) = s.range();
            // Overlapping, or two carets in the same place. Selections that merely touch
            // end to end stay separate (typing between them is a real case).
            if a < lz || (a == lz && (s.is_empty() || last.is_empty())) {
                let forward = last.caret >= last.anchor;
                let (na, nz) = (la.min(a), lz.max(z));
                *last = if forward {
                    Sel {
                        anchor: na,
                        caret: nz,
                    }
                } else {
                    Sel {
                        anchor: nz,
                        caret: na,
                    }
                };
                *lk |= k;
                continue;
            }
        }
        out.push((s, k));
    }
    let at = out
        .iter()
        .position(|x| x.1)
        .unwrap_or(out.len().saturating_sub(1));
    *v = out.into_iter().map(|x| x.0).collect();
    at
}

fn byte_of(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |x| x.0)
}

/// Char index to byte offset. With many selections a lookup table makes each one O(1)
/// instead of a scan from the start of the text.
struct Idx<'a> {
    text: &'a str,
    map: Option<Vec<u32>>,
}

impl<'a> Idx<'a> {
    fn new(text: &'a str, selections: usize) -> Self {
        let map = (selections > 16 && text.len() < u32::MAX as usize).then(|| {
            let mut m: Vec<u32> = text.char_indices().map(|(b, _)| b as u32).collect();
            m.push(text.len() as u32);
            m
        });
        Self { text, map }
    }
    fn byte(&self, c: usize) -> usize {
        match &self.map {
            Some(m) => m.get(c).map_or(self.text.len(), |b| *b as usize),
            None => byte_of(self.text, c),
        }
    }
    /// The grapheme boundary before `c`, looking at a small window only.
    fn prev(&self, c: usize) -> usize {
        let from = c.saturating_sub(24);
        let w = &self.text[self.byte(from)..self.byte(c)];
        from + prev_grapheme(w, c - from)
    }
    /// The grapheme boundary after `c`, looking at a small window only.
    fn next(&self, c: usize) -> usize {
        let w = &self.text[self.byte(c)..self.byte(c + 24)];
        c + next_grapheme(w, 0)
    }
}

/// Replace every selection with `with(index, selected_text)`, leaving a caret after each
/// insertion. `sels` must be normalized.
pub(crate) fn replace_all(
    text: &str,
    sels: &[Sel],
    with: &dyn Fn(usize, &str) -> String,
) -> (String, Vec<Sel>) {
    let idx = Idx::new(text, sels.len());
    let mut out = String::with_capacity(text.len() + sels.len() * 4);
    let mut carets = Vec::with_capacity(sels.len());
    let mut prev_byte = 0usize;
    let mut out_chars = 0usize;
    let mut prev_char = 0usize;
    for (i, s) in sels.iter().enumerate() {
        let (a, z) = s.range();
        let (ba, bz) = (idx.byte(a), idx.byte(z));
        let gap = &text[prev_byte..ba];
        out.push_str(gap);
        out_chars += a - prev_char;
        let ins = with(i, &text[ba..bz]);
        out.push_str(&ins);
        out_chars += ins.chars().count();
        carets.push(Sel::at(out_chars));
        prev_byte = bz;
        prev_char = z;
    }
    out.push_str(&text[prev_byte..]);
    (out, carets)
}

/// Type `s` at every selection.
pub(crate) fn insert(text: &str, sels: &[Sel], s: &str) -> (String, Vec<Sel>) {
    replace_all(text, sels, &|_, _| s.to_string())
}

/// Backspace (`forward == false`) or Delete at every caret; selections are deleted whole.
pub(crate) fn delete(text: &str, sels: &[Sel], forward: bool) -> (String, Vec<Sel>) {
    let idx = Idx::new(text, sels.len());
    let mut ext: Vec<Sel> = sels
        .iter()
        .map(|s| {
            if !s.is_empty() {
                *s
            } else if forward {
                Sel {
                    anchor: s.caret,
                    caret: idx.next(s.caret),
                }
            } else {
                Sel {
                    anchor: idx.prev(s.caret),
                    caret: s.caret,
                }
            }
        })
        .collect();
    normalize(&mut ext, 0);
    replace_all(text, &ext, &|_, _| String::new())
}

/// Char index of the start of each line.
fn line_starts(text: &str) -> Vec<usize> {
    let mut v = vec![0];
    let mut c = 0;
    for ch in text.chars() {
        c += 1;
        if ch == '\n' {
            v.push(c);
        }
    }
    v
}

fn line_of(starts: &[usize], idx: usize) -> usize {
    starts.partition_point(|&s| s <= idx).saturating_sub(1)
}

/// Line break at every caret, keeping that line's indent (up to the caret).
pub(crate) fn newline(text: &str, sels: &[Sel]) -> (String, Vec<Sel>) {
    let starts = line_starts(text);
    let idx = Idx::new(text, sels.len());
    replace_all(text, sels, &|i, _| {
        let at = sels[i].range().0;
        let ls = starts[line_of(&starts, at)];
        let lead: String = text[idx.byte(ls)..idx.byte(at)]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        format!("\n{lead}")
    })
}

/// Which way a movement goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// Move every selection. `extend` keeps the anchors (Shift held).
pub(crate) fn step(text: &str, sels: &[Sel], dir: Dir, extend: bool) -> Vec<Sel> {
    let starts = line_starts(text);
    let total = text.chars().count();
    let idx = Idx::new(text, sels.len());
    let line_end = |l: usize| {
        if l + 1 < starts.len() {
            starts[l + 1] - 1
        } else {
            total
        }
    };
    let mut out: Vec<Sel> = sels
        .iter()
        .map(|s| {
            let (a, z) = s.range();
            let caret = match dir {
                Dir::Left if !extend && !s.is_empty() => a,
                Dir::Right if !extend && !s.is_empty() => z,
                Dir::Left => idx.prev(s.caret),
                Dir::Right => idx.next(s.caret),
                Dir::Home => starts[line_of(&starts, s.caret)],
                Dir::End => line_end(line_of(&starts, s.caret)),
                Dir::Up | Dir::Down => {
                    let l = line_of(&starts, s.caret);
                    let col = s.caret - starts[l];
                    let to = if dir == Dir::Up {
                        l.checked_sub(1)
                    } else {
                        (l + 1 < starts.len()).then_some(l + 1)
                    };
                    match to {
                        Some(t) => starts[t] + col.min(line_end(t) - starts[t]),
                        None if dir == Dir::Up => 0,
                        None => total,
                    }
                }
            };
            Sel {
                anchor: if extend { s.anchor } else { caret },
                caret,
            }
        })
        .collect();
    normalize(&mut out, 0);
    out
}

/// Add a caret on the line above or below the first or last selection, at its column.
pub(crate) fn add_caret_vertical(text: &str, sels: &[Sel], down: bool) -> Vec<Sel> {
    let starts = line_starts(text);
    let total = text.chars().count();
    let from = if down {
        sels.iter().max_by_key(|s| s.caret)
    } else {
        sels.iter().min_by_key(|s| s.caret)
    };
    let mut out = sels.to_vec();
    if let Some(s) = from {
        let l = line_of(&starts, s.caret);
        let col = s.caret - starts[l];
        let to = if down {
            (l + 1 < starts.len()).then_some(l + 1)
        } else {
            l.checked_sub(1)
        };
        if let Some(t) = to {
            let end = if t + 1 < starts.len() {
                starts[t + 1] - 1
            } else {
                total
            };
            out.push(Sel::at(starts[t] + col.min(end - starts[t])));
        }
    }
    let keep = out.len() - 1;
    normalize(&mut out, keep);
    out
}

/// Select the word under the primary caret, or add the next occurrence of the selected text
/// (wrapping around) as a new selection. Returns the new set and whether anything changed.
pub(crate) fn add_next_occurrence(text: &str, sels: &[Sel], primary: usize) -> (Vec<Sel>, bool) {
    let mut out = sels.to_vec();
    let p = out[primary.min(out.len() - 1)];
    if p.is_empty() {
        let (a, z) = word_bounds(text, p.caret);
        if a == z {
            return (out, false);
        }
        out[primary.min(sels.len() - 1)] = Sel {
            anchor: a,
            caret: z,
        };
        return (out, true);
    }
    let (a, z) = p.range();
    let needle: String = text.chars().skip(a).take(z - a).collect();
    if needle.is_empty() {
        return (out, false);
    }
    let hay_from = byte_of(text, z);
    let found = text[hay_from..]
        .find(&needle)
        .map(|b| hay_from + b)
        .or_else(|| text[..byte_of(text, a)].find(&needle));
    let Some(b) = found else {
        return (out, false);
    };
    let start = text[..b].chars().count();
    let new = Sel {
        anchor: start,
        caret: start + needle.chars().count(),
    };
    if out.iter().any(|s| s.range() == new.range()) {
        return (out, false);
    }
    out.push(new);
    (out, true)
}

/// Every occurrence of the selected text (or of the word under the caret).
pub(crate) fn select_all_occurrences(text: &str, sels: &[Sel], primary: usize) -> Vec<Sel> {
    let p = sels[primary.min(sels.len() - 1)];
    let (a, z) = if p.is_empty() {
        word_bounds(text, p.caret)
    } else {
        p.range()
    };
    if a == z {
        return sels.to_vec();
    }
    let needle: String = text.chars().skip(a).take(z - a).collect();
    let n = needle.chars().count();
    let mut out = Vec::new();
    // Count chars incrementally between matches so this is linear in the text.
    let (mut at_byte, mut at_char) = (0usize, 0usize);
    let mut from = 0;
    while let Some(b) = text[from..].find(&needle) {
        let at = from + b;
        at_char += text[at_byte..at].chars().count();
        at_byte = at;
        out.push(Sel {
            anchor: at_char,
            caret: at_char + n,
        });
        from = at + needle.len().max(1);
    }
    out
}

/// The bracket matching the one at `at` (byte offset of the bracket), if any. Looks at most
/// 64 KiB each way and counts nesting; it does not know about strings and comments, like most
/// editors' quick match.
pub(crate) fn match_bracket(text: &str, at: usize) -> Option<usize> {
    const LIMIT: usize = 64 * 1024;
    let c = text[at..].chars().next()?;
    let (open, close, fwd) = match c {
        '(' => ('(', ')', true),
        '[' => ('[', ']', true),
        '{' => ('{', '}', true),
        ')' => ('(', ')', false),
        ']' => ('[', ']', false),
        '}' => ('{', '}', false),
        _ => return None,
    };
    let mut depth = 0i32;
    if fwd {
        for (i, ch) in text[at..].char_indices().take(LIMIT) {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Some(at + i);
                }
            }
        }
    } else {
        let upto = at + c.len_utf8();
        for (i, ch) in text[..upto].char_indices().rev().take(LIMIT) {
            if ch == close {
                depth += 1;
            } else if ch == open {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// The bracket pair to highlight for a caret at byte offset `caret`: the bracket just before
/// it, else the one just after, with its match. `Err` holds an unmatched bracket's offset.
pub(crate) fn bracket_pair(text: &str, caret: usize) -> Option<Result<(usize, usize), usize>> {
    let before = text[..caret]
        .chars()
        .next_back()
        .map(|c| caret - c.len_utf8());
    let candidates = [before, Some(caret)];
    for at in candidates.into_iter().flatten() {
        if at >= text.len() {
            continue;
        }
        let Some(c) = text[at..].chars().next() else {
            continue;
        };
        if "()[]{}".contains(c) {
            return Some(match match_bracket(text, at) {
                Some(m) => Ok((at, m)),
                None => Err(at),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carets(v: &[usize]) -> Vec<Sel> {
        v.iter().map(|&i| Sel::at(i)).collect()
    }

    #[test]
    fn typing_at_three_carets_inserts_three_times_and_keeps_them_aligned() {
        let (t, c) = insert("a\nb\nc", &carets(&[1, 3, 5]), "!");
        assert_eq!(t, "a!\nb!\nc!");
        assert_eq!(c.iter().map(|s| s.caret).collect::<Vec<_>>(), [2, 5, 8]);
    }

    #[test]
    fn backspace_deletes_at_each_caret_and_merges_collisions() {
        let (t, c) = delete("ab\ncd", &carets(&[2, 5]), false);
        assert_eq!(t, "a\nc");
        assert_eq!(c.iter().map(|s| s.caret).collect::<Vec<_>>(), [1, 3]);
        // Two carets one apart both delete the same neighbour once.
        let (t, _) = delete("abc", &carets(&[2, 3]), false);
        assert_eq!(t, "a");
    }

    #[test]
    fn selections_are_replaced_and_newline_keeps_indent() {
        let sels = vec![
            Sel {
                anchor: 0,
                caret: 3,
            },
            Sel {
                anchor: 4,
                caret: 7,
            },
        ];
        let (t, _) = insert("foo bar baz", &sels, "X");
        assert_eq!(t, "X X baz");
        let (t, c) = newline("  a b", &carets(&[3]));
        assert_eq!(t, "  a\n   b");
        assert_eq!(c[0].caret, 6);
    }

    #[test]
    fn vertical_moves_keep_the_column_and_clamp_short_lines() {
        let text = "abcdef\nab\nabcdef";
        let down = step(text, &carets(&[5]), Dir::Down, false);
        assert_eq!(down[0].caret, 9, "clamped to the end of the short line");
        let up = step(text, &carets(&[5, 12]), Dir::Up, false);
        assert_eq!(up.iter().map(|s| s.caret).collect::<Vec<_>>(), [0, 9]);
    }

    #[test]
    fn add_caret_below_and_next_occurrence() {
        let t = "let x = 1;\nlet y = 2;\nlet z = 3;";
        let c = add_caret_vertical(t, &carets(&[4]), true);
        assert_eq!(c.iter().map(|s| s.caret).collect::<Vec<_>>(), [4, 15]);
        // Word under caret first, then the next occurrence, wrapping.
        let t = "foo bar foo baz foo";
        let (c, ch) = add_next_occurrence(t, &carets(&[1]), 0);
        assert!(ch);
        assert_eq!(c[0].range(), (0, 3));
        let (c, _) = add_next_occurrence(t, &c, 0);
        assert_eq!(
            c.iter().map(|s| s.range()).collect::<Vec<_>>(),
            [(0, 3), (8, 11)]
        );
        let all = select_all_occurrences(t, &carets(&[1]), 0);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn brackets_match_across_nesting_and_report_unmatched() {
        let t = "fn f(a: (u8, u8)) { x[0] }";
        let open = t.find('(').unwrap();
        assert_eq!(match_bracket(t, open), Some(t.find("))").unwrap() + 1));
        let brace = t.find('{').unwrap();
        assert_eq!(match_bracket(t, brace), Some(t.rfind('}').unwrap()));
        assert_eq!(match_bracket(t, t.rfind('}').unwrap()), Some(brace));
        // Caret right after the closing paren finds the pair.
        let close = t.find("))").unwrap() + 1;
        assert_eq!(bracket_pair(t, close + 1), Some(Ok((close, open))));
        assert_eq!(bracket_pair("(()", 0), Some(Err(0)));
        assert_eq!(bracket_pair("abc", 1), None);
    }

    #[test]
    fn normalize_merges_overlaps_but_not_touching_selections() {
        let mut v = vec![
            Sel {
                anchor: 0,
                caret: 3,
            },
            Sel {
                anchor: 2,
                caret: 5,
            },
            Sel {
                anchor: 5,
                caret: 7,
            },
            Sel::at(7),
        ];
        normalize(&mut v, 0);
        assert_eq!(
            v.iter().map(|s| s.range()).collect::<Vec<_>>(),
            [(0, 5), (5, 7)]
        );
    }
}
