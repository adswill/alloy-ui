//! Vector drawing: [`Path`], [`Frame`] and the [`Canvas`] widget.
//!
//! A [`Canvas`] is a rectangle the app draws into with a closure. Coordinates
//! are logical pixels relative to the canvas's top-left corner, y down. Fills
//! are anti-aliased polygons (convex or concave); strokes are round-capped
//! and round-joined.
//!
//! ```ignore
//! Canvas::new(&ui, Length::Fill(1), Length::Fixed(160.0), |f| {
//!     let p = Path::new().move_to(10.0, 120.0).cubic_to(60.0, 0.0, 120.0, 160.0, 200.0, 40.0);
//!     f.stroke(&p, 2.0, f.palette().accent);
//!     f.circle(Point::new(200.0, 40.0), 5.0, f.palette().accent);
//! })
//! ```

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, FontFamily, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use alloy_ui_theme::Palette;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Seg {
    Move(Point),
    Line(Point),
    Quad(Point, Point),
    Cubic(Point, Point, Point),
    Close,
}

/// A sequence of lines and curves, built with chained calls.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path {
    segs: Vec<Seg>,
}

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

impl Path {
    /// An empty path.
    pub fn new() -> Self {
        Self::default()
    }
    /// Start a new sub-path at `(x, y)`.
    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.segs.push(Seg::Move(pt(x, y)));
        self
    }
    /// A straight line to `(x, y)`.
    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.segs.push(Seg::Line(pt(x, y)));
        self
    }
    /// A quadratic Bézier with control point `(cx, cy)`.
    pub fn quad_to(mut self, cx: f32, cy: f32, x: f32, y: f32) -> Self {
        self.segs.push(Seg::Quad(pt(cx, cy), pt(x, y)));
        self
    }
    /// A cubic Bézier with control points `(c1x, c1y)` and `(c2x, c2y)`.
    pub fn cubic_to(mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) -> Self {
        self.segs
            .push(Seg::Cubic(pt(c1x, c1y), pt(c2x, c2y), pt(x, y)));
        self
    }
    /// An arc of the circle at `center` from angle `from` to `to` (radians,
    /// 0 = right, increasing clockwise on screen). Continues the current
    /// sub-path with a line to the arc's start, or starts one.
    pub fn arc(mut self, center: Point, radius: f32, from: f32, to: f32) -> Self {
        let n = arc_steps(radius, (to - from).abs());
        for i in 0..=n {
            let a = from + (to - from) * i as f32 / n as f32;
            let p = pt(center.x + radius * a.cos(), center.y + radius * a.sin());
            self.segs.push(if i == 0 && self.segs.is_empty() {
                Seg::Move(p)
            } else {
                Seg::Line(p)
            });
        }
        self
    }
    /// Close the current sub-path back to its start.
    pub fn close(mut self) -> Self {
        self.segs.push(Seg::Close);
        self
    }

    /// A closed circle.
    pub fn circle(center: Point, radius: f32) -> Self {
        Self::new()
            .arc(center, radius, 0.0, std::f32::consts::TAU)
            .close()
    }
    /// A closed rounded rectangle.
    pub fn rounded_rect(r: Rectangle, radius: f32) -> Self {
        let (x0, y0) = (r.origin.x, r.origin.y);
        let (x1, y1) = (x0 + r.size.width, y0 + r.size.height);
        let k = radius
            .min(r.size.width / 2.0)
            .min(r.size.height / 2.0)
            .max(0.0);
        use std::f32::consts::{FRAC_PI_2, PI};
        Self::new()
            .arc(pt(x0 + k, y0 + k), k, PI, PI + FRAC_PI_2)
            .arc(pt(x1 - k, y0 + k), k, PI + FRAC_PI_2, 2.0 * PI)
            .arc(pt(x1 - k, y1 - k), k, 0.0, FRAC_PI_2)
            .arc(pt(x0 + k, y1 - k), k, FRAC_PI_2, PI)
            .close()
    }
    /// A closed ring sector between two radii, e.g. a donut slice.
    pub fn sector(center: Point, inner: f32, outer: f32, from: f32, to: f32) -> Self {
        let mut p = Self::new().arc(center, outer, from, to);
        if inner > 0.5 {
            p = p.arc(center, inner, to, from);
        } else {
            p = p.line_to(center.x, center.y);
        }
        p.close()
    }
    /// An open polyline through `points`.
    pub fn polyline(points: impl IntoIterator<Item = Point>) -> Self {
        let mut p = Self::new();
        for (i, q) in points.into_iter().enumerate() {
            p.segs
                .push(if i == 0 { Seg::Move(q) } else { Seg::Line(q) });
        }
        p
    }

    /// The path as polylines, curves flattened to about a quarter pixel.
    /// Each entry is `(points, closed)`.
    pub fn flatten(&self) -> Vec<(Vec<Point>, bool)> {
        let mut out: Vec<(Vec<Point>, bool)> = Vec::new();
        let mut cur: Vec<Point> = Vec::new();
        let mut start = Point::default();
        let flush = |cur: &mut Vec<Point>, out: &mut Vec<(Vec<Point>, bool)>, closed: bool| {
            if cur.len() > 1 || (closed && !cur.is_empty()) {
                out.push((std::mem::take(cur), closed));
            } else {
                cur.clear();
            }
        };
        for s in &self.segs {
            match *s {
                Seg::Move(p) => {
                    flush(&mut cur, &mut out, false);
                    start = p;
                    cur.push(p);
                }
                Seg::Line(p) => {
                    if cur.is_empty() {
                        cur.push(start);
                    }
                    cur.push(p);
                }
                Seg::Quad(c, p) => {
                    let a = *cur.last().unwrap_or(&start);
                    if cur.is_empty() {
                        cur.push(a);
                    }
                    let n = curve_steps(a, &[c, p]);
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        cur.push(pt(
                            u * u * a.x + 2.0 * u * t * c.x + t * t * p.x,
                            u * u * a.y + 2.0 * u * t * c.y + t * t * p.y,
                        ));
                    }
                }
                Seg::Cubic(c1, c2, p) => {
                    let a = *cur.last().unwrap_or(&start);
                    if cur.is_empty() {
                        cur.push(a);
                    }
                    let n = curve_steps(a, &[c1, c2, p]);
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let u = 1.0 - t;
                        let (b0, b1, b2, b3) =
                            (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        cur.push(pt(
                            b0 * a.x + b1 * c1.x + b2 * c2.x + b3 * p.x,
                            b0 * a.y + b1 * c1.y + b2 * c2.y + b3 * p.y,
                        ));
                    }
                }
                Seg::Close => {
                    if let (Some(f), Some(l)) = (cur.first().copied(), cur.last().copied()) {
                        if f == l && cur.len() > 1 {
                            cur.pop();
                        }
                    }
                    flush(&mut cur, &mut out, true);
                    cur.push(start);
                }
            }
        }
        flush(&mut cur, &mut out, false);
        out
    }
}

/// Segments needed so a chord deviates under a quarter pixel on an arc.
fn arc_steps(radius: f32, sweep: f32) -> usize {
    let r = radius.abs().max(0.5);
    let step = 2.0 * (1.0 - 0.25 / r).clamp(-1.0, 1.0).acos();
    ((sweep / step.max(0.02)).ceil() as usize).clamp(1, 720)
}

fn curve_steps(a: Point, rest: &[Point]) -> usize {
    let mut len = 0.0;
    let mut prev = a;
    for p in rest {
        len += ((p.x - prev.x).powi(2) + (p.y - prev.y).powi(2)).sqrt();
        prev = *p;
    }
    ((len / 3.0).ceil() as usize).clamp(4, 200)
}

/// A drawing surface handed to a [`Canvas`] closure. Coordinates are relative
/// to the canvas's top-left corner.
pub struct Frame {
    ui: Ui,
    origin: Point,
    size: Size,
    cmds: Vec<DrawCommand>,
}

impl Frame {
    /// A frame of `size` at window position `origin`, drawing for `ui`'s theme.
    pub fn new(ui: &Ui, origin: Point, size: Size) -> Self {
        Self {
            ui: ui.clone(),
            origin,
            size,
            cmds: Vec::new(),
        }
    }
    /// Width and height of the canvas.
    pub fn size(&self) -> Size {
        self.size
    }
    /// The theme palette, so drawings follow light and dark.
    pub fn palette(&self) -> Palette {
        self.ui.theme().palette.clone()
    }
    /// Width of `text` at `size`, to centre or right-align labels.
    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        self.ui
            .metrics()
            .width_styled(text, size, 400, FontFamily::Sans)
    }
    fn at(&self, p: Point) -> Point {
        pt(self.origin.x + p.x, self.origin.y + p.y)
    }
    fn rect_at(&self, r: Rectangle) -> Rectangle {
        Rectangle::new(self.at(r.origin), r.size)
    }
    /// Take the recorded commands.
    pub fn finish(self) -> Vec<DrawCommand> {
        self.cmds
    }

    /// A filled rectangle.
    pub fn rect(&mut self, r: Rectangle, color: Color) {
        self.round_rect(r, 0.0, color);
    }
    /// A filled rectangle with rounded corners.
    pub fn round_rect(&mut self, r: Rectangle, radius: f32, color: Color) {
        let bounds = self.rect_at(r);
        self.cmds.push(DrawCommand::Quad {
            bounds,
            fill: color,
            radius,
            border: None,
            shadow: None,
        });
    }
    /// A rectangle outline drawn inside `r`.
    pub fn stroke_rect(&mut self, r: Rectangle, radius: f32, width: f32, color: Color) {
        let bounds = self.rect_at(r);
        self.cmds.push(DrawCommand::Quad {
            bounds,
            fill: Color::TRANSPARENT,
            radius,
            border: Some(Border { color, width }),
            shadow: None,
        });
    }
    /// A filled circle.
    pub fn circle(&mut self, center: Point, radius: f32, color: Color) {
        let r = Rectangle::new(
            pt(center.x - radius, center.y - radius),
            Size::new(radius * 2.0, radius * 2.0),
        );
        self.round_rect(r, radius, color);
    }
    /// A circle outline of `width`, drawn inside `radius`.
    pub fn stroke_circle(&mut self, center: Point, radius: f32, width: f32, color: Color) {
        let r = Rectangle::new(
            pt(center.x - radius, center.y - radius),
            Size::new(radius * 2.0, radius * 2.0),
        );
        self.stroke_rect(r, radius, width, color);
    }
    /// A straight line with round caps.
    pub fn line(&mut self, from: Point, to: Point, width: f32, color: Color) {
        let (from, to) = (self.at(from), self.at(to));
        self.cmds.push(DrawCommand::Line {
            from,
            to,
            width,
            color,
        });
    }
    /// A filled polygon (simple outline, convex or concave).
    pub fn polygon(&mut self, points: &[Point], color: Color) {
        let points = points.iter().map(|p| self.at(*p)).collect();
        self.cmds.push(DrawCommand::Polygon {
            points,
            fill: color,
        });
    }
    /// Fill every closed sub-path of `path`.
    pub fn fill(&mut self, path: &Path, color: Color) {
        for (pts, _) in path.flatten() {
            if pts.len() >= 3 {
                self.polygon(&pts, color);
            }
        }
    }
    /// Stroke `path` with round caps and joins.
    pub fn stroke(&mut self, path: &Path, width: f32, color: Color) {
        for (pts, closed) in path.flatten() {
            for w in pts.windows(2) {
                self.line(w[0], w[1], width, color);
            }
            if closed && pts.len() > 2 {
                self.line(pts[pts.len() - 1], pts[0], width, color);
            }
        }
    }
    /// Text with its top-left at `p`, in the UI font at weight 400.
    pub fn text(&mut self, p: Point, content: impl Into<String>, size: f32, color: Color) {
        self.text_weight(p, content, size, 400, color);
    }
    /// Text at an explicit weight (600 for semibold).
    pub fn text_weight(
        &mut self,
        p: Point,
        content: impl Into<String>,
        size: f32,
        weight: u16,
        color: Color,
    ) {
        let origin = self.at(p);
        self.cmds.push(DrawCommand::Text {
            origin,
            content: content.into(),
            size,
            color,
            max_width: None,
            weight,
            family: FontFamily::Sans,
            line_height: None,
        });
    }
    /// A registered image (`Renderer::register_image`) stretched over `r`.
    pub fn image(&mut self, r: Rectangle, id: u64) {
        let bounds = self.rect_at(r);
        self.cmds.push(DrawCommand::Image { bounds, id });
    }
    /// Run `f` with drawing clipped to `r`.
    pub fn clip(&mut self, r: Rectangle, f: impl FnOnce(&mut Frame)) {
        let bounds = self.rect_at(r);
        self.cmds.push(DrawCommand::PushClip(bounds));
        f(self);
        self.cmds.push(DrawCommand::PopClip);
    }
}

/// What the pointer did over a [`Canvas`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pointer {
    /// Left button went down at this canvas-local position.
    Down(Point),
    /// The pointer moved (while over the canvas, or while a press that started
    /// on it is held).
    Move(Point),
    /// Left button released.
    Up(Point),
}

type Painter = Rc<dyn Fn(&mut Frame)>;
type Handler<M> = Rc<dyn Fn(Pointer) -> Option<M>>;

/// A rectangle the app paints with a closure. See the canvas module docs.
pub struct Canvas<M> {
    ui: Ui,
    id: u64,
    width: Length,
    height: Length,
    paint: Painter,
    on_pointer: Option<Handler<M>>,
}

impl<M: 'static> Canvas<M> {
    /// A canvas of the given size painted by `paint`.
    pub fn new(
        ui: &Ui,
        id: u64,
        width: Length,
        height: Length,
        paint: impl Fn(&mut Frame) + 'static,
    ) -> Self {
        Self {
            ui: ui.clone(),
            id,
            width,
            height,
            paint: Rc::new(paint),
            on_pointer: None,
        }
    }
    /// Turn pointer input into messages. Return `None` to ignore an event.
    pub fn on_pointer(mut self, f: impl Fn(Pointer) -> Option<M> + 'static) -> Self {
        self.on_pointer = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
    fn captured(&self) -> bool {
        self.ui.slot(self.id ^ 0xCA11_5E00).is_some()
    }
}

impl<M: 'static> Widget<M> for Canvas<M> {
    fn style(&self) -> Style {
        util::sized(self.width, self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(
            match self.width {
                Length::Fixed(w) => w,
                _ => 0.0,
            },
            match self.height {
                Length::Fixed(h) => h,
                _ => 0.0,
            },
        )
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let Some(handler) = self.on_pointer.clone() else {
            return false;
        };
        let b = layout.bounds;
        let local = |p: Point| pt(p.x - b.origin.x, p.y - b.origin.y);
        let key = self.id ^ 0xCA11_5E00;
        let msg = match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_slot(key, Some(1));
                handler(Pointer::Down(local(cursor)))
            }
            Event::CursorMoved(p) if self.captured() || b.contains(*p) => {
                handler(Pointer::Move(local(*p)))
            }
            Event::MouseReleased(MouseButton::Left) if self.captured() => {
                self.ui.set_slot(key, None);
                handler(Pointer::Up(local(cursor)))
            }
            _ => return false,
        };
        let consumed = msg.is_some() || self.captured();
        out.extend(msg);
        consumed
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let mut f = Frame::new(&self.ui, b.origin, b.size);
        (self.paint)(&mut f);
        out.push(DrawCommand::PushClip(b));
        out.0.extend(f.finish());
        out.push(DrawCommand::PopClip);
    }
}
