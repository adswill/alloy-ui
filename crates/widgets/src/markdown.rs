//! Markdown view: headings, paragraphs, emphasis, inline code, links, lists, quotes, code blocks.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Clone, PartialEq, Default)]
struct Span {
    text: String,
    bold: bool,
    emph: bool,
    code: bool,
    link: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Heading(u8, Vec<Span>),
    Para(Vec<Span>),
    Quote(Vec<Span>),
    Code(String),
    Item {
        depth: usize,
        number: Option<usize>,
        spans: Vec<Span>,
    },
    Rule,
}

/// Parse the inline markup of one paragraph.
fn inline(src: &str) -> Vec<Span> {
    let cs: Vec<char> = src.chars().collect();
    let mut spans = Vec::new();
    let (mut bold, mut emph) = (false, false);
    let mut cur = String::new();
    let flush = |cur: &mut String, spans: &mut Vec<Span>, bold: bool, emph: bool| {
        if !cur.is_empty() {
            spans.push(Span {
                text: std::mem::take(cur),
                bold,
                emph,
                ..Span::default()
            });
        }
    };
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let next = cs.get(i + 1).copied();
        match c {
            '\\' if next.is_some_and(|n| "\\`*_[]()#-+.!>".contains(n)) => {
                cur.push(cs[i + 1]);
                i += 2;
            }
            '*' if next == Some('*') => {
                flush(&mut cur, &mut spans, bold, emph);
                bold = !bold;
                i += 2;
            }
            '*' => {
                flush(&mut cur, &mut spans, bold, emph);
                emph = !emph;
                i += 1;
            }
            '_' if (i == 0 || !cs[i - 1].is_alphanumeric() || emph)
                && (next.is_some_and(char::is_alphanumeric) || emph) =>
            {
                flush(&mut cur, &mut spans, bold, emph);
                emph = !emph;
                i += 1;
            }
            '`' => {
                if let Some(end) = cs[i + 1..].iter().position(|&x| x == '`') {
                    flush(&mut cur, &mut spans, bold, emph);
                    spans.push(Span {
                        text: cs[i + 1..i + 1 + end].iter().collect(),
                        bold,
                        emph,
                        code: true,
                        link: None,
                    });
                    i += end + 2;
                } else {
                    cur.push(c);
                    i += 1;
                }
            }
            '[' => {
                let close = cs[i + 1..].iter().position(|&x| x == ']');
                let link = close.and_then(|e| {
                    let at = i + 1 + e;
                    (cs.get(at + 1) == Some(&'('))
                        .then(|| cs[at + 2..].iter().position(|&x| x == ')'))
                        .flatten()
                        .map(|u| (e, at, u))
                });
                if let Some((e, at, u)) = link {
                    flush(&mut cur, &mut spans, bold, emph);
                    spans.push(Span {
                        text: cs[i + 1..i + 1 + e].iter().collect(),
                        bold,
                        emph,
                        code: false,
                        link: Some(cs[at + 2..at + 2 + u].iter().collect()),
                    });
                    i = at + 3 + u;
                } else {
                    cur.push(c);
                    i += 1;
                }
            }
            _ => {
                cur.push(c);
                i += 1;
            }
        }
    }
    flush(&mut cur, &mut spans, bold, emph);
    spans
}

fn parse(src: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let (mut para, mut quote) = (String::new(), String::new());
    let mut code: Option<String> = None;
    macro_rules! flush {
        () => {
            if !para.is_empty() {
                out.push(Block::Para(inline(&std::mem::take(&mut para))));
            }
            if !quote.is_empty() {
                out.push(Block::Quote(inline(&std::mem::take(&mut quote))));
            }
        };
    }
    for line in src.lines() {
        if let Some(buf) = &mut code {
            if line.trim_start().starts_with("```") {
                out.push(Block::Code(code.take().unwrap_or_default()));
            } else {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(line);
            }
            continue;
        }
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if trimmed.starts_with("```") {
            flush!();
            code = Some(String::new());
        } else if trimmed.is_empty() {
            flush!();
        } else if let Some(h) = trimmed.strip_prefix('#').map(|_| {
            let n = trimmed.chars().take_while(|&c| c == '#').count();
            (n, trimmed[n..].trim())
        }) {
            if h.0 <= 6 && trimmed[h.0..].starts_with(' ') {
                flush!();
                out.push(Block::Heading(h.0 as u8, inline(h.1)));
            } else {
                push_line(&mut para, trimmed);
            }
        } else if trimmed.len() >= 3
            && trimmed
                .chars()
                .all(|c| c == '-' || c == '*' || c == '_' || c == ' ')
            && trimmed.chars().filter(|c| !c.is_whitespace()).count() >= 3
            && trimmed
                .chars()
                .filter(|c| !c.is_whitespace())
                .all(|c| c == trimmed.chars().next().unwrap_or('-'))
        {
            flush!();
            out.push(Block::Rule);
        } else if let Some(q) = trimmed.strip_prefix('>') {
            if !para.is_empty() {
                out.push(Block::Para(inline(&std::mem::take(&mut para))));
            }
            push_line(&mut quote, q.trim());
        } else if let Some(rest) = ["- ", "* ", "+ "]
            .iter()
            .find_map(|m| trimmed.strip_prefix(m))
        {
            flush!();
            out.push(Block::Item {
                depth: indent / 2,
                number: None,
                spans: inline(rest),
            });
        } else if let Some((n, rest)) = ordered(trimmed) {
            flush!();
            out.push(Block::Item {
                depth: indent / 2,
                number: Some(n),
                spans: inline(rest),
            });
        } else {
            if !quote.is_empty() {
                out.push(Block::Quote(inline(&std::mem::take(&mut quote))));
            }
            push_line(&mut para, trimmed);
        }
    }
    if let Some(c) = code {
        out.push(Block::Code(c));
    }
    flush!();
    out
}

fn push_line(buf: &mut String, line: &str) {
    if !buf.is_empty() {
        buf.push(' ');
    }
    buf.push_str(line);
}

fn ordered(s: &str) -> Option<(usize, &str)> {
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    (digits > 0 && s[digits..].starts_with(". ")).then(|| {
        (
            s[..digits].parse().unwrap_or(1),
            s[digits + 2..].trim_start(),
        )
    })
}

/// One positioned piece of a laid-out document, relative to its top-left.
enum Item {
    Word {
        x: f32,
        y: f32,
        w: f32,
        text: String,
        size: f32,
        line_h: f32,
        weight: u16,
        mono: bool,
        color: Color,
        code: bool,
        link: Option<String>,
    },
    Quad {
        rect: Rectangle,
        fill: Color,
        radius: f32,
        border: Option<Border>,
    },
}

struct Laid {
    items: Vec<Item>,
    height: f32,
}

/// Static, wrapped, read-only Markdown. It lays out to a fixed `width` (default 640 px) so it
/// has a definite height inside a [`Scrollable`](crate::Scrollable). Links are clickable and
/// reported through `on_link`. Italic is drawn as a medium weight until a bundled italic face
/// exists. Not supported: tables, images, HTML, nested block quotes.
pub struct Markdown<M> {
    ui: Ui,
    blocks: Vec<Block>,
    width: f32,
    on_link: Option<Box<dyn Fn(String) -> M>>,
    cache: RefCell<Option<Rc<Laid>>>,
}

impl<M: 'static> Markdown<M> {
    /// Render `source`.
    pub fn new(ui: &Ui, source: impl AsRef<str>) -> Self {
        Self {
            ui: ui.clone(),
            blocks: parse(source.as_ref()),
            width: 640.0,
            on_link: None,
            cache: RefCell::new(None),
        }
    }
    /// Wrap width in pixels.
    pub fn width(mut self, px: f32) -> Self {
        self.width = px.max(80.0);
        self
    }
    /// Called with the target of a clicked link.
    pub fn on_link(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.on_link = Some(Box::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn laid(&self) -> Rc<Laid> {
        if let Some(l) = &*self.cache.borrow() {
            return l.clone();
        }
        let l = Rc::new(self.layout_blocks());
        *self.cache.borrow_mut() = Some(l.clone());
        l
    }

    /// Greedy inline wrap of `spans` at `x0`, from `y`, `width` wide. Returns the height used.
    #[allow(clippy::too_many_arguments)]
    fn flow(
        &self,
        spans: &[Span],
        x0: f32,
        y: f32,
        width: f32,
        size: f32,
        line_h: f32,
        weight: u16,
        color: Color,
        items: &mut Vec<Item>,
    ) -> f32 {
        let t = self.ui.theme();
        let m = self.ui.metrics();
        let space = (m.width("a a", size) - m.width("aa", size)).max(size * 0.3);
        let (mut x, mut line) = (0.0f32, 0usize);
        for s in spans {
            let mono = s.code;
            let w_of = |word: &str| {
                if mono {
                    word.chars().count() as f32 * size * 0.92 * 0.6
                } else {
                    m.width(word, size) * if s.bold || weight >= 600 { 1.08 } else { 1.0 }
                }
            };
            let lead = s.text.starts_with(' ');
            let words: Vec<&str> = s.text.split_whitespace().collect();
            if lead && x > 0.0 {
                x += space;
            }
            for (k, word) in words.iter().enumerate() {
                let w = w_of(word) + if mono { 6.0 } else { 0.0 };
                if x > 0.0 && x + w > width {
                    x = 0.0;
                    line += 1;
                }
                items.push(Item::Word {
                    x: x0 + x,
                    y: y + line as f32 * line_h,
                    w,
                    text: (*word).to_owned(),
                    size: if mono { size * 0.92 } else { size },
                    line_h,
                    weight: if s.bold {
                        700
                    } else if s.emph {
                        weight.max(500)
                    } else {
                        weight
                    },
                    mono,
                    color: if s.link.is_some() {
                        t.palette.accent
                    } else {
                        color
                    },
                    code: mono,
                    link: s.link.clone(),
                });
                x += w;
                if k + 1 < words.len() || s.text.ends_with(' ') {
                    x += space;
                }
            }
        }
        (line + 1) as f32 * line_h
    }

    fn layout_blocks(&self) -> Laid {
        let t = self.ui.theme();
        let p = &t.palette;
        let ty = &t.typography;
        let mut items = Vec::new();
        let mut y = 0.0f32;
        let w = self.width;
        let mut prev: Option<&Block> = None;
        for b in &self.blocks {
            let gap = match (prev, b) {
                (None, _) => 0.0,
                (Some(Block::Item { .. }), Block::Item { .. }) => 4.0,
                (Some(Block::Heading(..)), _) => 8.0,
                (_, Block::Heading(1, _)) => 32.0,
                (_, Block::Heading(..)) => 24.0,
                (_, Block::Rule) | (Some(Block::Rule), _) => 16.0,
                _ => 12.0,
            };
            y += gap;
            match b {
                Block::Heading(level, spans) => {
                    let s = match level {
                        1 => ty.display,
                        2 => ty.heading,
                        3 => ty.subheading,
                        _ => ty.body,
                    };
                    let weight = if *level >= 4 {
                        600
                    } else {
                        s.weight.0.max(600)
                    };
                    y += self.flow(
                        spans,
                        0.0,
                        y,
                        w,
                        s.size,
                        s.line_height,
                        weight,
                        p.text,
                        &mut items,
                    );
                    if *level == 1 {
                        // A hairline under the title carries the hierarchy without a box.
                        y += 8.0;
                        items.push(Item::Quad {
                            rect: Rectangle::new(Point::new(0.0, y), Size::new(w, 1.0)),
                            fill: p.border,
                            radius: 0.0,
                            border: None,
                        });
                        y += 1.0;
                    }
                }
                Block::Para(spans) => {
                    let s = ty.body;
                    y += self.flow(
                        spans,
                        0.0,
                        y,
                        w,
                        s.size,
                        s.line_height * 1.1,
                        400,
                        p.text,
                        &mut items,
                    );
                }
                Block::Quote(spans) => {
                    let s = ty.body;
                    items.push(Item::Quad {
                        rect: Rectangle::new(Point::new(0.0, y), Size::new(2.0, s.line_height)),
                        fill: p.border_strong,
                        radius: 1.0,
                        border: None,
                    });
                    let before = items.len();
                    let h = self.flow(
                        spans,
                        16.0,
                        y,
                        w - 16.0,
                        s.size,
                        s.line_height * 1.1,
                        400,
                        p.text_secondary,
                        &mut items,
                    );
                    if let Item::Quad { rect, .. } = &mut items[before - 1] {
                        rect.size.height = h;
                    }
                    y += h;
                }
                Block::Item {
                    depth,
                    number,
                    spans,
                } => {
                    let s = ty.body;
                    let lh = s.line_height * 1.1;
                    let x = 4.0 + *depth as f32 * 22.0;
                    match number {
                        None => items.push(Item::Quad {
                            rect: Rectangle::new(
                                Point::new(x + 3.0, y + lh / 2.0 - 2.5),
                                Size::new(5.0, 5.0),
                            ),
                            fill: if *depth == 0 {
                                p.text_secondary
                            } else {
                                Color::TRANSPARENT
                            },
                            radius: 2.5,
                            border: (*depth > 0).then_some(Border {
                                color: p.text_secondary,
                                width: 1.0,
                            }),
                        }),
                        Some(n) => items.push(Item::Word {
                            x,
                            y,
                            w: 20.0,
                            text: format!("{n}."),
                            size: s.size,
                            line_h: lh,
                            weight: 400,
                            mono: false,
                            color: p.text_secondary,
                            code: false,
                            link: None,
                        }),
                    }
                    let ind = x + 22.0;
                    y += self.flow(spans, ind, y, w - ind, s.size, lh, 400, p.text, &mut items);
                }
                Block::Code(text) => {
                    let s = ty.body;
                    let (size, lh) = (s.size * 0.9, s.line_height * 0.95);
                    let pad = t.spacing.md;
                    let lines: Vec<&str> = text.split('\n').collect();
                    let h = lines.len() as f32 * lh + 2.0 * pad;
                    items.push(Item::Quad {
                        rect: Rectangle::new(Point::new(0.0, y), Size::new(w, h)),
                        fill: p.surface,
                        radius: t.radii.medium,
                        border: Some(Border {
                            color: p.border,
                            width: 1.0,
                        }),
                    });
                    for (i, l) in lines.iter().enumerate() {
                        if !l.is_empty() {
                            items.push(Item::Word {
                                x: pad,
                                y: y + pad + i as f32 * lh,
                                w: w - 2.0 * pad,
                                text: (*l).to_owned(),
                                size,
                                line_h: lh,
                                weight: 400,
                                mono: true,
                                color: p.text,
                                code: false,
                                link: None,
                            });
                        }
                    }
                    y += h;
                }
                Block::Rule => {
                    items.push(Item::Quad {
                        rect: Rectangle::new(Point::new(0.0, y), Size::new(w, 1.0)),
                        fill: p.border,
                        radius: 0.0,
                        border: None,
                    });
                    y += 1.0;
                }
            }
            prev = Some(b);
        }
        Laid { items, height: y }
    }
}

impl<M: 'static> Widget<M> for Markdown<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fixed(self.width), Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(self.width, self.laid().height)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let (Event::MousePressed(MouseButton::Left), Some(f)) = (event, &self.on_link) else {
            return false;
        };
        let o = layout.bounds.origin;
        for it in &self.laid().items {
            if let Item::Word {
                x,
                y,
                w,
                line_h,
                link: Some(url),
                ..
            } = it
            {
                if Rectangle::new(Point::new(o.x + x, o.y + y), Size::new(*w, *line_h))
                    .contains(cursor)
                {
                    out.push(f(url.clone()));
                    return true;
                }
            }
        }
        false
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let t = self.ui.theme();
        let o = b.origin;
        for it in &self.laid().items {
            match it {
                Item::Quad {
                    rect,
                    fill,
                    radius,
                    border,
                } => {
                    let r = Rectangle::new(
                        Point::new(o.x + rect.origin.x, o.y + rect.origin.y),
                        rect.size,
                    );
                    if !self.ui.culled(&r) {
                        out.push(DrawCommand::Quad {
                            bounds: r,
                            fill: *fill,
                            radius: *radius,
                            border: *border,
                            shadow: None,
                        });
                    }
                }
                Item::Word {
                    x,
                    y,
                    w,
                    text,
                    size,
                    line_h,
                    weight,
                    mono,
                    color,
                    code,
                    link,
                } => {
                    let r = Rectangle::new(Point::new(o.x + x, o.y + y), Size::new(*w, *line_h));
                    if self.ui.culled(&r) {
                        continue;
                    }
                    if *code {
                        out.push(DrawCommand::Quad {
                            bounds: Rectangle::new(
                                Point::new(r.origin.x, r.origin.y + 1.0),
                                Size::new(*w, *line_h - 2.0),
                            ),
                            fill: t.palette.surface,
                            radius: t.radii.small,
                            border: Some(Border {
                                color: t.palette.border,
                                width: 1.0,
                            }),
                            shadow: None,
                        });
                    }
                    let hover = link.is_some() && r.contains(self.ui.cursor());
                    out.push(DrawCommand::Text {
                        origin: Point::new(r.origin.x + if *code { 3.0 } else { 0.0 }, r.origin.y),
                        content: text.clone(),
                        size: *size,
                        color: if hover {
                            t.palette.accent_hover
                        } else {
                            *color
                        },
                        max_width: None,
                        weight: *weight,
                        family: if *mono {
                            FontFamily::Mono
                        } else {
                            FontFamily::Sans
                        },
                        line_height: Some(*line_h),
                    });
                    if link.is_some() {
                        out.push(DrawCommand::Quad {
                            bounds: Rectangle::new(
                                Point::new(r.origin.x, r.origin.y + *line_h - 4.0),
                                Size::new(*w, 1.0),
                            ),
                            fill: color.with_alpha(0.55),
                            radius: 0.0,
                            border: None,
                            shadow: None,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_styles() {
        let s = inline("a **b** *c* `d` [e](http://x)");
        assert_eq!(s.iter().filter(|s| s.bold).count(), 1);
        assert_eq!(s.iter().filter(|s| s.emph).count(), 1);
        assert_eq!(s.iter().filter(|s| s.code).count(), 1);
        assert_eq!(
            s.iter().find_map(|s| s.link.clone()).as_deref(),
            Some("http://x")
        );
    }

    #[test]
    fn snake_case_is_not_emphasis() {
        let s = inline("use my_var_name here");
        assert!(s.iter().all(|s| !s.emph));
    }

    #[test]
    fn blocks() {
        let b = parse("# T\n\nhello\nworld\n\n- a\n- b\n1. c\n\n```\nx\n  y\n```\n> q\n---\n");
        assert!(matches!(b[0], Block::Heading(1, _)));
        assert!(matches!(&b[1], Block::Para(s) if s[0].text == "hello world"));
        assert!(matches!(b[2], Block::Item { number: None, .. }));
        assert!(matches!(
            b[4],
            Block::Item {
                number: Some(1),
                ..
            }
        ));
        assert!(matches!(&b[5], Block::Code(c) if c == "x\n  y"));
        assert!(matches!(b[6], Block::Quote(_)));
        assert!(matches!(b[7], Block::Rule));
    }
}
