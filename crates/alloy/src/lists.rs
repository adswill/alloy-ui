//! Lists and layout helpers: a virtualised grid and a carousel.
//!
//! ```ignore
//! vgrid(10_000, 4, |i| text(format!("Game {i}")).pad(8.0)).aspect(1.5).act(|s: &mut S, i| s.open = i)
//! carousel([hero("Portal"), hero("Half-Life"), hero("Dota")]).auto(secs(5.0))
//! ```

use crate::{
    cx::{self, auto_id},
    local,
    view::Node,
    Act,
};
use alloy_ui_core::Element;
use alloy_ui_widgets as w;
use std::{rc::Rc, sync::Arc, time::Duration};

// ---------------------------------------------------------------- vgrid

type Run2<S> = Arc<dyn Fn(&mut S, usize) + Send + Sync>;

/// A virtualised grid under construction. See [`vgrid`].
pub struct VGrid<S> {
    count: usize,
    columns: usize,
    build: Rc<dyn Fn(usize) -> Element<Act<S>>>,
    fit: Option<f32>,
    cell_h: Option<f32>,
    aspect: Option<f32>,
    gap: Option<f32>,
    selected: Option<Option<usize>>,
    on: Option<Run2<S>>,
    id: u64,
}

/// A grid of `count` cells in `columns` columns that only builds what is on screen, so 100 000
/// tiles scroll as smoothly as 20: [`vlist`](crate::vlist) in two dimensions. `cell(i)` builds
/// cell `i`; it fills the cell's rectangle (set the size with `.cell_height(px)`, or
/// `.aspect(ratio)` to derive it from the column width). Cells keep their own state by index,
/// so a cell that scrolls away and back is not mistaken for another. Click, the arrow keys in
/// two dimensions, Home/End, PageUp/PageDown, the wheel and a draggable scrollbar all work.
/// It remembers the selected cell itself; react with `.act(|s, i| ..)` or pin it with
/// `.selected(..)`.
#[track_caller]
pub fn vgrid<S: 'static, N: Into<Node<Act<S>>>>(
    count: usize,
    columns: usize,
    cell: impl Fn(usize) -> N + 'static,
) -> VGrid<S> {
    let ui = cx::ui();
    VGrid {
        count,
        columns,
        build: Rc::new(move |i| cx::scoped(&ui, i as u64, || cell(i).into().into_element())),
        fit: None,
        cell_h: None,
        aspect: None,
        gap: None,
        selected: None,
        on: None,
        id: auto_id(560),
    }
}

impl<S: 'static> VGrid<S> {
    /// As many columns as fit, each at least `min_width` pixels wide (overrides `columns`).
    pub fn fit(mut self, min_width: f32) -> Self {
        self.fit = Some(min_width);
        self
    }
    /// Cell height in pixels.
    pub fn cell_height(mut self, px: f32) -> Self {
        self.cell_h = Some(px);
        self
    }
    /// Cell height as column width divided by `ratio` (`1.0` square, `1.5` landscape).
    pub fn aspect(mut self, ratio: f32) -> Self {
        self.aspect = Some(ratio);
        self
    }
    /// Space between cells.
    pub fn gap(mut self, px: f32) -> Self {
        self.gap = Some(px);
        self
    }
    /// Control the selection yourself.
    pub fn selected(mut self, i: Option<usize>) -> Self {
        self.selected = Some(i);
        self
    }
    /// Called with the index of a picked cell.
    pub fn act(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<VGrid<S>> for Node<Act<S>> {
    fn from(v: VGrid<S>) -> Self {
        let picked = local(None::<usize>);
        let sel = v.selected.unwrap_or_else(|| picked.get());
        let on = v.on;
        let build = v.build;
        let mut e = w::VirtualGrid::new(&cx::ui(), v.id, v.count, v.columns, move |i| build(i))
            .selected(sel)
            .on_select(move |i| {
                let on = on.clone();
                Act::new(move |s: &mut S| {
                    picked.set(Some(i));
                    if let Some(f) = &on {
                        f(s, i);
                    }
                })
            });
        if let Some(m) = v.fit {
            e = e.fit(m);
        }
        if let Some(h) = v.cell_h {
            e = e.cell_height(h);
        }
        if let Some(r) = v.aspect {
            e = e.aspect(r);
        }
        if let Some(g) = v.gap {
            e = e.gap(g);
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- carousel

type Run1<S> = Arc<dyn Fn(&mut S, usize) + Send + Sync>;

/// A carousel under construction. See [`carousel`].
pub struct Carousel<M> {
    slides: Vec<Element<M>>,
    index: Option<usize>,
    auto: Option<Duration>,
    fade: Option<u32>,
    height: Option<f32>,
    arrows: bool,
    dots: bool,
    on: Option<Box<dyn Fn(usize) -> M>>,
    id: u64,
}

/// Slides shown one at a time with a crossfade, arrows and dots. It remembers the current slide
/// itself; `.auto(secs(5.0))` advances on a timer (paused while the pointer is over it),
/// `.index(i)` pins it to your state and `.act(|s, i| ..)` hears about changes. The Left and
/// Right keys work when it has focus. Every slide is built each frame, so keep them light (an
/// image and a caption).
#[track_caller]
pub fn carousel<M: 'static, N: Into<Node<M>>>(slides: impl IntoIterator<Item = N>) -> Carousel<M> {
    Carousel {
        slides: slides
            .into_iter()
            .map(|n| n.into().into_element())
            .collect(),
        index: None,
        auto: None,
        fade: None,
        height: None,
        arrows: true,
        dots: true,
        on: None,
        id: auto_id(561),
    }
}

impl<M: 'static> Carousel<M> {
    /// Advance to the next slide every `every` (wrapping round), except while hovered.
    pub fn auto(mut self, every: Duration) -> Self {
        self.auto = Some(every);
        self
    }
    /// Show slide `i`. Pair it with [`on`](Self::on) or [`act`](Carousel::act) to keep your own
    /// position.
    pub fn index(mut self, i: usize) -> Self {
        self.index = Some(i);
        self
    }
    /// Crossfade length in milliseconds.
    pub fn fade(mut self, ms: u32) -> Self {
        self.fade = Some(ms);
        self
    }
    /// A fixed height in pixels (by default the tallest slide, at least 96).
    pub fn height(mut self, px: f32) -> Self {
        self.height = Some(px);
        self
    }
    /// Hide the previous/next arrows.
    pub fn no_arrows(mut self) -> Self {
        self.arrows = false;
        self
    }
    /// Hide the dots.
    pub fn no_dots(mut self) -> Self {
        self.dots = false;
        self
    }
    /// Message sent with the new slide index whenever it changes.
    pub fn on(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
}

impl<S: 'static> Carousel<Act<S>> {
    /// Run `f(state, slide)` whenever the slide changes.
    pub fn act(self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        let f: Run1<S> = Arc::new(f);
        self.on(move |i| {
            let f = f.clone();
            Act::new(move |s: &mut S| f(s, i))
        })
    }
}

impl<M: 'static> From<Carousel<M>> for Node<M> {
    fn from(c: Carousel<M>) -> Self {
        let mut e = w::Carousel::new(&cx::ui(), c.id, c.slides)
            .arrows(c.arrows)
            .dots(c.dots);
        if let Some(i) = c.index {
            e = e.index(i);
        }
        if let Some(d) = c.auto {
            e = e.auto(d.as_millis() as u64);
        }
        if let Some(h) = c.height {
            e = e.height(h);
        }
        if let Some(ms) = c.fade {
            e = e.fade(ms);
        }
        if let Some(f) = c.on {
            e = e.on_change(f);
        }
        Node(e.into_element())
    }
}
