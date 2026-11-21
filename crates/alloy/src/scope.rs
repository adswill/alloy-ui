//! RF display widgets: a streaming [`Waterfall`] and a [`spectrum`] trace.
//!
//! Both are built for live data at display rate. The waterfall keeps its pixels in a
//! [`StreamTexture`] ring and uploads only the rows added since the last frame; the spectrum
//! draws one `Polyline` batch however many points it has.

use crate::{
    cx::{self, auto_id},
    view::Node,
    Act,
};
use alloy_ui_core::{Color, Point, Rectangle, Size};
use alloy_ui_layout::Length;
use alloy_ui_render::StreamTexture;
use alloy_ui_widgets as w;
use std::{cell::Cell, rc::Rc};

/// Colour ramp from a 0..=255 level: black, blue, cyan, yellow, red, white.
fn ramp(level: u8) -> [u8; 4] {
    const STOPS: [(f32, [f32; 3]); 6] = [
        (0.0, [0.0, 0.0, 0.0]),
        (0.2, [0.0, 0.0, 0.7]),
        (0.45, [0.0, 0.8, 0.9]),
        (0.7, [1.0, 0.9, 0.0]),
        (0.9, [1.0, 0.1, 0.0]),
        (1.0, [1.0, 1.0, 1.0]),
    ];
    let t = f32::from(level) / 255.0;
    let i = STOPS.iter().position(|s| t <= s.0).unwrap_or(5).max(1);
    let (a, b) = (STOPS[i - 1], STOPS[i]);
    let k = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    let c = |n: usize| ((a.1[n] + (b.1[n] - a.1[n]) * k) * 255.0).round() as u8;
    [c(0), c(1), c(2), 255]
}

struct WfInner {
    tex: StreamTexture,
    /// Texture row the next line is written to.
    head: Cell<u32>,
    lut: Vec<[u8; 4]>,
    range: Cell<(f32, f32)>,
}

/// A scrolling waterfall of power rows (dB), newest at the top. Cheap to clone: clones share
/// the same buffer, so keep one in your state, [`push_row`](Waterfall::push_row) from your
/// data source and show it with [`view`](Waterfall::view).
#[derive(Clone)]
pub struct Waterfall(Rc<WfInner>);

impl Waterfall {
    /// A waterfall `cols` bins wide keeping `rows` lines of history.
    pub fn new(cols: u32, rows: u32) -> Self {
        Self(Rc::new(WfInner {
            tex: StreamTexture::new(cols, rows),
            head: Cell::new(0),
            lut: (0..=255u8).map(ramp).collect(),
            range: Cell::new((-110.0, -30.0)),
        }))
    }
    /// The dB values shown as black and white (default -110 and -30).
    pub fn range(&self, min_db: f32, max_db: f32) {
        self.0.range.set((min_db, max_db.max(min_db + 1.0)));
    }
    /// Add one line of dB values, newest on top. Any length: wider lines are reduced to the
    /// texture width by taking the strongest value of each group.
    pub fn push_row(&self, db: &[f32]) {
        let inner = &self.0;
        let (cols, rows) = (inner.tex.width() as usize, inner.tex.height());
        let (lo, hi) = inner.range.get();
        let scale = 255.0 / (hi - lo);
        let mut px = Vec::with_capacity(cols * 4);
        for c in 0..cols {
            let (a, b) = (
                c * db.len() / cols,
                ((c + 1) * db.len() / cols).max(c * db.len() / cols + 1),
            );
            let v = db[a.min(db.len().saturating_sub(1))..b.min(db.len())]
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            let v = if v.is_finite() { v } else { lo };
            px.extend_from_slice(&inner.lut[((v - lo) * scale).clamp(0.0, 255.0) as usize]);
        }
        let head = inner.head.get();
        inner.tex.write(0, head, cols as u32, 1, &px);
        inner.head.set((head + 1) % rows);
    }
    /// The waterfall as a view of `height` pixels, as wide as its container. It repaints every
    /// frame while on screen.
    #[track_caller]
    pub fn view<S: 'static>(&self, height: f32) -> Node<Act<S>> {
        let inner = self.0.clone();
        let e = w::Canvas::<Act<S>>::new(
            &cx::ui(),
            auto_id(70),
            Length::Fill(1),
            Length::Fixed(height),
            move |f| {
                let Size { width, height } = f.size();
                let rows = inner.tex.height() as f32;
                let split = inner.head.get() as f32 / rows;
                let id = inner.tex.id();
                let top = height * split;
                // Newest rows (0..head, drawn upside down so the newest is first), then the older
                // ones that wrapped round.
                if split > 0.0 {
                    f.stream_image(
                        Rectangle::new(Point::new(0.0, 0.0), Size::new(width, top)),
                        id,
                        [0.0, split, 1.0, 0.0],
                    );
                }
                if split < 1.0 {
                    f.stream_image(
                        Rectangle::new(Point::new(0.0, top), Size::new(width, height - top)),
                        id,
                        [0.0, 1.0, 1.0, split],
                    );
                }
            },
        )
        .live();
        Node(e.into_element())
    }
}

/// A spectrum trace of dB values between `min_db` (bottom) and `max_db` (top), as wide as its
/// container and `height` pixels tall, with a 10 dB grid. Long traces are reduced to a
/// minimum and maximum per pixel column, so the cost follows the width, not the data size.
#[track_caller]
pub fn spectrum<S: 'static>(db: &[f32], min_db: f32, max_db: f32, height: f32) -> Node<Act<S>> {
    let data: Rc<Vec<f32>> = Rc::new(db.to_vec());
    let e = w::Canvas::<Act<S>>::new(
        &cx::ui(),
        auto_id(71),
        Length::Fill(1),
        Length::Fixed(height),
        move |f| {
            let Size { width, height } = f.size();
            let pal = f.palette();
            let y_of =
                |v: f32| height - ((v - min_db) / (max_db - min_db)).clamp(0.0, 1.0) * height;
            let grid = Color {
                a: 0.18,
                ..pal.text
            };
            let mut d = (min_db / 10.0).ceil() * 10.0;
            while d <= max_db {
                let y = y_of(d);
                f.line(Point::new(0.0, y), Point::new(width, y), 1.0, grid);
                d += 10.0;
            }
            let n = data.len();
            if n < 2 {
                return;
            }
            let cols = width.max(1.0) as usize;
            let mut pts: Vec<Point> = Vec::new();
            if n <= cols * 2 {
                let step = width / (n - 1) as f32;
                pts.extend(
                    data.iter()
                        .enumerate()
                        .map(|(i, v)| Point::new(i as f32 * step, y_of(*v))),
                );
            } else {
                for c in 0..cols {
                    let (a, b) = (c * n / cols, ((c + 1) * n / cols).min(n));
                    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                    for v in &data[a..b] {
                        lo = lo.min(*v);
                        hi = hi.max(*v);
                    }
                    let x = c as f32 + 0.5;
                    pts.push(Point::new(x, y_of(hi)));
                    pts.push(Point::new(x, y_of(lo)));
                }
            }
            f.polyline(&pts, 1.5, pal.accent);
        },
    );
    Node(e.into_element())
}
