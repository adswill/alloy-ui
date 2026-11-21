//! Zoom and pan over the x axis of a chart: a window of `len` samples starting at `start`.
//!
//! The window lives in the [`Ui`] (keyed by the chart's id) so it survives view rebuilds.
//! Scroll zooms about the cursor, dragging pans, `0`, Home or Esc (or the "Reset zoom" chip)
//! restores the full range. The chart slices its data to the window and draws as usual, so
//! the y axis rescales to what is visible.

use crate::ui::Ui;
use alloy_ui_core::{
    Border, DrawCommand, DrawList, Event, FontFamily, Key, MouseButton, Point, Rectangle, Size,
};

const K_START: u64 = 0x5A00_0001; // thousandths of a sample
const K_LEN: u64 = 0x5A00_0002;
const K_PX: u64 = 0x5A00_0003; // plot left edge, 1/16 px
const K_PW: u64 = 0x5A00_0004; // plot width, 1/16 px
const K_DRAG: u64 = 0x5A00_0005;
const K_LAST: u64 = 0x5A00_0006;

/// Fewest samples a zoomed window may hold.
pub const MIN_WINDOW: f32 = 6.0;

pub(crate) struct Zoom {
    ui: Ui,
    id: u64,
    n: usize,
}

impl Zoom {
    pub(crate) fn new(ui: &Ui, id: u64, n: usize) -> Self {
        Self {
            ui: ui.clone(),
            id,
            n,
        }
    }
    fn slot(&self, k: u64) -> Option<i64> {
        self.ui.slot(self.id ^ k)
    }
    fn set(&self, k: u64, v: Option<i64>) {
        self.ui.set_slot(self.id ^ k, v);
    }
    fn min_len(&self) -> f32 {
        MIN_WINDOW.min(self.n as f32).max(1.0)
    }
    /// The visible window as floats, clamped to the data.
    fn raw(&self) -> (f32, f32) {
        let n = self.n as f32;
        let len = self
            .slot(K_LEN)
            .map_or(n, |v| v as f32 / 1000.0)
            .clamp(self.min_len(), n.max(1.0));
        let start = self
            .slot(K_START)
            .map_or(0.0, |v| v as f32 / 1000.0)
            .clamp(0.0, (n - len).max(0.0));
        (start, len)
    }
    /// `(first sample, sample count)` currently shown.
    pub(crate) fn window(&self) -> (usize, usize) {
        let (s, l) = self.raw();
        let start = s.round() as usize;
        let len = (l.round() as usize).clamp(1, self.n.max(1));
        (start.min(self.n.saturating_sub(len)), len)
    }
    pub(crate) fn zoomed(&self) -> bool {
        self.window() != (0, self.n)
    }
    fn set_window(&self, start: f32, len: f32) {
        let n = self.n as f32;
        let len = len.clamp(self.min_len(), n.max(1.0));
        let start = start.clamp(0.0, (n - len).max(0.0));
        self.set(K_START, Some((start * 1000.0) as i64));
        self.set(K_LEN, Some((len * 1000.0) as i64));
    }
    pub(crate) fn reset(&self) {
        self.set(K_START, None);
        self.set(K_LEN, None);
    }
    /// Remember where the plot area is, for hit-testing the next events.
    pub(crate) fn record_plot(&self, x: f32, w: f32) {
        self.set(K_PX, Some((x * 16.0) as i64));
        self.set(K_PW, Some((w * 16.0) as i64));
    }
    fn plot_x(&self, b: &Rectangle) -> (f32, f32) {
        (
            self.slot(K_PX).map_or(b.origin.x, |v| v as f32 / 16.0),
            self.slot(K_PW)
                .map_or(b.size.width, |v| v as f32 / 16.0)
                .max(1.0),
        )
    }
    /// Where the "Reset zoom" chip sits.
    fn chip(b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x + b.size.width - 86.0, b.origin.y),
            Size::new(86.0, 22.0),
        )
    }
    /// Zoom by `factor` (above 1 zooms in) keeping the sample under `x` fixed.
    pub(crate) fn zoom_at(&self, b: &Rectangle, x: f32, factor: f32) -> bool {
        let (px, pw) = self.plot_x(b);
        let (s, l) = self.raw();
        let f = ((x - px) / pw).clamp(0.0, 1.0);
        let l2 = (l / factor).clamp(self.min_len(), self.n as f32);
        if (l2 - l).abs() < 1e-3 {
            return false;
        }
        self.set_window(s + f * l - f * l2, l2);
        true
    }
    /// Slide the window by `dx` pixels of drag.
    pub(crate) fn pan_by(&self, b: &Rectangle, dx: f32) -> bool {
        let (_, pw) = self.plot_x(b);
        let (s, l) = self.raw();
        let s2 = (s - dx / pw * l).clamp(0.0, (self.n as f32 - l).max(0.0));
        if (s2 - s).abs() < 1e-4 {
            return false;
        }
        self.set_window(s2, l);
        true
    }

    /// Feed an event. `focused` gates the keys.
    pub(crate) fn on_event(
        &self,
        event: &Event,
        b: &Rectangle,
        cursor: Point,
        focused: bool,
    ) -> bool {
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                if self.zoomed() && Self::chip(b).contains(cursor) {
                    self.reset();
                    return true;
                }
                self.ui.set_focus(Some(self.id));
                self.set(K_DRAG, Some(1));
                self.set(K_LAST, Some((cursor.x * 16.0) as i64));
                true
            }
            Event::CursorMoved(p) if self.slot(K_DRAG).is_some() => {
                let last = self.slot(K_LAST).map_or(p.x, |v| v as f32 / 16.0);
                self.set(K_LAST, Some((p.x * 16.0) as i64));
                self.pan_by(b, p.x - last);
                true
            }
            Event::MouseReleased(MouseButton::Left) if self.slot(K_DRAG).is_some() => {
                self.set(K_DRAG, None);
                self.set(K_LAST, None);
                true
            }
            Event::Scrolled { dy, .. } if b.contains(cursor) => {
                self.zoom_at(b, cursor.x, 1.0 + dy * 0.0015)
            }
            Event::KeyPressed(key, _) if focused => match key {
                Key::Character(c) if c == "0" => {
                    self.reset();
                    true
                }
                Key::Named(n) if n == "Home" || n == "Escape" => {
                    self.reset();
                    true
                }
                Key::Character(c) if c == "+" || c == "=" => {
                    let (px, pw) = self.plot_x(b);
                    self.zoom_at(b, px + pw / 2.0, 1.25)
                }
                Key::Character(c) if c == "-" || c == "_" => {
                    let (px, pw) = self.plot_x(b);
                    self.zoom_at(b, px + pw / 2.0, 0.8)
                }
                Key::Named(n) if n == "ArrowLeft" => self.pan_by(b, 24.0),
                Key::Named(n) if n == "ArrowRight" => self.pan_by(b, -24.0),
                _ => false,
            },
            _ => false,
        }
    }

    /// Draw the "Reset zoom" chip while zoomed.
    pub(crate) fn draw_chip(&self, b: &Rectangle, out: &mut DrawList) {
        if !self.zoomed() {
            return;
        }
        let t = self.ui.theme();
        let p = &t.palette;
        let r = Self::chip(b);
        let hot = r.contains(self.ui.cursor());
        out.push(DrawCommand::Quad {
            bounds: r,
            fill: if hot { p.hover } else { p.surface_raised },
            radius: 11.0,
            border: Some(Border {
                color: p.border_strong,
                width: 1.0,
            }),
            shadow: None,
        });
        let cap = t.typography.caption;
        let label = "Reset zoom";
        let w = self
            .ui
            .metrics()
            .width_styled(label, cap.size, 500, FontFamily::Sans);
        out.push(DrawCommand::Text {
            origin: Point::new(
                r.origin.x + (r.size.width - w) / 2.0,
                r.origin.y + (r.size.height - cap.line_height) / 2.0,
            ),
            content: label.into(),
            size: cap.size,
            color: p.text_secondary,
            max_width: None,
            weight: 500,
            family: FontFamily::Sans,
            line_height: Some(cap.line_height),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_ui_theme::Theme;

    fn b() -> Rectangle {
        Rectangle::new(Point::new(0.0, 0.0), Size::new(400.0, 200.0))
    }

    #[test]
    fn zoom_keeps_the_sample_under_the_cursor() {
        let ui = Ui::new(Theme::light());
        let z = Zoom::new(&ui, 1, 100);
        z.record_plot(0.0, 400.0);
        // Cursor a quarter of the way in: sample 25 of 100.
        assert!(z.zoom_at(&b(), 100.0, 2.0));
        let (s, l) = z.window();
        assert_eq!(l, 50);
        // Sample 25 still sits a quarter of the way across the window.
        assert!((s as f32 + 0.25 * l as f32 - 25.0).abs() <= 1.0, "{s} {l}");
        assert!(z.zoomed());
    }

    #[test]
    fn pan_is_clamped_and_reset_restores_everything() {
        let ui = Ui::new(Theme::light());
        let z = Zoom::new(&ui, 2, 100);
        z.record_plot(0.0, 400.0);
        z.zoom_at(&b(), 200.0, 4.0);
        assert_eq!(z.window().1, 25);
        z.pan_by(&b(), 1.0e6);
        assert_eq!(z.window().0, 0);
        z.pan_by(&b(), -1.0e6);
        assert_eq!(z.window(), (75, 25));
        z.reset();
        assert_eq!(z.window(), (0, 100));
        assert!(!z.zoomed());
    }

    #[test]
    fn cannot_zoom_below_the_minimum_or_above_everything() {
        let ui = Ui::new(Theme::light());
        let z = Zoom::new(&ui, 3, 100);
        z.record_plot(0.0, 400.0);
        assert!(!z.zoom_at(&b(), 10.0, 0.5), "already fully zoomed out");
        for _ in 0..20 {
            z.zoom_at(&b(), 10.0, 2.0);
        }
        assert_eq!(z.window().1, MIN_WINDOW as usize);
    }
}
