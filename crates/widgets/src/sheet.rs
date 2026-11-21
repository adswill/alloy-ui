//! A draggable bottom sheet with snap points.

use crate::{
    motion::Spring,
    physics::{rubber_band, snap_target},
    ui::Ui,
    util,
};
use alloy_ui_core::{
    style::{Axis, Style},
    Border, Color, DrawCommand, DrawList, Element, Event, Layout, MouseButton, Point, Rectangle,
    Size, Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

/// Height of the strip at the top of the panel that takes hold of a drag.
const GRAB: f32 = 30.0;
const K_STATE: u64 = 0x5E01_0000_0000;
const K_TRACK: u64 = 0x5E02_0000_0000;
const K_SPRING: u64 = 0x5E03_0000_0000;
const K_SNAP: u64 = 0x5E04_0000_0000;
const SETTLE: Spring = Spring {
    stiffness: 260.0,
    damping: 32.25, // 2 * sqrt(260)
};

/// A panel that rises from the bottom of its area and rests at one of several
/// snap heights. Drag the handle (or flick it): it follows the pointer, resists
/// when pulled past the lowest or highest snap point, and on release carries its
/// velocity on to the snap point it is heading for, settling without bounce.
///
/// Put it over the page in a [`crate::Stack`]; it ignores pointer events that miss it.
pub struct Sheet<M> {
    ui: Ui,
    id: u64,
    snaps: Vec<f32>,
    initial: usize,
    scrim: bool,
    on_snap: Option<Rc<dyn Fn(usize) -> M>>,
    child: Vec<Element<M>>,
}

impl<M: 'static> Sheet<M> {
    /// A sheet with `content`. `snaps` are the visible heights as fractions of the
    /// area (`[0.15, 0.5, 0.92]`); `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, snaps: &[f32], content: Element<M>) -> Self {
        let mut snaps: Vec<f32> = snaps.iter().map(|f| f.clamp(0.0, 1.0)).collect();
        snaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if snaps.is_empty() {
            snaps.push(0.5);
        }
        Self {
            ui: ui.clone(),
            id,
            snaps,
            initial: 0,
            scrim: false,
            on_snap: None,
            child: vec![content],
        }
    }
    /// Which snap point (0 = lowest) it starts at.
    pub fn start_at(mut self, index: usize) -> Self {
        self.initial = index.min(self.snaps.len() - 1);
        self
    }
    /// Dim the area behind the sheet as it rises.
    pub fn scrim(mut self) -> Self {
        self.scrim = true;
        self
    }
    /// Called with the snap index each time the sheet settles on a new one.
    pub fn on_snap(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_snap = Some(Rc::new(f));
        self
    }
    /// As an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn snap_px(&self, area: f32) -> Vec<f32> {
        self.snaps.iter().map(|f| f * area).collect()
    }
    fn index(&self) -> usize {
        self.ui
            .slot(self.id ^ K_SNAP)
            .map_or(self.initial, |i| (i as usize).min(self.snaps.len() - 1))
    }
    /// Visible height of the panel right now.
    fn visible(&self, area: f32) -> f32 {
        let pts = self.snap_px(area);
        let (lo, hi) = (pts[0], pts[pts.len() - 1]);
        let m = self.ui.memo(self.id ^ K_STATE);
        if m[0] >= 1.0 {
            // Following the pointer; resist beyond the ends.
            let raw = m[3];
            return if raw > hi {
                hi + rubber_band(raw - hi, area * 0.5)
            } else if raw < lo {
                lo - rubber_band(lo - raw, area * 0.5)
            } else {
                raw
            };
        }
        self.ui
            .spring(self.id ^ K_SPRING, pts[self.index()], SETTLE)
    }
    fn panel(&self, b: &Rectangle) -> Rectangle {
        let pts = self.snap_px(b.size.height);
        let full = pts[pts.len() - 1].max(GRAB * 2.0);
        let vis = self.visible(b.size.height);
        Rectangle::new(
            Point::new(b.origin.x, b.origin.y + b.size.height - vis),
            Size::new(b.size.width, full),
        )
    }
    fn content_rect(&self, panel: &Rectangle) -> Rectangle {
        let pad = self.ui.theme().spacing.lg;
        Rectangle::new(
            Point::new(panel.origin.x + pad, panel.origin.y + GRAB),
            Size::new(
                (panel.size.width - 2.0 * pad).max(0.0),
                (panel.size.height - GRAB).max(0.0),
            ),
        )
    }
}

impl<M: 'static> Widget<M> for Sheet<M> {
    fn style(&self) -> Style {
        Style {
            axis: Axis::Vertical,
            width: Length::Fill(1),
            height: Length::Fill(1),
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(0.0, 0.0)
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        let panel = self.panel(&bounds);
        Layout {
            bounds,
            children: vec![self.child[0].0.arrange(self.content_rect(&panel))],
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        // A leaf for the layout engine (like `Stack`): the content is placed here.
        let layout = &self.arrange(b);
        let key = self.id ^ K_STATE;
        let tkey = self.id ^ K_TRACK;
        let m = self.ui.memo(key);
        let wall = self.ui.wall_ms();
        let panel = self.panel(&b);
        match event {
            Event::MousePressed(MouseButton::Left) if panel.contains(cursor) => {
                if cursor.y < panel.origin.y + GRAB {
                    let vis = self.visible(b.size.height);
                    self.ui.set_memo(key, [1.0, vis, cursor.y, vis]);
                    self.ui.set_memo(tkey, [wall as f32, cursor.y, 0.0, 0.0]);
                    return true;
                }
                util::dispatch(&mut self.child, event, layout, cursor, out);
                true
            }
            Event::CursorMoved(p) if m[0] >= 1.0 => {
                let raw = m[1] + (m[2] - p.y);
                self.ui.set_memo(key, [1.0, m[1], m[2], raw]);
                let t = self.ui.memo(tkey);
                let dt = (wall as f32 - t[0]).max(1.0) / 1000.0;
                let inst = (p.y - t[1]) / dt;
                let v = if t[3] > 0.0 {
                    0.6 * inst + 0.4 * t[2]
                } else {
                    inst
                };
                self.ui.set_memo(tkey, [wall as f32, p.y, v, 1.0]);
                true
            }
            Event::MouseReleased(MouseButton::Left) if m[0] >= 1.0 => {
                let vis = self.visible(b.size.height);
                let t = self.ui.memo(tkey);
                let quiet = wall as f32 - t[0] > 90.0;
                // Pointer moving up grows the visible height.
                let vel = if quiet { 0.0 } else { -t[2] };
                let pts = self.snap_px(b.size.height);
                let target = snap_target(vis, vel, &pts);
                let idx = pts.iter().position(|p| *p == target).unwrap_or(0);
                let changed = idx != self.index();
                self.ui.set_slot(self.id ^ K_SNAP, Some(idx as i64));
                self.ui.set_memo(key, [0.0; 4]);
                self.ui.spring_release(self.id ^ K_SPRING, vis, vel, SETTLE);
                if changed {
                    if let Some(f) = &self.on_snap {
                        out.push(f(idx));
                    }
                }
                true
            }
            _ if panel.contains(cursor) || m[0] >= 1.0 => {
                util::dispatch(&mut self.child, event, layout, cursor, out)
            }
            _ => false,
        }
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let layout = &self.arrange(b);
        let t = self.ui.theme();
        let pts = self.snap_px(b.size.height);
        let panel = self.panel(&b);
        let vis = self.visible(b.size.height);
        out.push(DrawCommand::PushClip(b));
        if self.scrim {
            let (lo, hi) = (pts[0], pts[pts.len() - 1]);
            let k = if hi > lo {
                ((vis - lo) / (hi - lo)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if k > 0.002 {
                out.push(DrawCommand::Quad {
                    bounds: b,
                    fill: Color::hex(0x000000).with_alpha(0.38 * k),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
        }
        out.push(DrawCommand::Quad {
            bounds: panel,
            fill: t.palette.surface_raised,
            radius: t.radii.large * 2.0,
            border: Some(Border {
                color: t.palette.border,
                width: 1.0,
            }),
            shadow: Some(t.elevation.high),
        });
        let held = self.ui.memo(self.id ^ K_STATE)[0] >= 1.0;
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(
                    panel.origin.x + panel.size.width * 0.5 - 18.0,
                    panel.origin.y + 9.0,
                ),
                Size::new(36.0, 4.0),
            ),
            fill: if held {
                t.palette.accent
            } else {
                t.palette.border_strong
            },
            radius: 2.0,
            border: None,
            shadow: None,
        });
        out.push(DrawCommand::PushClip(self.content_rect(&panel)));
        util::draw_children(&self.child, layout, out);
        out.push(DrawCommand::PopClip);
        out.push(DrawCommand::PopClip);
    }
}
