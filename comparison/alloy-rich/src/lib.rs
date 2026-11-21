//! The finish shared by the three rich demos: pills, swatches, a chart with axes, key caps,
//! settings rows and a sized text preview.
use alloy_ui::prelude::*;
use alloy_ui::{
    theme::Palette,
    style::{Length, Style},
    view::Boxed,
    DrawCommand, DrawList, Element, Event, Layout, MouseButton, Point, Rectangle, Size, Widget,
};

pub const INDIGO: u32 = 0x4F6BED;

fn quad(bounds: Rectangle, fill: Color, radius: f32) -> DrawCommand {
    DrawCommand::Quad { bounds, fill, radius, border: None, shadow: None }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn label(out: &mut DrawList, s: &str, x: f32, y: f32, size: f32, color: Color, right: bool) {
    let x = if right { x - s.chars().count() as f32 * size * 0.56 } else { x };
    let family = Default::default();
    out.push(DrawCommand::Text {
        origin: Point::new(x, y),
        content: s.into(),
        size,
        color,
        max_width: None,
        weight: 400,
        family,
        line_height: None,
    });
}

/// Amber that reads on light and dark surfaces.
pub fn amber() -> Color {
    Color::hex(if palette().background.r < 0.5 { 0xE3A83D } else { 0xA85F00 })
}

/// Success, warning or danger colour for a status word.
pub fn tone(status: &str) -> Color {
    let p = palette();
    match status {
        "Paid" | "Active" | "Up" => p.success,
        "Pending" => amber(),
        "Refunded" | "Failed" => p.danger,
        _ => p.text_secondary,
    }
}

/// A tinted status pill.
pub fn pill<M: 'static>(s: &str) -> Boxed<M> {
    let c = tone(s);
    caption(s).color(c).pad_xy(9.0, 3.0).bg(c.with_alpha(0.13)).round(10.0)
}

/// A small outlined chip; `on` fills it with the accent tint.
pub fn chip<M: 'static>(s: &str, on: bool) -> Boxed<M> {
    let c = palette().accent;
    let b = caption(s).color(if on { c } else { palette().text_secondary }).pad_xy(10.0, 4.0);
    if on { b.bg(c.with_alpha(0.12)).round(12.0) } else { b.outline().round(12.0) }
}

/// A key cap, drawn for the platform's modifier.
pub fn kbd<M: 'static>(keys: &str) -> Boxed<M> {
    let m = if cfg!(target_os = "macos") { "\u{2318}" } else { "Ctrl+" };
    caption(keys.replace("mod+", m)).dim().pad_xy(6.0, 2.0).bg(palette().surface_raised).outline().round(4.0)
}

/// A card of rows divided by hairlines.
pub fn group<M: 'static>(rows: Vec<Node<M>>) -> Boxed<M> {
    let n = rows.len();
    let rows = rows.into_iter().enumerate().flat_map(|(i, r)| {
        let r = r.pad_xy(16.0, 12.0).fill_w().into();
        [Some(r), (i + 1 < n).then(rule)]
    });
    col(rows.flatten()).gap(0.0).card().pad(0.0).fill_w()
}

/// A settings row: title and hint on the left, the control on the right.
pub fn setting<M: 'static>(title: &str, hint: impl Into<String>, control: impl Into<Node<M>>) -> Node<M> {
    let hint = hint.into();
    row![col![text(title).label(), caption(&hint).dim().when(!hint.is_empty())].gap(2.0), space(), control].align_center().into()
}

/// An empty state: a title, a hint and an optional action below.
pub fn empty<M: 'static>(title: &str, hint: &str, action: impl Into<Node<M>>) -> Node<M> {
    col![heading(title), caption(hint).dim(), vspace(), action].gap(6.0).pad(32.0).center().into()
}

struct Swatches<M> {
    colors: Vec<Color>,
    sel: usize,
    on: Box<dyn Fn(usize) -> M>,
}

const D: f32 = 24.0;
const G: f32 = 12.0;

impl<M> Widget<M> for Swatches<M> {
    fn style(&self) -> Style {
        Style::default()
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(self.colors.len() as f32 * (D + G) - G, D + 8.0)
    }
    fn on_event(&mut self, e: &Event, l: &Layout, at: Point, out: &mut Vec<M>) -> bool {
        let hit = (at.x - l.bounds.origin.x) / (D + G);
        let inside = l.bounds.contains(at) && hit.fract() * (D + G) <= D;
        if matches!(e, Event::MousePressed(MouseButton::Left)) && inside {
            out.push((self.on)(hit as usize));
            return true;
        }
        false
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let b = l.bounds;
        for (i, c) in self.colors.iter().enumerate() {
            let x = b.origin.x + i as f32 * (D + G);
            if i == self.sel {
                let ring = DrawCommand::Quad {
                    bounds: rect(x - 4.0, b.origin.y, D + 8.0, D + 8.0),
                    fill: Color::TRANSPARENT,
                    radius: (D + 8.0) / 2.0,
                    border: Some(alloy_ui::Border { color: *c, width: 2.0 }),
                    shadow: None,
                };
                out.push(ring);
            }
            out.push(quad(rect(x, b.origin.y + 4.0, D, D), *c, D / 2.0));
        }
    }
}

/// Round colour swatches; the selected one wears a ring. `f` receives the picked index.
pub fn swatches<S: 'static>(colors: &[Color], sel: usize, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Node<Act<S>> {
    let f = std::sync::Arc::new(f);
    let on = Box::new(move |i| {
        let f = f.clone();
        Act::new(move |s: &mut S| f(s, i))
    });
    Node::from(Element::new(Swatches { colors: colors.to_vec(), sel, on }))
}

struct Preview(String, f32, Color);

impl<M> Widget<M> for Preview {
    fn measure(&self, _: Size) -> Size {
        Size::new(self.0.chars().count() as f32 * self.1 * 0.56, self.1 * 1.5)
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        label(out, &self.0, l.bounds.origin.x, l.bounds.origin.y, self.1, self.2, false);
    }
}

/// Text at any pixel size, for live previews.
pub fn sized_text<M: 'static>(s: &str, px: f32) -> Node<M> {
    Node::from(Element::new(Preview(s.into(), px, palette().text)))
}

/// A bar chart with a value axis, a label under every bar, gridlines, a selected bar
/// (click one to pick it) and bars that grow in with `grow` from 0 to 1.
pub struct Plot<M> {
    bars: Vec<(String, f32)>,
    top: f32,
    fmt: fn(f32) -> String,
    sel: usize,
    grow: f32,
    on: Option<Box<dyn Fn(usize) -> M>>,
    p: Palette,
    line: bool,
}

pub fn plot<M>(bars: Vec<(String, f32)>, top: f32, fmt: fn(f32) -> String) -> Plot<M> {
    Plot { bars, top, fmt, sel: usize::MAX, grow: 1.0, on: None, p: palette(), line: false }
}

impl<S: 'static> Plot<Act<S>> {
    /// Run `f` on the state with the clicked bar's index.
    pub fn act(self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        let f = std::sync::Arc::new(f);
        self.on(move |i| {
            let f = f.clone();
            Act::new(move |s| f(s, i))
        })
    }
}

impl<M> Plot<M> {
    pub fn selected(mut self, i: usize) -> Self {
        self.sel = i;
        self
    }
    /// Draw a line with markers instead of bars.
    pub fn line(mut self, yes: bool) -> Self {
        self.line = yes;
        self
    }
    pub fn grow(mut self, t: f32) -> Self {
        self.grow = 1.0 - (1.0 - t).powi(3);
        self
    }
    pub fn on(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
    fn geometry(&self, b: Rectangle) -> (f32, f32, f32, f32) {
        let (left, bottom) = (46.0, 26.0);
        let n = self.bars.len().max(1) as f32;
        (b.origin.x + left, b.size.width - left, b.size.height - bottom, (b.size.width - left) / n)
    }
}

impl<M: 'static> Widget<M> for Plot<M> {
    fn style(&self) -> Style {
        Style { width: Length::Fill(1), height: Length::Fill(1), ..Style::default() }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(0.0, 220.0)
    }
    fn on_event(&mut self, e: &Event, l: &Layout, at: Point, out: &mut Vec<M>) -> bool {
        let (x0, w, h, slot) = self.geometry(l.bounds);
        let inside = at.x >= x0 && at.x < x0 + w && at.y >= l.bounds.origin.y && at.y < l.bounds.origin.y + h;
        match (&self.on, e) {
            (Some(f), Event::MousePressed(MouseButton::Left)) if inside => {
                out.push(f(((at.x - x0) / slot) as usize));
                true
            }
            _ => false,
        }
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let p = &self.p;
        let (x0, w, h, slot) = self.geometry(l.bounds);
        let y0 = l.bounds.origin.y;
        for i in 0..=4 {
            let y = y0 + h * (1.0 - i as f32 / 4.0);
            let line = if i == 0 { p.border_control } else { p.border.with_alpha(0.7) };
            out.push(quad(rect(x0, y - 0.5, w, 1.0), line, 0.0));
            label(out, &(self.fmt)(self.top * i as f32 / 4.0), x0 - 10.0, y - 7.0, 11.0, p.text_secondary, true);
        }
        let inset = (slot * 0.2).min(14.0);
        let mut last: Option<Point> = None;
        for (i, (name, v)) in self.bars.iter().enumerate() {
            let x = x0 + i as f32 * slot + inset;
            let bh = h * (v / self.top).clamp(0.0, 1.0) * self.grow;
            let on = i == self.sel || self.sel == usize::MAX;
            let c = if on { p.accent } else { p.accent.with_alpha(0.38) };
            if self.line {
                let at = Point::new(x0 + (i as f32 + 0.5) * slot, y0 + h - bh);
                if let Some(from) = last {
                    out.push(DrawCommand::Line { from, to: at, width: 2.0, color: p.accent });
                }
                last = Some(at);
                let r = if i == self.sel { 5.0 } else { 3.5 };
                out.push(quad(rect(at.x - r - 2.0, at.y - r - 2.0, r * 2.0 + 4.0, r * 2.0 + 4.0), p.surface, r + 2.0));
                out.push(quad(rect(at.x - r, at.y - r, r * 2.0, r * 2.0), p.accent, r));
            } else {
                out.push(quad(rect(x, y0 + h - bh, slot - inset * 2.0, bh), c, 3.0));
            }
            let name_w = name.chars().count() as f32 * 6.2;
            label(out, name, x0 + (i as f32 + 0.5) * slot - name_w / 2.0, y0 + h + 8.0, 11.0, p.text_secondary, false);
        }
    }
}

impl<M: 'static> From<Plot<M>> for Node<M> {
    fn from(p: Plot<M>) -> Self {
        Node::from(Element::new(p))
    }
}

struct Spark(Vec<f32>, Color);

impl<M> Widget<M> for Spark {
    fn measure(&self, _: Size) -> Size {
        Size::new(72.0, 28.0)
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let b = l.bounds;
        let (lo, hi) = self.0.iter().fold((f32::MAX, f32::MIN), |(a, z), v| (a.min(*v), z.max(*v)));
        let pt = |i: usize, v: f32| {
            let x = b.origin.x + b.size.width * i as f32 / (self.0.len() - 1) as f32;
            Point::new(x, b.origin.y + b.size.height * (1.0 - (v - lo) / (hi - lo).max(1e-6)))
        };
        for i in 1..self.0.len() {
            out.push(DrawCommand::Line { from: pt(i - 1, self.0[i - 1]), to: pt(i, self.0[i]), width: 1.5, color: self.1 });
        }
    }
}

/// A 72 by 28 px trend line.
pub fn spark<M: 'static>(values: &[f32], color: Color) -> Node<M> {
    Node::from(Element::new(Spark(values.to_vec(), color)))
}

/// The label of a choice variant.
pub fn name<T: Choice>(v: T) -> &'static str {
    T::all().iter().find(|(_, x)| *x == v).map_or("", |(l, _)| l)
}

struct Stack(Vec<(f32, Color)>);

impl<M> Widget<M> for Stack {
    fn style(&self) -> Style {
        Style { width: Length::Fill(1), height: Length::Fixed(10.0), ..Style::default() }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(0.0, 10.0)
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let b = l.bounds;
        let total: f32 = self.0.iter().map(|p| p.0).sum();
        let mut x = b.origin.x;
        for (v, c) in &self.0 {
            let w = (b.size.width - 2.0 * (self.0.len() - 1) as f32) * v / total;
            out.push(quad(rect(x, b.origin.y, w, b.size.height), *c, 3.0));
            x += w + 2.0;
        }
    }
}

/// A proportional bar split into coloured parts.
pub fn stack<M: 'static>(parts: &[(f32, Color)]) -> Node<M> {
    Node::from(Element::new(Stack(parts.to_vec())))
}

/// A colour dot followed by a caption.
pub fn legend<M: 'static>(c: Color, s: impl Into<String>) -> Node<M> {
    row![col![space()].w(8.0).h(8.0).bg(c).round(4.0), caption(s).dim()].gap(6.0).align_center().into()
}
