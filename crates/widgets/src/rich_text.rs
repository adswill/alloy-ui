//! The document model behind [`RichEditor`](crate::RichEditor): blocks of styled runs.
//!
//! Positions are *flat character offsets*: the characters of every block in order with one
//! `\n` between blocks, exactly like a plain string. That lets selection, word and
//! grapheme movement reuse the plain-text helpers, and keeps every edit a single
//! `replace(range, text)`.

use std::hash::{Hash, Hasher};

/// Inline formatting. Italic is drawn as a medium weight until the renderer has an italic face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Marks {
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Underline.
    pub underline: bool,
    /// Inline code (monospace on a tinted chip).
    pub code: bool,
}

/// One inline mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Bold.
    Bold,
    /// Italic.
    Italic,
    /// Underline.
    Underline,
    /// Inline code.
    Code,
}

impl Marks {
    /// Is `m` set?
    pub fn has(&self, m: Mark) -> bool {
        match m {
            Mark::Bold => self.bold,
            Mark::Italic => self.italic,
            Mark::Underline => self.underline,
            Mark::Code => self.code,
        }
    }
    /// Set or clear `m`.
    pub fn set(&mut self, m: Mark, on: bool) {
        match m {
            Mark::Bold => self.bold = on,
            Mark::Italic => self.italic = on,
            Mark::Underline => self.underline = on,
            Mark::Code => self.code = on,
        }
    }
}

/// A stretch of text with one set of marks and an optional link target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Run {
    /// The text (no newlines).
    pub text: String,
    /// Formatting.
    pub marks: Marks,
    /// Link target, if this is a link.
    pub link: Option<String>,
}

impl Run {
    /// Plain text.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
    fn len(&self) -> usize {
        self.text.chars().count()
    }
    fn same_style(&self, o: &Run) -> bool {
        self.marks == o.marks && self.link == o.link
    }
}

/// What a block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlockKind {
    /// A paragraph.
    #[default]
    Paragraph,
    /// A heading, level 1 to 3.
    Heading(u8),
    /// A bullet list item.
    Bullet,
    /// A numbered list item (numbers count consecutive items).
    Numbered,
    /// A block quote.
    Quote,
}

/// A paragraph, heading, list item or quote.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Block {
    /// What it is.
    pub kind: BlockKind,
    /// Its text.
    pub runs: Vec<Run>,
}

impl Block {
    /// Number of characters.
    pub fn len(&self) -> usize {
        self.runs.iter().map(Run::len).sum()
    }
    /// No characters.
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|r| r.text.is_empty())
    }
    /// The block's text.
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// A rich-text document. Cheap to clone for the sizes people type by hand; the editor hands
/// you a new one on every edit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RichText {
    /// The blocks, top to bottom. Never empty.
    pub blocks: Vec<Block>,
}

impl Default for RichText {
    fn default() -> Self {
        Self {
            blocks: vec![Block::default()],
        }
    }
}

/// Where a flat offset sits: block index and character offset in that block.
type At = (usize, usize);

fn split_chars(s: &str, n: usize) -> (&str, &str) {
    let i = s.char_indices().nth(n).map_or(s.len(), |x| x.0);
    s.split_at(i)
}

impl RichText {
    /// An empty document (one empty paragraph).
    pub fn new() -> Self {
        Self::default()
    }
    /// Parse a small Markdown subset: `#`..`###` headings, `-`/`*` bullets, `1.` numbered
    /// items, `>` quotes, `**bold**`, `*italic*`, `` `code` `` and `[text](url)`.
    pub fn from_markdown(src: &str) -> Self {
        let mut blocks = Vec::new();
        for line in src.lines() {
            let t = line.trim_end();
            if t.is_empty() {
                continue;
            }
            let (kind, rest) = if let Some(r) = t.strip_prefix("### ") {
                (BlockKind::Heading(3), r)
            } else if let Some(r) = t.strip_prefix("## ") {
                (BlockKind::Heading(2), r)
            } else if let Some(r) = t.strip_prefix("# ") {
                (BlockKind::Heading(1), r)
            } else if let Some(r) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
                (BlockKind::Bullet, r)
            } else if let Some(r) = t.strip_prefix("> ") {
                (BlockKind::Quote, r)
            } else if let Some((n, r)) = t.split_once(". ") {
                if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
                    (BlockKind::Numbered, r)
                } else {
                    (BlockKind::Paragraph, t)
                }
            } else {
                (BlockKind::Paragraph, t)
            };
            blocks.push(Block {
                kind,
                runs: parse_inline(rest),
            });
        }
        if blocks.is_empty() {
            return Self::default();
        }
        Self { blocks }
    }
    /// The document as Markdown. Underline has no Markdown form and is dropped.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let mut n = 0;
        let mut prev: Option<BlockKind> = None;
        for b in &self.blocks {
            n = if b.kind == BlockKind::Numbered {
                n + 1
            } else {
                0
            };
            if let Some(p) = prev {
                // Consecutive items of one list stay on adjacent lines.
                let tight = p == b.kind && matches!(p, BlockKind::Bullet | BlockKind::Numbered);
                out.push_str(if tight { "\n" } else { "\n\n" });
            }
            prev = Some(b.kind);
            out.push_str(&match b.kind {
                BlockKind::Paragraph => String::new(),
                BlockKind::Heading(l) => format!("{} ", "#".repeat(l.clamp(1, 3) as usize)),
                BlockKind::Bullet => "- ".into(),
                BlockKind::Numbered => format!("{n}. "),
                BlockKind::Quote => "> ".into(),
            });
            for r in &b.runs {
                let mut t = r.text.clone();
                if r.marks.code {
                    t = format!("`{t}`");
                }
                if r.marks.bold {
                    t = format!("**{t}**");
                }
                if r.marks.italic {
                    t = format!("*{t}*");
                }
                if let Some(u) = &r.link {
                    t = format!("[{t}]({u})");
                }
                out.push_str(&t);
            }
        }
        out
    }
    /// The text with blocks separated by `\n`.
    pub fn plain(&self) -> String {
        self.blocks
            .iter()
            .map(Block::text)
            .collect::<Vec<_>>()
            .join("\n")
    }
    /// Total flat length in characters.
    pub fn len(&self) -> usize {
        self.blocks.iter().map(Block::len).sum::<usize>() + self.blocks.len() - 1
    }
    /// No text at all.
    pub fn is_empty(&self) -> bool {
        self.blocks.len() == 1 && self.blocks[0].is_empty()
    }
    /// Number of words.
    pub fn words(&self) -> usize {
        self.plain().split_whitespace().count()
    }
    /// A hash of the content, for caches.
    pub fn fingerprint(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }

    /// Block and offset of flat position `p` (clamped).
    pub fn at(&self, p: usize) -> At {
        let mut rest = p;
        for (i, b) in self.blocks.iter().enumerate() {
            let n = b.len();
            if rest <= n {
                return (i, rest);
            }
            rest -= n + 1;
        }
        let last = self.blocks.len() - 1;
        (last, self.blocks[last].len())
    }
    /// Flat position of the start of block `i`.
    pub fn block_start(&self, i: usize) -> usize {
        self.blocks[..i].iter().map(|b| b.len() + 1).sum()
    }

    /// Split `runs` at character `n` into the part before and after.
    fn split_runs(runs: &[Run], n: usize) -> (Vec<Run>, Vec<Run>) {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        let mut at = 0;
        for r in runs {
            let len = r.len();
            if at + len <= n {
                left.push(r.clone());
            } else if at >= n {
                right.push(r.clone());
            } else {
                let (a, z) = split_chars(&r.text, n - at);
                left.push(Run {
                    text: a.into(),
                    ..r.clone()
                });
                right.push(Run {
                    text: z.into(),
                    ..r.clone()
                });
            }
            at += len;
        }
        (left, right)
    }
    /// Merge neighbours with the same style and drop empty runs.
    fn normalize(runs: &mut Vec<Run>) {
        let mut out: Vec<Run> = Vec::with_capacity(runs.len());
        for r in runs.drain(..) {
            if r.text.is_empty() {
                continue;
            }
            match out.last_mut() {
                Some(l) if l.same_style(&r) => l.text.push_str(&r.text),
                _ => out.push(r),
            }
        }
        *runs = out;
    }

    /// The marks and link a character typed at `p` inherits: those of the character before it
    /// (or after it at the start of a block).
    pub fn style_at(&self, p: usize) -> (Marks, Option<String>) {
        let (bi, off) = self.at(p);
        let b = &self.blocks[bi];
        let mut at = 0;
        let mut first = None;
        for r in &b.runs {
            let len = r.len();
            first.get_or_insert((r.marks, r.link.clone()));
            if off > at && off <= at + len {
                // A link does not grow when typing at its end.
                let link = if off == at + len {
                    None
                } else {
                    r.link.clone()
                };
                return (r.marks, link);
            }
            at += len;
        }
        if off == 0 {
            if let Some((m, _)) = first {
                return (m, None);
            }
        }
        (Marks::default(), None)
    }

    /// Replace flat range `a..z` with `text` (which may hold `\n`), typed with `marks` and
    /// `link`. New blocks split off a heading become paragraphs; list and quote blocks continue.
    pub fn replace(&mut self, a: usize, z: usize, text: &str, marks: Marks, link: Option<&str>) {
        let (a, z) = (a.min(z), a.max(z).min(self.len()));
        let ((ba, oa), (bz, oz)) = (self.at(a), self.at(z));
        // Delete the range, joining the end block's tail onto the start block.
        let head = Self::split_runs(&self.blocks[ba].runs, oa).0;
        let tail = Self::split_runs(&self.blocks[bz].runs, oz).1;
        let mut runs = head;
        runs.extend(tail);
        self.blocks.drain(ba + 1..=bz);
        self.blocks[ba].runs = runs;
        let mut off = oa;
        let mut bi = ba;
        // Insert, splitting on newlines.
        for (i, seg) in text.split('\n').enumerate() {
            if i > 0 {
                let (l, r) = Self::split_runs(&self.blocks[bi].runs, off);
                self.blocks[bi].runs = l;
                let kind = match self.blocks[bi].kind {
                    BlockKind::Heading(_) => BlockKind::Paragraph,
                    k => k,
                };
                self.blocks.insert(bi + 1, Block { kind, runs: r });
                bi += 1;
                off = 0;
            }
            if !seg.is_empty() {
                let (l, r) = Self::split_runs(&self.blocks[bi].runs, off);
                let mut runs = l;
                runs.push(Run {
                    text: seg.into(),
                    marks,
                    link: link.map(String::from),
                });
                runs.extend(r);
                self.blocks[bi].runs = runs;
                off += seg.chars().count();
            }
        }
        for b in &mut self.blocks {
            Self::normalize(&mut b.runs);
        }
    }

    /// The flat range as `(block, from, to)` pieces.
    fn pieces(&self, a: usize, z: usize) -> Vec<(usize, usize, usize)> {
        let ((ba, oa), (bz, oz)) = (self.at(a.min(z)), self.at(a.max(z)));
        (ba..=bz)
            .map(|b| {
                let from = if b == ba { oa } else { 0 };
                let to = if b == bz { oz } else { self.blocks[b].len() };
                (b, from, to)
            })
            .collect()
    }
    /// Apply `f` to the style of every character in `a..z`.
    fn restyle(&mut self, a: usize, z: usize, f: &dyn Fn(&mut Run)) {
        for (b, from, to) in self.pieces(a, z) {
            if from == to {
                continue;
            }
            let (l, rest) = Self::split_runs(&self.blocks[b].runs, from);
            let (mut mid, r) = Self::split_runs(&rest, to - from);
            mid.iter_mut().for_each(f);
            let mut runs = l;
            runs.extend(mid);
            runs.extend(r);
            Self::normalize(&mut runs);
            self.blocks[b].runs = runs;
        }
    }
    /// Does every character of `a..z` carry `m`? (False for an empty range.)
    pub fn all_marked(&self, a: usize, z: usize, m: Mark) -> bool {
        let mut any = false;
        for (b, from, to) in self.pieces(a, z) {
            let mut at = 0;
            for r in &self.blocks[b].runs {
                let len = r.len();
                if at < to && at + len > from {
                    any = true;
                    if !r.marks.has(m) {
                        return false;
                    }
                }
                at += len;
            }
        }
        any
    }
    /// Turn `m` on for `a..z`, or off if everything in it already has it.
    pub fn toggle_mark(&mut self, a: usize, z: usize, m: Mark) {
        let on = !self.all_marked(a, z, m);
        self.restyle(a, z, &|r| r.marks.set(m, on));
    }
    /// Make `a..z` a link to `url`, or remove links with `None`.
    pub fn set_link(&mut self, a: usize, z: usize, url: Option<&str>) {
        let url = url.map(String::from);
        self.restyle(a, z, &|r| r.link = url.clone());
    }
    /// The link at flat position `p`, if the character before or after it is linked.
    pub fn link_at(&self, p: usize) -> Option<(usize, usize, String)> {
        let (bi, off) = self.at(p);
        let base = self.block_start(bi);
        let mut at = 0;
        for r in &self.blocks[bi].runs {
            let len = r.len();
            if let Some(u) = &r.link {
                if off >= at && off <= at + len && p >= base + at && p <= base + at + len {
                    return Some((base + at, base + at + len, u.clone()));
                }
            }
            at += len;
        }
        None
    }
    /// The kind shared by every block touched by `a..z`, if they agree.
    pub fn kind_of(&self, a: usize, z: usize) -> Option<BlockKind> {
        let ps = self.pieces(a, z);
        let k = self.blocks[ps[0].0].kind;
        ps.iter().all(|p| self.blocks[p.0].kind == k).then_some(k)
    }
    /// Give every block touched by `a..z` kind `k`, or back to a paragraph if they all have it.
    pub fn toggle_kind(&mut self, a: usize, z: usize, k: BlockKind) {
        let to = if self.kind_of(a, z) == Some(k) {
            BlockKind::Paragraph
        } else {
            k
        };
        self.set_kind(a, z, to);
    }
    /// Give every block touched by `a..z` kind `k`.
    pub fn set_kind(&mut self, a: usize, z: usize, k: BlockKind) {
        for (b, ..) in self.pieces(a, z) {
            self.blocks[b].kind = k;
        }
    }
    /// The text of `a..z`, blocks joined with newlines.
    pub fn slice(&self, a: usize, z: usize) -> String {
        let p = self.plain();
        let (a, z) = (a.min(z), a.max(z));
        let s = p.char_indices().nth(a).map_or(p.len(), |x| x.0);
        let e = p.char_indices().nth(z).map_or(p.len(), |x| x.0);
        p[s..e].to_string()
    }
}

/// Inline Markdown: `**bold**`, `*italic*`, `` `code` ``, `[text](url)`.
fn parse_inline(s: &str) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut marks = Marks::default();
    let link: Option<String> = None;
    let mut buf = String::new();
    let flush = |buf: &mut String, runs: &mut Vec<Run>, marks: Marks, link: &Option<String>| {
        if !buf.is_empty() {
            runs.push(Run {
                text: std::mem::take(buf),
                marks,
                link: link.clone(),
            });
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let rest: String = chars[i..].iter().collect();
        if c == '\\' && i + 1 < chars.len() {
            buf.push(chars[i + 1]);
            i += 2;
        } else if rest.starts_with("**") && (marks.bold || rest[2..].contains("**")) {
            flush(&mut buf, &mut runs, marks, &link);
            marks.bold = !marks.bold;
            i += 2;
        } else if c == '*' && (marks.italic || rest[1..].contains('*')) {
            flush(&mut buf, &mut runs, marks, &link);
            marks.italic = !marks.italic;
            i += 1;
        } else if c == '`' && (marks.code || rest[1..].contains('`')) {
            flush(&mut buf, &mut runs, marks, &link);
            marks.code = !marks.code;
            i += 1;
        } else if c == '[' && !marks.code && link.is_none() {
            // [text](url)
            if let Some(close) = rest.find("](") {
                if let Some(end) = rest[close..].find(')') {
                    let label = &rest[1..close];
                    let url = &rest[close + 2..close + end];
                    flush(&mut buf, &mut runs, marks, &link);
                    let mut inner = parse_inline(label);
                    for r in &mut inner {
                        r.link = Some(url.to_string());
                    }
                    runs.extend(inner);
                    i += rest[..close + end + 1].chars().count();
                    continue;
                }
            }
            buf.push(c);
            i += 1;
        } else {
            buf.push(c);
            i += 1;
        }
    }
    flush(&mut buf, &mut runs, marks, &link);
    RichText::normalize(&mut runs);
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(md: &str) -> RichText {
        RichText::from_markdown(md)
    }

    #[test]
    fn markdown_round_trips() {
        let src = "# Title\n\nHello **bold** and *it* with `code` and [a link](https://x.io).\n\n- one\n- two\n\n1. first\n2. second\n\n> quoted";
        let d = doc(src);
        assert_eq!(d.blocks[0].kind, BlockKind::Heading(1));
        assert_eq!(d.blocks[1].runs.len(), 9);
        assert!(d.blocks[1].runs[1].marks.bold && d.blocks[1].runs[3].marks.italic);
        assert_eq!(d.blocks[1].runs[5].link.as_deref(), None);
        assert_eq!(d.to_markdown(), src);
        assert_eq!(doc(&d.to_markdown()), d);
    }
    #[test]
    fn typing_inserts_and_enter_splits_with_kind() {
        let mut d = doc("- item");
        d.replace(6, 6, "\nnext", Marks::default(), None);
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.blocks[1].kind, BlockKind::Bullet, "lists continue");
        let mut h = doc("# Head");
        h.replace(6, 6, "\nbody", Marks::default(), None);
        assert_eq!(h.blocks[1].kind, BlockKind::Paragraph, "headings end");
        assert_eq!(h.plain(), "Head\nbody");
    }
    #[test]
    fn deleting_across_blocks_joins_them() {
        let mut d = doc("one\n\ntwo\n\nthree");
        assert_eq!(d.plain(), "one\ntwo\nthree");
        d.replace(2, 6, "", Marks::default(), None);
        assert_eq!(d.plain(), "ono\nthree");
        assert_eq!(d.blocks.len(), 2);
        d.replace(0, d.len(), "", Marks::default(), None);
        assert!(d.is_empty());
    }
    #[test]
    fn marks_toggle_over_ranges_and_merge_back() {
        let mut d = doc("hello world");
        d.toggle_mark(0, 5, Mark::Bold);
        assert!(d.all_marked(0, 5, Mark::Bold) && !d.all_marked(0, 11, Mark::Bold));
        assert_eq!(d.to_markdown(), "**hello** world");
        d.toggle_mark(0, 11, Mark::Bold);
        assert_eq!(
            d.to_markdown(),
            "**hello world**",
            "partly bold becomes all bold"
        );
        d.toggle_mark(0, 11, Mark::Bold);
        assert_eq!(d.blocks[0].runs.len(), 1, "runs merge back");
    }
    #[test]
    fn links_apply_and_are_found() {
        let mut d = doc("see docs here");
        d.set_link(4, 8, Some("https://a.b"));
        assert_eq!(d.to_markdown(), "see [docs](https://a.b) here");
        assert_eq!(d.link_at(6), Some((4, 8, "https://a.b".to_string())));
        assert_eq!(d.link_at(1), None);
        d.set_link(4, 8, None);
        assert_eq!(d.to_markdown(), "see docs here");
    }
    #[test]
    fn typing_after_a_link_does_not_extend_it() {
        let mut d = doc("[ab](u)");
        let (m, l) = d.style_at(2);
        assert_eq!(l, None);
        d.replace(2, 2, "c", m, l.as_deref());
        assert_eq!(d.to_markdown(), "[ab](u)c");
    }
    #[test]
    fn block_kinds_toggle() {
        let mut d = doc("a\n\nb");
        d.toggle_kind(0, d.len(), BlockKind::Bullet);
        assert!(d.blocks.iter().all(|b| b.kind == BlockKind::Bullet));
        d.toggle_kind(0, d.len(), BlockKind::Bullet);
        assert!(d.blocks.iter().all(|b| b.kind == BlockKind::Paragraph));
    }
}
