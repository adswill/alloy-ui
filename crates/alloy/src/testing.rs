//! Headless testing: drive an [`App`] through the real runtime without a window.
//!
//! ```ignore
//! let mut ui = Harness::new(Counter::default(), 360.0, 180.0);
//! ui.click("Add").click("Add");
//! assert!(ui.has_text("2"));
//! ui.save_png("counter.png");
//! ```

use crate::app::{App, Shell};
use alloy_ui_core::{
    runtime::Runtime, DrawCommand, Event, Key, Modifiers, MouseButton, Point, Size,
};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer, TextMeasurer};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::{TextMetrics, Ui};
use std::rc::Rc;

struct Metrics(TextMeasurer);
impl TextMetrics for Metrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.0.measure(text, size, 400, None).width
    }
}

/// A running app with scripted input and software rendering.
pub struct Harness<A: App> {
    rt: Runtime<Shell<A>>,
    size: (f32, f32),
    now: u64,
}

impl<A: App> Harness<A> {
    /// Start `app` in a `width` x `height` window with the light theme and real text metrics.
    pub fn new(app: A, width: f32, height: f32) -> Self {
        Self::themed(app, width, height, Theme::light())
    }
    /// Like [`new`](Self::new) with an explicit theme.
    pub fn themed(app: A, width: f32, height: f32, theme: Theme) -> Self {
        let ui = Ui::new(theme);
        ui.set_metrics(Rc::new(Metrics(TextMeasurer::new())));
        let rt = Runtime::new(
            Shell::new(app, ui),
            Size::new(width, height),
            crate::layout_root::<A::Msg>,
        );
        Self {
            rt,
            size: (width, height),
            now: 1000,
        }
    }
    fn texts(&self) -> Vec<(Point, String)> {
        self.rt
            .draw()
            .0
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text {
                    origin, content, ..
                } => Some((*origin, content.clone())),
                _ => None,
            })
            .collect()
    }
    /// True if some text on screen equals `text`.
    pub fn has_text(&self, text: &str) -> bool {
        self.texts().iter().any(|(_, t)| t == text)
    }
    /// Where the first text equal to `text` is drawn.
    pub fn position(&self, text: &str) -> Option<Point> {
        self.texts()
            .into_iter()
            .find(|(_, t)| t == text)
            .map(|(p, _)| p)
    }
    /// Click the first widget showing exactly `text`. Panics if there is none.
    pub fn click(&mut self, text: &str) -> &mut Self {
        let (p, _) = self
            .texts()
            .into_iter()
            .find(|(_, t)| t == text)
            .unwrap_or_else(|| panic!("no widget with text {text:?}"));
        let p = Point::new(p.x + 3.0, p.y + 3.0);
        self.rt.handle_event(&Event::CursorMoved(p));
        self.rt
            .handle_event(&Event::MousePressed(MouseButton::Left));
        self.rt
            .handle_event(&Event::MouseReleased(MouseButton::Left));
        self
    }
    /// Right-click the first widget showing exactly `text` (opens context menus). Panics if there is none.
    pub fn right_click(&mut self, text: &str) -> &mut Self {
        self.hover(text);
        self.rt
            .handle_event(&Event::MousePressed(MouseButton::Right));
        self.rt
            .handle_event(&Event::MouseReleased(MouseButton::Right));
        self
    }
    /// Move the pointer onto the first widget showing exactly `text`. Panics if there is none.
    pub fn hover(&mut self, text: &str) -> &mut Self {
        let (p, _) = self
            .texts()
            .into_iter()
            .find(|(_, t)| t == text)
            .unwrap_or_else(|| panic!("no widget with text {text:?}"));
        self.rt
            .handle_event(&Event::CursorMoved(Point::new(p.x + 3.0, p.y + 3.0)));
        self
    }
    /// Wait (up to 5 s) for background [`resource`](crate::resource)s to finish and show their data.
    pub fn settle(&mut self) -> &mut Self {
        for _ in 0..200 {
            crate::resource::pump();
            self.rt.dispatch_all(Vec::new());
            if crate::resource::idle() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        self
    }
    /// Type text into the focused widget.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        self.rt.handle_event(&Event::TextInput(text.to_string()));
        self
    }
    /// Press a key.
    pub fn press(&mut self, key: Key) -> &mut Self {
        self.rt
            .handle_event(&Event::KeyPressed(key, Modifiers::default()));
        self
    }
    /// Press a named key such as `"ArrowDown"`, `"PageDown"`, `"Enter"` or `"Escape"`.
    pub fn key(&mut self, name: &str) -> &mut Self {
        self.press(match name {
            "Enter" => Key::Enter,
            "Escape" => Key::Escape,
            "Tab" => Key::Tab,
            n => Key::Named(n.into()),
        })
    }
    /// Press a shortcut such as `"mod+s"` or `"mod+shift+z"`.
    pub fn chord(&mut self, keys: &str) -> &mut Self {
        let sc = alloy_ui_platform::shortcut::Shortcut::parse(keys)
            .unwrap_or_else(|| panic!("bad shortcut {keys:?}"));
        let key = match sc.key.as_str() {
            "enter" | "Enter" => Key::Enter,
            k => Key::Character(k.into()),
        };
        self.rt.handle_event(&Event::KeyPressed(key, sc.mods));
        self
    }
    /// Let `ms` milliseconds pass: animations and toasts progress.
    pub fn advance(&mut self, ms: u64) -> &mut Self {
        self.now += ms;
        self.rt.handle_event(&Event::Tick(self.now));
        self
    }
    /// The current frame's draw commands, for renderers and benchmarks.
    pub fn draw_list(&self) -> alloy_ui_core::DrawList {
        self.rt.draw()
    }
    /// Render the current frame at 2x and encode it as PNG.
    pub fn png(&self) -> Vec<u8> {
        let (w, h) = (self.size.0 as u32 * 2, self.size.1 as u32 * 2);
        CpuRenderer::new(w, h)
            .render_to_image(&self.rt.draw(), w, h, 2.0)
            .encode_png()
            .expect("png encode")
    }
    /// Write [`png`](Self::png) to `path`.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) {
        std::fs::write(path, self.png()).expect("write png");
    }
}
