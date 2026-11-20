//! Stand-in widget kit for the examples.
//!
//! `alloy-ui-widgets` is still being written by another workstream, so the
//! examples build their UI from these small widgets, which implement the
//! public [`Widget`] contract from `alloy-ui-core`. Once the real widget crate
//! lands, `kit` is replaced by it; the applications' `State`/`Message`/
//! `update` code does not change.
//!
//! Known limits (the contract gives widgets no cursor in `draw` and no
//! persistent state): no hover or pressed visuals, no slider dragging, and
//! text width is estimated rather than shaped.

use alloy_ui_core::style::{Align, Axis, Length, Padding, Style};
use alloy_ui_core::{
    Border, Color, DrawCommand, DrawList, Element, Event, Key, Layout, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::{compute, Node};

/// Estimated advance of one character, as a fraction of the font size.
const ADVANCE: f32 = 0.55;

/// Estimate the pixel width of `s` at `size`.
pub fn text_width(s: &str, size: f32) -> f32 {
    s.chars().count() as f32 * size * ADVANCE
}

fn build_node<M>(e: &Element<M>, avail: Size) -> Node {
    let kids = e.0.children();
    let style = e.0.style();
    if kids.is_empty() {
        let s = e.0.measure(avail);
        let mut n = Node::leaf(s.width, s.height);
        n.style = style;
        n
    } else {
        let mut n = Node::container(style.axis, kids.iter().map(|k| build_node(k, avail)).collect());
        n.style = style;
        n
    }
}

/// The layouter handed to [`alloy_ui_core::runtime::Runtime::new`].
pub fn layouter<M>(root: &Element<M>, size: Size) -> Layout {
    compute(&build_node(root, size), Point::new(0.0, 0.0), size)
}

fn quad(bounds: Rectangle, fill: Color, radius: f32, border: Option<Border>) -> DrawCommand {
    DrawCommand::Quad { bounds, fill, radius, border, shadow: None }
}

/// Text label.
pub struct Label {
    content: String,
    size: f32,
    color: Color,
    width: Length,
}

/// Build a label.
pub fn label(content: impl Into<String>, size: f32, color: Color) -> Label {
    Label { content: content.into(), size, color, width: Length::Shrink }
}

impl Label {
    /// Make the label take a share of the row.
    pub fn fill(mut self) -> Self {
        self.width = Length::Fill(1);
        self
    }
}

impl<M> Widget<M> for Label {
    fn style(&self) -> Style {
        Style { width: self.width, ..Style::default() }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(text_width(&self.content, self.size), self.size * 1.4)
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        out.push(DrawCommand::Text {
            origin: l.bounds.origin,
            content: self.content.clone(),
            size: self.size,
            color: self.color,
            max_width: None,
        });
    }
}
impl<M: 'static> From<Label> for Element<M> {
    fn from(w: Label) -> Self {
        Element::new(w)
    }
}

/// Visual weight of a button.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Emphasis {
    /// The one accent-filled action per view.
    Primary,
    /// Outlined.
    Secondary,
    /// Text only.
    Quiet,
}

/// A pressable button.
pub struct Button<M> {
    text: String,
    emphasis: Emphasis,
    on_press: M,
    pal: alloy_ui_theme::Palette,
    selected: bool,
}

/// Build a button.
pub fn button<M>(text: impl Into<String>, emphasis: Emphasis, pal: &alloy_ui_theme::Palette, on_press: M) -> Button<M> {
    Button { text: text.into(), emphasis, on_press, pal: pal.clone(), selected: false }
}

impl<M> Button<M> {
    /// Mark as the active choice in a segmented group.
    pub fn selected(mut self, yes: bool) -> Self {
        self.selected = yes;
        self
    }
}

impl<M: Clone> Widget<M> for Button<M> {
    fn measure(&self, _: Size) -> Size {
        Size::new(text_width(&self.text, 14.0) + 24.0, 32.0)
    }
    fn on_event(&mut self, e: &Event, l: &Layout, cursor: Point, out: &mut Vec<M>) -> bool {
        if matches!(e, Event::MousePressed(_)) && l.bounds.contains(cursor) {
            out.push(self.on_press.clone());
            return true;
        }
        false
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let p = &self.pal;
        let (fill, fg, border) = match (self.emphasis, self.selected) {
            (Emphasis::Primary, _) => (p.accent, p.on_accent, None),
            (_, true) => (p.selection, p.text, Some(Border { color: p.accent, width: 1.0 })),
            (Emphasis::Secondary, _) => (p.surface_raised, p.text, Some(Border { color: p.border_strong, width: 1.0 })),
            (Emphasis::Quiet, _) => (Color::TRANSPARENT, p.accent, None),
        };
        out.push(quad(l.bounds, fill, 4.0, border));
        let tw = text_width(&self.text, 14.0);
        out.push(DrawCommand::Text {
            origin: Point::new(
                l.bounds.origin.x + (l.bounds.size.width - tw) / 2.0,
                l.bounds.origin.y + (l.bounds.size.height - 14.0 * 1.4) / 2.0,
            ),
            content: self.text.clone(),
            size: 14.0,
            color: fg,
            max_width: None,
        });
    }
}
impl<M: Clone + 'static> From<Button<M>> for Element<M> {
    fn from(w: Button<M>) -> Self {
        Element::new(w)
    }
}

/// A checkbox with a trailing label; emits `on_toggle` when clicked.
pub struct Checkbox<M> {
    text: String,
    checked: bool,
    on_toggle: M,
    pal: alloy_ui_theme::Palette,
}

/// Build a checkbox.
pub fn checkbox<M>(text: impl Into<String>, checked: bool, pal: &alloy_ui_theme::Palette, on_toggle: M) -> Checkbox<M> {
    Checkbox { text: text.into(), checked, on_toggle, pal: pal.clone() }
}

impl<M: Clone> Widget<M> for Checkbox<M> {
    fn style(&self) -> Style {
        Style { width: Length::Fill(1), ..Style::default() }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(26.0 + text_width(&self.text, 14.0), 24.0)
    }
    fn on_event(&mut self, e: &Event, l: &Layout, cursor: Point, out: &mut Vec<M>) -> bool {
        if matches!(e, Event::MousePressed(_)) && l.bounds.contains(cursor) {
            out.push(self.on_toggle.clone());
            return true;
        }
        false
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let p = &self.pal;
        let o = l.bounds.origin;
        let bx = Rectangle::new(Point::new(o.x, o.y + 3.0), Size::new(18.0, 18.0));
        if self.checked {
            out.push(quad(bx, p.accent, 3.0, None));
            out.push(quad(
                Rectangle::new(Point::new(o.x + 5.0, o.y + 8.0), Size::new(8.0, 8.0)),
                p.on_accent,
                1.0,
                None,
            ));
        } else {
            out.push(quad(bx, p.surface_raised, 3.0, Some(Border { color: p.border_strong, width: 1.0 })));
        }
        let color = if self.checked { p.text_secondary } else { p.text };
        out.push(DrawCommand::Text {
            origin: Point::new(o.x + 26.0, o.y + 2.0),
            content: self.text.clone(),
            size: 14.0,
            color,
            max_width: None,
        });
    }
}
impl<M: Clone + 'static> From<Checkbox<M>> for Element<M> {
    fn from(w: Checkbox<M>) -> Self {
        Element::new(w)
    }
}

/// A horizontal slider over `0.0..=1.0`; click to set.
pub struct Slider<M> {
    value: f32,
    on_change: fn(f32) -> M,
    pal: alloy_ui_theme::Palette,
}

/// Build a slider.
pub fn slider<M>(value: f32, pal: &alloy_ui_theme::Palette, on_change: fn(f32) -> M) -> Slider<M> {
    Slider { value: value.clamp(0.0, 1.0), on_change, pal: pal.clone() }
}

impl<M> Widget<M> for Slider<M> {
    fn style(&self) -> Style {
        Style { width: Length::Fill(1), ..Style::default() }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(120.0, 24.0)
    }
    fn on_event(&mut self, e: &Event, l: &Layout, cursor: Point, out: &mut Vec<M>) -> bool {
        if matches!(e, Event::MousePressed(_)) && l.bounds.contains(cursor) {
            let t = (cursor.x - l.bounds.origin.x) / l.bounds.size.width.max(1.0);
            out.push((self.on_change)(t.clamp(0.0, 1.0)));
            return true;
        }
        false
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let b = l.bounds;
        let cy = b.origin.y + b.size.height / 2.0;
        let track = Rectangle::new(Point::new(b.origin.x, cy - 2.0), Size::new(b.size.width, 4.0));
        out.push(quad(track, self.pal.border, 2.0, None));
        let w = b.size.width * self.value;
        out.push(quad(Rectangle::new(track.origin, Size::new(w, 4.0)), self.pal.accent, 2.0, None));
        let knob = Rectangle::new(Point::new(b.origin.x + w - 8.0, cy - 8.0), Size::new(16.0, 16.0));
        out.push(quad(knob, self.pal.surface_raised, 8.0, Some(Border { color: self.pal.accent, width: 2.0 })));
    }
}
impl<M: 'static> From<Slider<M>> for Element<M> {
    fn from(w: Slider<M>) -> Self {
        Element::new(w)
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
    Bars { values, color, track }
}

impl<M> Widget<M> for Bars {
    fn style(&self) -> Style {
        Style { width: Length::Fill(1), height: Length::Fixed(120.0), ..Style::default() }
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
            out.push(quad(Rectangle::new(Point::new(x, l.bounds.origin.y), Size::new(w, l.bounds.size.height)), self.track, 2.0, None));
            out.push(quad(Rectangle::new(Point::new(x, base - h), Size::new(w, h)), self.color, 2.0, None));
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
pub fn editor<M>(lines: Vec<String>, caret: (usize, usize), pal: &alloy_ui_theme::Palette, on_input: fn(Input) -> M) -> Editor<M> {
    Editor { lines, caret, on_input, pal: pal.clone(), height: Length::Fill(1) }
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
        Style { width: Length::Fill(1), height: self.height, padding: Padding::all(8.0), ..Style::default() }
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
        out.push(quad(l.bounds, p.surface_raised, 4.0, Some(Border { color: p.border, width: 1.0 })));
        out.push(DrawCommand::PushClip(l.bounds));
        let (x0, y0) = (l.bounds.origin.x + 8.0, l.bounds.origin.y + 8.0);
        for (i, line) in self.lines.iter().enumerate() {
            out.push(DrawCommand::Text {
                origin: Point::new(x0, y0 + i as f32 * LINE_H),
                content: line.clone(),
                size: MONO,
                color: p.text,
                max_width: None,
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

/// A flex container with optional background.
pub struct Flex<M> {
    style: Style,
    children: Vec<Element<M>>,
    fill: Option<Color>,
    border: Option<Border>,
    radius: f32,
}

/// A horizontal container.
pub fn row<M>(children: Vec<Element<M>>) -> Flex<M> {
    Flex { style: Style { axis: Axis::Horizontal, ..Style::default() }, children, fill: None, border: None, radius: 0.0 }
}

/// A vertical container.
pub fn column<M>(children: Vec<Element<M>>) -> Flex<M> {
    Flex { style: Style { axis: Axis::Vertical, ..Style::default() }, children, fill: None, border: None, radius: 0.0 }
}

impl<M> Flex<M> {
    /// Gap between children.
    pub fn spacing(mut self, s: f32) -> Self {
        self.style.spacing = s;
        self
    }
    /// Inner padding.
    pub fn padding(mut self, p: Padding) -> Self {
        self.style.padding = p;
        self
    }
    /// Cross-axis alignment.
    pub fn align(mut self, a: Align) -> Self {
        self.style.align = a;
        self
    }
    /// Width.
    pub fn width(mut self, l: Length) -> Self {
        self.style.width = l;
        self
    }
    /// Height.
    pub fn height(mut self, l: Length) -> Self {
        self.style.height = l;
        self
    }
    /// Fill the background.
    pub fn background(mut self, c: Color) -> Self {
        self.fill = Some(c);
        self
    }
    /// Draw a border with a corner radius.
    pub fn outline(mut self, c: Color, radius: f32) -> Self {
        self.border = Some(Border { color: c, width: 1.0 });
        self.radius = radius;
        self
    }
}

impl<M> Widget<M> for Flex<M> {
    fn style(&self) -> Style {
        self.style
    }
    fn measure(&self, _: Size) -> Size {
        Size::default()
    }
    fn children(&self) -> &[Element<M>] {
        &self.children
    }
    fn on_event(&mut self, e: &Event, l: &Layout, cursor: Point, out: &mut Vec<M>) -> bool {
        for (c, cl) in self.children.iter_mut().zip(&l.children) {
            if c.0.on_event(e, cl, cursor, out) {
                return true;
            }
        }
        false
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        if self.fill.is_some() || self.border.is_some() {
            out.push(quad(l.bounds, self.fill.unwrap_or(Color::TRANSPARENT), self.radius, self.border));
        }
        for (c, cl) in self.children.iter().zip(&l.children) {
            c.0.draw(cl, out);
        }
    }
}
impl<M: 'static> From<Flex<M>> for Element<M> {
    fn from(w: Flex<M>) -> Self {
        Element::new(w)
    }
}

/// An empty element that takes the remaining space.
pub struct Spacer;
/// Build a spacer.
pub fn spacer() -> Spacer {
    Spacer
}
impl<M> Widget<M> for Spacer {
    fn style(&self) -> Style {
        Style { width: Length::Fill(1), height: Length::Fill(1), ..Style::default() }
    }
    fn measure(&self, _: Size) -> Size {
        Size::default()
    }
    fn draw(&self, _: &Layout, _: &mut DrawList) {}
}
impl<M: 'static> From<Spacer> for Element<M> {
    fn from(w: Spacer) -> Self {
        Element::new(w)
    }
}
