//! A rectangle that shows a texture your own wgpu code renders into.
//!
//! Get the renderer's device with `alloy_ui_render::shared_gpu()`, make a target with
//! `SharedGpu::create_target`, draw your scene into it every frame, and show it with
//! [`GpuView`]. The window renderer samples the texture directly: there is no readback
//! and no copy. Pointer input over the view arrives as [`GpuInput`] messages with
//! coordinates relative to the view, so a scene can orbit, pick and zoom.
//!
//! The view knows nothing about wgpu (this crate does not depend on the renderer): it only
//! holds the texture id. Resize the texture from the size it reports through a
//! [`SizeProbe`].

use crate::util;
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Layout, MouseButton, Point, Size, Widget,
};
use alloy_ui_layout::Length;
use std::{cell::Cell, rc::Rc};

/// Pointer input over a [`GpuView`], relative to its top-left corner in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuInput {
    /// Left button went down at this point.
    Press(Point),
    /// The cursor moved while the left button is held.
    Drag {
        /// Where the cursor is now.
        at: Point,
        /// Movement since the last event.
        delta: Point,
    },
    /// Left button released.
    Release(Point),
    /// Scroll wheel or trackpad over the view (`dy` positive scrolls down).
    Scroll {
        /// Where the cursor is.
        at: Point,
        /// Vertical scroll amount in logical pixels.
        dy: f32,
    },
}

/// Receives the logical size the view was last laid out at, so the app can size its texture.
#[derive(Debug, Clone, Default)]
pub struct SizeProbe(Rc<Cell<Size>>);

impl SizeProbe {
    /// A probe that reads zero until the view is first drawn.
    pub fn new() -> Self {
        Self::default()
    }
    /// The last laid-out logical size.
    pub fn get(&self) -> Size {
        self.0.get()
    }
}

/// Shows the shared texture `texture` (an id from `SharedGpu::publish`/`create_target`).
pub struct GpuView<M> {
    texture: u64,
    width: Length,
    height: Length,
    live: bool,
    probe: Option<SizeProbe>,
    paint: Option<Rc<dyn Fn(Size)>>,
    on_input: Option<Rc<dyn Fn(GpuInput) -> M>>,
    dragging: bool,
    last: Point,
}

impl<M: 'static> GpuView<M> {
    /// Show the texture `texture`, filling the width and 280 px tall by default.
    pub fn new(texture: u64) -> Self {
        Self {
            texture,
            width: Length::Fill(1),
            height: Length::Fixed(280.0),
            live: false,
            probe: None,
            paint: None,
            on_input: None,
            dragging: false,
            last: Point::new(0.0, 0.0),
        }
    }
    /// Width (default fill).
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Height (default 280 px).
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// Keep the window redrawing every frame, for a scene that animates itself. Leave it off
    /// for a scene that only changes on input so idle CPU stays near zero.
    pub fn live(mut self, on: bool) -> Self {
        self.live = on;
        self
    }
    /// Report the laid-out size through `probe`.
    pub fn probe(mut self, probe: &SizeProbe) -> Self {
        self.probe = Some(probe.clone());
        self
    }
    /// Called every time the view is drawn, just before its texture is sampled, with the
    /// laid-out logical size: render your scene here. Pair it with [`GpuView::live`] for
    /// an animation, or leave `live` off and the scene renders only when something else redraws.
    pub fn paint(mut self, f: impl Fn(Size) + 'static) -> Self {
        self.paint = Some(Rc::new(f));
        self
    }
    /// Turn pointer input over the view into messages.
    pub fn on_input(mut self, f: impl Fn(GpuInput) -> M + 'static) -> Self {
        self.on_input = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for GpuView<M> {
    fn style(&self) -> Style {
        util::sized(self.width, self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        let f = |l: Length| match l {
            Length::Fixed(v) => v,
            _ => 240.0,
        };
        Size::new(f(self.width), f(self.height))
    }
    fn animating(&self) -> bool {
        self.live
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let Some(f) = self.on_input.clone() else {
            return false;
        };
        let b = layout.bounds;
        let rel = |p: Point| Point::new(p.x - b.origin.x, p.y - b.origin.y);
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.dragging = true;
                self.last = cursor;
                out.push(f(GpuInput::Press(rel(cursor))));
                true
            }
            Event::CursorMoved(p) if self.dragging => {
                let delta = Point::new(p.x - self.last.x, p.y - self.last.y);
                self.last = *p;
                out.push(f(GpuInput::Drag { at: rel(*p), delta }));
                true
            }
            Event::MouseReleased(MouseButton::Left) if self.dragging => {
                self.dragging = false;
                out.push(f(GpuInput::Release(rel(cursor))));
                true
            }
            Event::Scrolled { dy, .. } if b.contains(cursor) => {
                out.push(f(GpuInput::Scroll {
                    at: rel(cursor),
                    dy: *dy,
                }));
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        if let Some(p) = &self.probe {
            p.0.set(layout.bounds.size);
        }
        if let Some(f) = &self.paint {
            f(layout.bounds.size);
        }
        out.push(DrawCommand::Image {
            bounds: layout.bounds,
            id: self.texture,
        });
    }
}
