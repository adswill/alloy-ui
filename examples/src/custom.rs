//! Widgets the examples need that `alloy-ui-widgets` does not provide yet:
//! a multi-line [`Editor`] and a [`Bars`] chart. Everything else comes from
//! the real widget crate.

use alloy_ui_core::style::{Length, Padding, Style};
use alloy_ui_core::{
    Border, Color, DrawCommand, DrawList, Element, Event, Key, Layout, Point, Rectangle, Size,
    Widget,
};

/// Lay out a view tree with the widgets' own `arrange`.
pub fn layouter<M>(root: &Element<M>, size: Size) -> Layout {
    root.0.arrange(Rectangle::new(Point::new(0.0, 0.0), size))
}

fn quad(bounds: Rectangle, fill: Color, radius: f32, border: Option<Border>) -> DrawCommand {
    DrawCommand::Quad {
        bounds,
        fill,
        radius,
        border,
        shadow: None,
    }
}

/// Vertical bar series.
pub struct Bars {
    values: Vec<f32>,
    color: Color,
    track: Color,
}

/// Build a bar chart over values in `0.0..=1.0`.
pub fn bars(values: Vec<f32>, color: Color, track: Color) -> Bars {
    Bars {
        values,
        color,
        track,
    }
}

impl<M> Widget<M> for Bars {
    fn style(&self) -> Style {
        Style {
            width: Length::Fill(1),
            height: Length::Fixed(120.0),
            ..Style::default()
        }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(0.0, 120.0)
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let n = self.values.len().max(1) as f32;
        let gap = 6.0;
        let w = ((l.bounds.size.width - gap * (n - 1.0)) / n).max(1.0);
        let base = l.bounds.origin.y + l.bounds.size.height;
        for (i, v) in self.values.iter().enumerate() {
            let x = l.bounds.origin.x + i as f32 * (w + gap);
            let h = l.bounds.size.height * v.clamp(0.0, 1.0);
            out.push(quad(
                Rectangle::new(
                    Point::new(x, l.bounds.origin.y),
                    Size::new(w, l.bounds.size.height),
                ),
                self.track,
                2.0,
                None,
            ));
            out.push(quad(
                Rectangle::new(Point::new(x, base - h), Size::new(w, h)),
                self.color,
                2.0,
                None,
            ));
        }
    }
}
impl<M: 'static> From<Bars> for Element<M> {
    fn from(w: Bars) -> Self {
        Element::new(w)
    }
}

/// Input delivered by an [`Editor`].
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// Typed or IME-committed text.
    Text(String),
    /// A non-text key (arrows, backspace, enter).
    Key(Key),
}

/// A text area that shows lines with a caret and reports raw input.
pub struct Editor<M> {
    lines: Vec<String>,
    caret: (usize, usize),
    on_input: fn(Input) -> M,
    pal: alloy_ui_theme::Palette,
    height: Length,
}

/// Build an editor over `lines` with the caret at (line, column).
pub fn editor<M>(
    lines: Vec<String>,
    caret: (usize, usize),
    pal: &alloy_ui_theme::Palette,
    on_input: fn(Input) -> M,
) -> Editor<M> {
    Editor {
        lines,
        caret,
        on_input,
        pal: pal.clone(),
        height: Length::Fill(1),
    }
}

impl<M> Editor<M> {
    /// Fix the height instead of filling.
    pub fn height(mut self, h: f32) -> Self {
        self.height = Length::Fixed(h);
        self
    }
}

const MONO: f32 = 13.0;
const LINE_H: f32 = 20.0;

impl<M> Widget<M> for Editor<M> {
    fn style(&self) -> Style {
        Style {
            width: Length::Fill(1),
            height: self.height,
            padding: Padding::all(8.0),
            ..Style::default()
        }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(0.0, LINE_H)
    }
    fn on_event(&mut self, e: &Event, _: &Layout, _: Point, out: &mut Vec<M>) -> bool {
        match e {
            Event::TextInput(s) => out.push((self.on_input)(Input::Text(s.clone()))),
            Event::KeyPressed(k, _) => out.push((self.on_input)(Input::Key(k.clone()))),
            _ => return false,
        }
        true
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let p = &self.pal;
        out.push(quad(
            l.bounds,
            p.surface_raised,
            4.0,
            Some(Border {
                color: p.border,
                width: 1.0,
            }),
        ));
        out.push(DrawCommand::PushClip(l.bounds));
        let (x0, y0) = (l.bounds.origin.x + 8.0, l.bounds.origin.y + 8.0);
        for (i, line) in self.lines.iter().enumerate() {
            out.push(DrawCommand::Text {
                origin: Point::new(x0, y0 + i as f32 * LINE_H),
                content: line.clone(),
                size: MONO,
                color: p.text,
                max_width: None,
                weight: 400,
                line_height: Some(LINE_H),
            });
        }
        let (cl, cc) = self.caret;
        out.push(quad(
            Rectangle::new(
                Point::new(x0 + cc as f32 * MONO * 0.6, y0 + cl as f32 * LINE_H),
                Size::new(1.5, LINE_H - 2.0),
            ),
            p.accent,
            0.0,
            None,
        ));
        out.push(DrawCommand::PopClip);
    }
}
impl<M: 'static> From<Editor<M>> for Element<M> {
    fn from(w: Editor<M>) -> Self {
        Element::new(w)
    }
}
