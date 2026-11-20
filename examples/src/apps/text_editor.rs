//! Text editor: multi-line buffer with caret movement and a status line.
use crate::kit::{column, editor, label, row, spacer, Input};
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Length, Padding};
use alloy_ui_core::{Application, Command, Element, Key};
use alloy_ui_theme::Theme;

/// Editor state: lines plus a (line, column) caret in characters.
pub struct TextEditor {
    /// Buffer lines, never empty.
    pub lines: Vec<String>,
    /// Caret as (line, column).
    pub caret: (usize, usize),
    theme: Theme,
}

/// Editor messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Raw input from the editor widget.
    Input(Input),
}

fn byte_at(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

impl TextEditor {
    /// A buffer with starter text.
    pub fn new(theme: Theme) -> Self {
        let text = "Alloy draws every pixel itself.\nNo webview, no HTML.\n";
        Self {
            lines: text.split('\n').map(String::from).collect(),
            caret: (2, 0),
            theme,
        }
    }

    fn insert(&mut self, s: &str) {
        for ch in s.chars() {
            let (l, c) = self.caret;
            if ch == '\n' {
                let at = byte_at(&self.lines[l], c);
                let rest = self.lines[l].split_off(at);
                self.lines.insert(l + 1, rest);
                self.caret = (l + 1, 0);
            } else {
                let at = byte_at(&self.lines[l], c);
                self.lines[l].insert(at, ch);
                self.caret.1 += 1;
            }
        }
    }

    fn key(&mut self, k: &Key) {
        let (l, c) = self.caret;
        match k {
            Key::Enter => self.insert("\n"),
            Key::Backspace if c > 0 => {
                let at = byte_at(&self.lines[l], c - 1);
                self.lines[l].remove(at);
                self.caret.1 -= 1;
            }
            Key::Backspace if l > 0 => {
                let cur = self.lines.remove(l);
                self.caret = (l - 1, self.lines[l - 1].chars().count());
                self.lines[l - 1].push_str(&cur);
            }
            Key::Named(n) => match n.as_str() {
                "ArrowLeft" if c > 0 => self.caret.1 -= 1,
                "ArrowLeft" if l > 0 => self.caret = (l - 1, self.lines[l - 1].chars().count()),
                "ArrowRight" if c < self.lines[l].chars().count() => self.caret.1 += 1,
                "ArrowRight" if l + 1 < self.lines.len() => self.caret = (l + 1, 0),
                "ArrowUp" if l > 0 => {
                    self.caret = (l - 1, c.min(self.lines[l - 1].chars().count()))
                }
                "ArrowDown" if l + 1 < self.lines.len() => {
                    self.caret = (l + 1, c.min(self.lines[l + 1].chars().count()))
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Application for TextEditor {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Input(Input::Text(s)) => self.insert(&s),
            Msg::Input(Input::Key(k)) => self.key(&k),
        }
        Command::None
    }

    fn view(&self) -> Element<Msg> {
        let p = &self.theme.palette;
        let words: usize = self
            .lines
            .iter()
            .map(|l| l.split_whitespace().count())
            .sum();
        column(vec![
            editor(self.lines.clone(), self.caret, p, Msg::Input).into(),
            row(vec![
                label(
                    format!("Ln {}, Col {}", self.caret.0 + 1, self.caret.1 + 1),
                    12.0,
                    p.text_secondary,
                )
                .into(),
                spacer().into(),
                label(format!("{words} words"), 12.0, p.text_secondary).into(),
            ])
            .width(Length::Fill(1))
            .into(),
        ])
        .spacing(8.0)
        .padding(Padding::all(16.0))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .background(p.background)
        .into()
    }
}

/// Headless demo: type a line, break it, step back with backspace.
pub fn demo() -> Demo<TextEditor> {
    Demo {
        name: "text_editor",
        app: TextEditor::new(Theme::light()),
        size: (520.0, 260.0),
        steps: vec![
            Step::Type("Typed text lands at the caret."),
            Step::Press(Key::Enter),
            Step::Type("Second line"),
            Step::Press(Key::Backspace),
            Step::Press(Key::Named("ArrowUp".into())),
        ],
    }
}
