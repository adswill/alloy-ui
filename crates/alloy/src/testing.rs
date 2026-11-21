//! Headless testing: drive an [`App`] through the real runtime without a window.
//!
//! ```ignore
//! let mut ui = Harness::new(Counter::default(), 360.0, 180.0);
//! ui.click("Add").click("Add");
//! assert!(ui.has_text("2"));
//! ui.expect("2").expect_no("3");
//! ui.snapshot("counter_two"); // compares with tests/snapshots/counter_two.png
//! ```
//!
//! Everything is found by the text on screen, the way a person finds it, so tests survive
//! layout changes. Assertion failures list what *is* on screen. `UPDATE_GOLDEN=1 cargo test`
//! (re)writes snapshots.

use crate::app::{App, Shell};
use alloy_ui_core::{
    runtime::Runtime, Application, DrawCommand, Event, HostEvent, Key, Modifiers, MouseButton,
    Point, Size,
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
    fn width_styled(
        &self,
        text: &str,
        size: f32,
        weight: u16,
        family: alloy_ui_core::FontFamily,
    ) -> f32 {
        self.0
            .measure_styled(text, size, weight, family, None, None)
            .width
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
    fn placed(&self) -> Vec<(Point, String)> {
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
    /// Every piece of text on screen, in draw order.
    pub fn texts(&self) -> Vec<String> {
        self.placed().into_iter().map(|(_, t)| t).collect()
    }
    /// How many pieces of text on screen equal `text`.
    pub fn count(&self, text: &str) -> usize {
        self.placed().iter().filter(|(_, t)| t == text).count()
    }
    /// Panic, listing what is on screen, unless some text equals `text`.
    #[track_caller]
    pub fn expect(&mut self, text: &str) -> &mut Self {
        assert!(
            self.has_text(text),
            "expected {text:?} on screen, found {:?}",
            self.texts()
        );
        self
    }
    /// Panic unless some text contains `part`.
    #[track_caller]
    pub fn expect_containing(&mut self, part: &str) -> &mut Self {
        assert!(
            self.texts().iter().any(|t| t.contains(part)),
            "expected text containing {part:?} on screen, found {:?}",
            self.texts()
        );
        self
    }
    /// Panic if some text equals `text`.
    #[track_caller]
    pub fn expect_no(&mut self, text: &str) -> &mut Self {
        assert!(
            !self.has_text(text),
            "did not expect {text:?} on screen, found {:?}",
            self.texts()
        );
        self
    }
    /// [`settle`](Self::settle), then panic unless `text` is on screen: for data that arrives
    /// from a [`resource`](crate::resource).
    #[track_caller]
    pub fn wait_for(&mut self, text: &str) -> &mut Self {
        for _ in 0..200 {
            if self.has_text(text) {
                return self;
            }
            self.settle();
            self.advance(25);
        }
        self.expect(text)
    }
    /// Focus the field showing `target` (its label or placeholder), replace its contents with
    /// `value` and leave the focus there.
    pub fn fill(&mut self, target: &str, value: &str) -> &mut Self {
        self.click(target).chord("mod+a");
        self.type_text(value)
    }
    /// Scroll the wheel by `dy` pixels (positive moves content up) over the text `over`.
    pub fn scroll(&mut self, over: &str, dy: f32) -> &mut Self {
        self.hover(over);
        self.rt.handle_event(&Event::Scrolled { dx: 0.0, dy });
        self
    }
    /// Resize the window to `width` x `height` logical pixels.
    pub fn resize(&mut self, width: f32, height: f32) -> &mut Self {
        self.size = (width, height);
        self.rt
            .handle_event(&Event::Resized(Size::new(width, height)));
        self
    }
    /// Compare the current frame with `tests/snapshots/<name>.png` (relative to the crate being
    /// tested). Small anti-aliasing differences are tolerated, but fonts differ between systems,
    /// so keep snapshots for the platform CI runs on. Writes the file when `UPDATE_GOLDEN=1`.
    #[track_caller]
    pub fn snapshot(&mut self, name: &str) -> &mut Self {
        let (w, h) = (self.size.0 as u32 * 2, self.size.1 as u32 * 2);
        let img = CpuRenderer::new(w, h).render_to_image(&self.rt.draw(), w, h, 2.0);
        let tol = alloy_ui_render::testing::Tolerance {
            channel: 6,
            bad_fraction: 0.002,
        };
        alloy_ui_render::testing::assert_golden(
            std::path::Path::new("tests/snapshots"),
            name,
            &img,
            tol,
        );
        self
    }
    /// True if some text on screen equals `text`.
    pub fn has_text(&self, text: &str) -> bool {
        self.placed().iter().any(|(_, t)| t == text)
    }
    /// Where the first text equal to `text` is drawn.
    pub fn position(&self, text: &str) -> Option<Point> {
        self.placed()
            .into_iter()
            .find(|(_, t)| t == text)
            .map(|(p, _)| p)
    }
    /// Click the first widget showing exactly `text`. Panics if there is none.
    pub fn click(&mut self, text: &str) -> &mut Self {
        let (p, _) = self
            .placed()
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
            .placed()
            .into_iter()
            .find(|(_, t)| t == text)
            .unwrap_or_else(|| panic!("no widget with text {text:?}"));
        self.rt
            .handle_event(&Event::CursorMoved(Point::new(p.x + 3.0, p.y + 3.0)));
        self
    }
    fn text_point(&self, text: &str) -> Point {
        let (p, _) = self
            .placed()
            .into_iter()
            .find(|(_, t)| t == text)
            .unwrap_or_else(|| panic!("no widget with text {text:?}"));
        Point::new(p.x + 3.0, p.y + 3.0)
    }
    /// Press on the widget showing `from`, move the pointer onto the one showing `to` in
    /// steps, and keep the button down (the drag stays in flight, for a screenshot of it).
    /// Panics if either text is missing.
    pub fn drag_hold(&mut self, from: &str, to: &str) -> &mut Self {
        let (a, b) = (self.text_point(from), self.text_point(to));
        self.rt.handle_event(&Event::CursorMoved(a));
        self.rt
            .handle_event(&Event::MousePressed(MouseButton::Left));
        for i in 1..=8 {
            let k = i as f32 / 8.0;
            let p = Point::new(a.x + (b.x - a.x) * k, a.y + (b.y - a.y) * k);
            self.rt.handle_event(&Event::CursorMoved(p));
        }
        self
    }
    /// Drag the widget showing `from` onto the one showing `to` and release there: what a user's
    /// drag and drop does. Panics if either text is missing.
    pub fn drag(&mut self, from: &str, to: &str) -> &mut Self {
        self.drag_hold(from, to);
        self.rt
            .handle_event(&Event::MouseReleased(MouseButton::Left));
        self
    }
    /// Let go of the button after [`drag_hold`](Self::drag_hold).
    pub fn release(&mut self) -> &mut Self {
        self.rt
            .handle_event(&Event::MouseReleased(MouseButton::Left));
        self
    }
    fn host(&mut self, events: Vec<HostEvent>) -> &mut Self {
        let msgs: Vec<_> = events
            .into_iter()
            .filter_map(|e| self.rt.app().host_event(e))
            .collect();
        self.rt.dispatch_all(msgs);
        self
    }
    /// Drop files on the window, as another app's drag would.
    pub fn drop_files<P: Into<std::path::PathBuf>>(
        &mut self,
        files: impl IntoIterator<Item = P>,
    ) -> &mut Self {
        crate::cx::set_files_hovering(false);
        self.host(vec![
            HostEvent::FilesHovering(false),
            HostEvent::FilesDropped(files.into_iter().map(Into::into).collect()),
        ])
    }
    /// Start or stop dragging files over the window without dropping them.
    pub fn hover_files(&mut self, over: bool) -> &mut Self {
        crate::cx::set_files_hovering(over);
        self.host(vec![HostEvent::FilesHovering(over)])
    }
    /// Open a deep link, as the OS does when a `myapp://...` URL is clicked.
    pub fn open_url(&mut self, url: &str) -> &mut Self {
        self.host(vec![HostEvent::OpenUrl(url.into())])
    }
    /// Open files, as the OS does for a file association or a forwarded second launch.
    pub fn open_files<P: Into<std::path::PathBuf>>(
        &mut self,
        files: impl IntoIterator<Item = P>,
    ) -> &mut Self {
        self.host(vec![HostEvent::OpenFiles(
            files.into_iter().map(Into::into).collect(),
        )])
    }
    /// Move the pointer to window position `(x, y)`.
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.rt.handle_event(&Event::CursorMoved(Point::new(x, y)));
        self
    }
    /// Press the left button where the pointer is.
    pub fn mouse_down(&mut self) -> &mut Self {
        self.rt
            .handle_event(&Event::MousePressed(MouseButton::Left));
        self
    }
    /// Release the left button where the pointer is.
    pub fn mouse_up(&mut self) -> &mut Self {
        self.rt
            .handle_event(&Event::MouseReleased(MouseButton::Left));
        self
    }
    /// Click at window position `(x, y)`.
    pub fn click_at(&mut self, x: f32, y: f32) -> &mut Self {
        self.move_to(x, y).mouse_down().mouse_up()
    }
    /// Drag the left button from one window position to another in a few steps,
    /// releasing at the end.
    pub fn drag_px(&mut self, from: (f32, f32), to: (f32, f32)) -> &mut Self {
        self.drag_hold_px(from, to);
        self.mouse_up()
    }
    /// Like [`drag_px`](Self::drag_px) but leave the button down, for a frame mid-drag.
    pub fn drag_hold_px(&mut self, from: (f32, f32), to: (f32, f32)) -> &mut Self {
        self.move_to(from.0, from.1).mouse_down();
        for i in 1..=8 {
            let t = i as f32 / 8.0;
            self.move_to(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        }
        self
    }
    /// Scroll the wheel by `dy` logical pixels at the pointer (positive scrolls content up).
    pub fn scroll_here(&mut self, dy: f32) -> &mut Self {
        self.rt.handle_event(&Event::Scrolled { dx: 0.0, dy });
        self
    }
    /// Show or hide the layout inspector overlay (what F12 does in a running app).
    pub fn inspect(&mut self, on: bool) -> &mut Self {
        self.rt.set_inspector(on);
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
            k if k.eq_ignore_ascii_case("enter") => Key::Enter,
            k if k.eq_ignore_ascii_case("escape") || k.eq_ignore_ascii_case("esc") => Key::Escape,
            k if k.eq_ignore_ascii_case("tab") => Key::Tab,
            k if k.eq_ignore_ascii_case("backspace") => Key::Backspace,
            k if k.chars().count() > 1 => Key::Named(k.into()),
            k => Key::Character(k.into()),
        };
        self.rt.handle_event(&Event::KeyPressed(key, sc.mods));
        self
    }
    /// Do what the OS does when the user picks a menu item, the tray icon or a global hotkey:
    /// `h.desktop("mod+o")` (an item with a shortcut), `h.desktop("cmd:About")` (an item
    /// without one), `h.desktop("tray:Show")`, `h.desktop("tray:click")` or
    /// `h.desktop("global:ctrl+alt+space")`.
    pub fn desktop(&mut self, id: &str) -> &mut Self {
        // Like the real shell: names are queued for the page's shortcut layer, and an item
        // identified by its shortcut is replayed as that key press.
        if ["cmd:", "tray:", "global:"]
            .iter()
            .any(|p| id.starts_with(p))
        {
            crate::keys::invoke(id.to_string());
            self.rt.handle_event(&Event::Tick(self.now));
            self
        } else {
            self.chord(id)
        }
    }
    /// The menus, tray icon and global hotkeys the app currently asks the OS for.
    pub fn desktop_spec(&self) -> crate::DesktopSpec {
        crate::keys::current_desktop()
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
