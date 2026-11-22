//! The windowing event loop. Wraps `winit`, translating its events into
//! [`PlatformEvent`]s and driving a [`ShellHandler`].
//!
//! The loop uses `ControlFlow::Wait`, so an idle app uses no CPU; redraws are
//! only produced on request ([`ShellContext::request_redraw`]) or when the OS
//! asks for one. Background tasks wake the loop through a [`ShellProxy`].

use crate::safe_area::Insets;
use crate::touch::{Fling, Gesture, GestureRecognizer, PointerEmulation, TouchEvent, TouchPhase};
use crate::{convert, PlatformError, WindowSettings};
use alloy_ui_core::{Event, Key, Point, Size};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, Ime, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::ModifiersState,
    window::{Theme, Window, WindowId},
};

/// Everything the shell can report to the application.
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformEvent {
    /// A normal input event (mouse, keyboard, text, IME, resize, redraw).
    Input(Event),
    /// A file is being dragged over the window.
    FileHovered(PathBuf),
    /// The drag left the window without dropping.
    FileHoverCancelled,
    /// Files were dropped on the window. One event per drop, however many
    /// files it carried. (winit reports them one by one; the shell batches
    /// them at the end of the loop turn.)
    FilesDropped(Vec<PathBuf>),
    /// The OS appearance or accessibility settings changed, or were read for
    /// the first time (shortly after start-up). See [`crate::settings`].
    SettingsChanged(crate::settings::SystemSettings),
    /// The app was asked to open something: a URL for a registered scheme
    /// (`myapp://...`), or files (macOS "Open With", a second launch that was
    /// forwarded to this single instance). Carries the raw argument strings.
    Open(Vec<String>),
    /// The OS switched between light and dark.
    ThemeChanged {
        /// True when the new theme is dark.
        dark: bool,
    },
    /// The display scale factor changed (e.g. window moved between monitors).
    ScaleFactorChanged(f64),
    /// The user asked to close the window that [`ShellContext::window_key`]
    /// names. The default closes it (the loop exits when the last window is
    /// gone); call [`ShellContext::cancel_close`] to keep it open.
    CloseRequested,
    /// The window gained (`true`) or lost (`false`) keyboard focus.
    Focused(bool),
    /// A window is about to be destroyed. [`ShellContext::window_key`] still
    /// names it and [`ShellContext::window`] is still valid, so per-window
    /// state (surfaces, runtimes) can be dropped here.
    WindowClosed,
    /// The user chose a menu item (menu bar or tray menu); carries its id.
    MenuAction(String),
    /// The tray icon was clicked (left button).
    TrayClicked,
    /// A hotkey registered with [`ShellContext::register_global_hotkey`] was
    /// pressed, possibly while another application had the focus. Carries the
    /// string it was registered with.
    GlobalHotkey(String),
    /// A [`ShellProxy::wake`] arrived from another thread.
    Wake,
    /// A finger touched, moved or lifted (raw, logical pixels). The shell
    /// already turns the first finger into mouse events and pans into
    /// scrolling, so most apps only need [`PlatformEvent::Gesture`].
    Touch(TouchEvent),
    /// A recognised gesture: tap, double tap, long press, pan, fling release
    /// or pinch. Taps and pans have already been delivered as mouse and
    /// scroll input; this is for apps that want more (pinch to zoom).
    Gesture(Gesture),
    /// The native surface is gone (Android moved the app to the background).
    /// Drop GPU surfaces now; [`PlatformEvent::Resumed`] says when to rebuild.
    /// Never sent on desktop or iOS, whose windows survive.
    Suspended,
    /// The app returned to the foreground with a fresh window. Build surfaces
    /// again for the windows in [`ShellContext::window_keys`].
    Resumed,
    /// The area covered by system chrome (notch, bars, home indicator)
    /// changed; see [`ShellContext::safe_area`].
    SafeAreaChanged(Insets),
    /// Assistive technology (a screen reader) asked a control to do something.
    /// `window` is the window whose [`ShellContext::set_accessibility`] tree
    /// holds the target.
    Accessibility {
        /// The window the request is for.
        window: WindowKey,
        /// What to do to which node.
        request: crate::access::AccessRequest,
    },
}

/// Application-side hooks driven by the shell.
pub trait ShellHandler {
    /// Called once, after the window exists and before any event.
    fn started(&mut self, _ctx: &mut ShellContext) {}
    /// Called for every platform event.
    fn event(&mut self, ctx: &mut ShellContext, event: PlatformEvent);
    /// Called when a window must be painted. Draw into the surface for
    /// `ctx.window()` here; `ctx.window_key()` says which window it is.
    fn redraw(&mut self, ctx: &mut ShellContext);
}

/// Thread-safe handle that wakes the event loop from background threads.
#[derive(Clone)]
pub struct ShellProxy(EventLoopProxy<Wake>);

#[derive(Debug, Clone, Copy)]
struct Wake;

impl ShellProxy {
    /// Wake the loop; the handler receives [`PlatformEvent::Wake`].
    /// Returns false if the loop has already exited.
    pub fn wake(&self) -> bool {
        self.0.send_event(Wake).is_ok()
    }
}

/// Stable identifier of a window, valid for the life of the event loop.
/// The window opened by [`run`] is [`WindowKey::MAIN`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowKey(pub u64);

impl WindowKey {
    /// The first window, created by [`run`].
    pub const MAIN: WindowKey = WindowKey(0);
}

/// Live frame statistics, for benchmarks and perf overlays.
///
/// A frame is one return from [`ShellHandler::redraw`] (any window), i.e. the
/// moment the application has submitted and presented its frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Frames painted since the loop started, across all windows.
    pub frames: u64,
    /// Time from entering [`run`] to the end of the first paint. `None` until
    /// the first frame has been painted.
    pub first_paint: Option<Duration>,
    /// Frames painted during the last completed one-second window.
    pub fps: f32,
    since_start: Duration,
    window_start: Duration,
    window_frames: u64,
}

impl FrameStats {
    /// Time since [`run`] was entered, as of the last painted frame.
    pub fn elapsed(&self) -> Duration {
        self.since_start
    }

    fn record(&mut self, since_start: Duration) {
        self.frames += 1;
        self.since_start = since_start;
        self.first_paint.get_or_insert(since_start);
        self.window_frames += 1;
        let span = since_start.saturating_sub(self.window_start);
        if span >= Duration::from_secs(1) {
            self.fps = self.window_frames as f32 / span.as_secs_f32();
            self.window_start = since_start;
            self.window_frames = 0;
        }
    }
}

/// Window access and control, passed to handler callbacks.
pub struct ShellContext {
    /// The window the current callback is about.
    window: Arc<Window>,
    key: WindowKey,
    windows: HashMap<WindowKey, Arc<Window>>,
    open_queue: Vec<(WindowKey, WindowSettings)>,
    close_queue: Vec<WindowKey>,
    stats: FrameStats,
    exit: bool,
    redraw: bool,
    close_cancelled: bool,
    proxy: ShellProxy,
    /// Shortcuts the shell resolves itself (menus on OSes without native accelerators).
    shortcuts: Vec<(crate::shortcut::Shortcut, String)>,
    access: HashMap<WindowKey, Option<Box<dyn crate::access::AccessAdapter>>>,
}

impl ShellContext {
    /// The window this callback is about (the one that received the input or
    /// needs painting; the main window for global events such as `Wake`).
    /// `Arc<Window>` implements `HasWindowHandle` and `HasDisplayHandle`,
    /// which is what the wgpu surface needs.
    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }
    /// Key of the window this callback is about.
    pub fn window_key(&self) -> WindowKey {
        self.key
    }
    /// Look up any open window.
    pub fn window_by_key(&self, key: WindowKey) -> Option<&Arc<Window>> {
        self.windows.get(&key)
    }
    /// Keys of all open windows, in no particular order.
    pub fn window_keys(&self) -> impl Iterator<Item = WindowKey> + '_ {
        self.windows.keys().copied()
    }
    /// Open another window under a key the caller picks (`WindowKey::MAIN`
    /// is taken; reusing an open key is ignored). It is created when this
    /// callback returns and gets a first `redraw` call right after; build its
    /// surface there, keyed by [`ShellContext::window_key`].
    pub fn open_window(&mut self, key: WindowKey, settings: WindowSettings) {
        if self.windows.contains_key(&key) || self.open_queue.iter().any(|(k, _)| *k == key) {
            log::warn!("open_window: key {key:?} is already in use");
            return;
        }
        self.open_queue.push((key, settings));
    }
    /// Close a window after this callback, without a `CloseRequested`
    /// round trip. The loop exits when the last window closes.
    pub fn close_window(&mut self, key: WindowKey) {
        self.close_queue.push(key);
    }
    /// Frame counter and first-paint time. See [`FrameStats`].
    pub fn frame_stats(&self) -> FrameStats {
        self.stats
    }
    /// Redraw one specific window on the next loop turn.
    pub fn request_redraw_of(&self, key: WindowKey) {
        if let Some(w) = self.windows.get(&key) {
            w.request_redraw();
        }
    }
    /// A proxy for waking the loop from other threads.
    pub fn proxy(&self) -> ShellProxy {
        self.proxy.clone()
    }
    /// Ask for a redraw on the next loop turn.
    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }
    /// Bring the current window to the front and give it focus (used when a
    /// second launch hands its arguments to this instance).
    pub fn focus_window(&self) {
        self.window.set_minimized(false);
        self.window.focus_window();
    }
    /// Hide the window (`false`) without closing it, or bring it back and focus it (`true`).
    /// The app keeps running, e.g. in the tray.
    pub fn set_window_visible(&self, visible: bool) {
        self.window.set_visible(visible);
        if visible {
            self.window.set_minimized(false);
            self.window.focus_window();
        }
    }
    /// Exit the event loop after this callback.
    pub fn exit(&mut self) {
        self.exit = true;
    }
    /// Veto a [`PlatformEvent::CloseRequested`].
    pub fn cancel_close(&mut self) {
        self.close_cancelled = true;
    }
    /// Set the window title.
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }
    /// Scale factor (physical pixels per logical pixel).
    pub fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }
    /// Window size in physical pixels (what the surface should be configured to).
    pub fn physical_size(&self) -> (u32, u32) {
        let s = self.window.inner_size();
        (s.width, s.height)
    }
    /// Window size in logical pixels.
    pub fn logical_size(&self) -> Size {
        let s = self
            .window
            .inner_size()
            .to_logical::<f32>(self.scale_factor());
        Size::new(s.width, s.height)
    }
    /// Parts of the window system chrome may cover (notch, status and
    /// navigation bars, home indicator), in logical pixels. All zero on
    /// desktop. Pad scrolling content by this and let backgrounds run under it.
    pub fn safe_area(&self) -> Insets {
        crate::os::safe_area(&self.window)
    }
    /// Show or hide the on-screen keyboard together with IME. On phones and
    /// tablets this is what brings the soft keyboard up: call it with true
    /// when a text field gains focus and false when focus leaves. Same as
    /// [`ShellContext::set_ime_allowed`].
    pub fn set_soft_keyboard(&self, visible: bool) {
        self.window.set_ime_allowed(visible);
    }
    /// Enable or disable IME (call with true while a text field has focus).
    pub fn set_ime_allowed(&self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
    }
    /// Tell the IME where the caret is (logical pixels) so its candidate
    /// window appears next to it.
    pub fn set_ime_cursor_area(&self, at: Point, size: Size) {
        use winit::dpi::{LogicalPosition, LogicalSize};
        self.window.set_ime_cursor_area(
            LogicalPosition::new(at.x, at.y),
            LogicalSize::new(size.width, size.height),
        );
    }
    /// Publish what a screen reader should see in the current window. Call it
    /// after each frame; only differences reach the OS. The first call connects
    /// the window to the platform's accessibility service (AT-SPI2 on Linux,
    /// NSAccessibility on macOS, UI Automation on Windows). Requests from
    /// assistive technology arrive as [`PlatformEvent::Accessibility`].
    pub fn set_accessibility(&mut self, tree: &crate::access::AccessTree) {
        let key = self.key;
        if !self.access.contains_key(&key) {
            let sink: crate::access::ActionSink = Arc::new(move |request| {
                crate::os::push_pending(PlatformEvent::Accessibility {
                    window: key,
                    request,
                });
            });
            let adapter = crate::os::new_access_adapter(&self.window, key, sink);
            self.access.insert(key, adapter);
        }
        if let Some(Some(a)) = self.access.get_mut(&key) {
            a.update(tree);
        }
    }
    /// Install the application menu bar. Item activations arrive as
    /// [`PlatformEvent::MenuAction`]. Errors where the OS has no menu bar.
    pub fn set_menu(&mut self, menus: &[crate::menu::Menu]) -> Result<(), PlatformError> {
        let result = crate::os::set_menu(self.main_window(), menus);
        // macOS menus handle their own key equivalents; elsewhere the shell
        // matches shortcuts itself and reports them as MenuAction.
        self.shortcuts = if cfg!(target_os = "macos") {
            Vec::new()
        } else {
            crate::shortcut::collect(menus)
        };
        // Shortcuts work even where the OS has no native menu bar (Linux);
        // the backend error is still reported so the app can show its own menu.
        result
    }
    /// Show a tray icon. Activations arrive as [`PlatformEvent::TrayClicked`]
    /// and [`PlatformEvent::MenuAction`].
    pub fn set_tray(&self, tray: &crate::tray::TraySettings) -> Result<(), PlatformError> {
        crate::os::set_tray(self.main_window(), tray)
    }
    /// Remove the tray icon.
    pub fn clear_tray(&self) {
        crate::os::clear_tray();
    }
    /// Grab `keys` (`"ctrl+alt+space"`) for the whole desktop. Presses arrive as
    /// [`PlatformEvent::GlobalHotkey`]. See [`crate::hotkey`] for what each OS supports.
    pub fn register_global_hotkey(&self, keys: &str) -> Result<(), PlatformError> {
        crate::hotkey::register(keys)
    }
    /// Release every global hotkey.
    pub fn unregister_global_hotkeys(&self) {
        crate::hotkey::unregister_all();
    }
    fn main_window(&self) -> &Window {
        self.windows.get(&WindowKey::MAIN).unwrap_or(&self.window)
    }
    /// Give the main window a see-through look (Mica, acrylic, blur,
    /// plain transparency) or take it away with [`alloy_ui_core::WindowEffect::None`].
    /// The answer says what the OS really did; the app then picks a page
    /// background opacity to match.
    ///
    /// On X11 the window must have been created see-through: call
    /// [`request_window_effect`] before [`run`].
    pub fn set_effect(&mut self, effect: alloy_ui_core::WindowEffect) -> crate::EffectOutcome {
        let w = self.main_window();
        w.set_transparent(effect != alloy_ui_core::WindowEffect::None);
        crate::os::set_effect(w, effect)
    }
    /// Current OS theme as reported by the window system, if known.
    pub fn is_dark(&self) -> Option<bool> {
        self.window.theme().map(|t| t == Theme::Dark)
    }
}

struct App<H> {
    settings: WindowSettings,
    make: Option<Box<dyn FnOnce(ShellProxy) -> H>>,
    handler: Option<H>,
    ctx: Option<ShellContext>,
    ids: HashMap<WindowId, WindowKey>,
    mods: ModifiersState,
    cursor: Point,
    error: Option<PlatformError>,
    proxy: ShellProxy,
    t0: Instant,
    /// True while an IME preedit string is showing; key events are dropped.
    composing: bool,
    /// Files of the drop being delivered; flushed in `about_to_wait`.
    dropped: Vec<PathBuf>,
    #[cfg(not(target_os = "linux"))]
    settings_seen: Option<crate::settings::SystemSettings>,
    gestures: GestureRecognizer,
    /// Makes the first finger act as the mouse.
    pointer: PointerEmulation,
    fling: Option<(Fling, Instant)>,
    safe: Insets,
}

static INITIAL_EFFECT: std::sync::Mutex<alloy_ui_core::WindowEffect> =
    std::sync::Mutex::new(alloy_ui_core::WindowEffect::None);

static INITIAL_MIN: std::sync::Mutex<Option<(f32, f32)>> = std::sync::Mutex::new(None);

/// Ask for the main window to refuse to be resized below `width` x `height` logical pixels,
/// before [`run`]. Applied by the OS window manager (`setContentMinSize:` on macOS).
pub fn request_min_window_size(width: f32, height: f32) {
    if let Ok(mut m) = INITIAL_MIN.lock() {
        *m = Some((width, height));
    }
}

fn with_min(a: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    match INITIAL_MIN.lock().ok().and_then(|m| *m) {
        Some((w, h)) => a.with_min_inner_size(LogicalSize::new(w as f64, h as f64)),
        None => a,
    }
}

/// Ask for windows to be created see-through, before [`run`]. X11 needs a
/// 32-bit visual at creation time, so an app that will ever use a window
/// effect has to say so up front; [`ShellContext::set_effect`] then switches
/// effects (or turns them off) at any time.
pub fn request_window_effect(effect: alloy_ui_core::WindowEffect) {
    if let Ok(mut e) = INITIAL_EFFECT.lock() {
        *e = effect;
    }
}

/// `alloy dev` restarts the app on every save. When it sets `ALLOY_DEV_WINDOW` to a file, the
/// main window remembers where it was (position and logical size) in that file and reopens
/// there, so a restart looks like a reload instead of a window jumping back to the default spot.
/// Unset in normal runs: nothing is read or written.
mod dev_geometry {
    use super::*;

    fn file() -> Option<std::path::PathBuf> {
        std::env::var_os("ALLOY_DEV_WINDOW").map(Into::into)
    }

    pub fn restore(a: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
        let Some(txt) = file().and_then(|f| std::fs::read_to_string(f).ok()) else {
            return a;
        };
        let v: Vec<f64> = txt
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        match v[..] {
            [x, y, w, h] if w >= 100.0 && h >= 100.0 => a
                .with_inner_size(LogicalSize::new(w, h))
                .with_position(winit::dpi::PhysicalPosition::new(x, y)),
            _ => a,
        }
    }

    pub fn save(w: &Window) {
        let Some(f) = file() else { return };
        let scale = w.scale_factor();
        let size = w.inner_size().to_logical::<f64>(scale);
        if let Ok(p) = w.outer_position() {
            let _ = std::fs::write(f, format!("{} {} {} {}", p.x, p.y, size.width, size.height));
        }
    }
}

fn attributes(s: &WindowSettings) -> winit::window::WindowAttributes {
    let effect = INITIAL_EFFECT.lock().map(|e| *e).unwrap_or_default();
    Window::default_attributes()
        .with_title(s.title.clone())
        .with_transparent(effect != alloy_ui_core::WindowEffect::None)
        .with_inner_size(LogicalSize::new(s.size.width as f64, s.size.height as f64))
}

impl<H: ShellHandler> App<H> {
    /// Point the context at `key` (falling back to the main window).
    fn focus_ctx(&mut self, key: WindowKey) {
        if let Some(ctx) = self.ctx.as_mut() {
            if let Some(w) = ctx.windows.get(&key) {
                ctx.window = w.clone();
                ctx.key = key;
            }
        }
    }

    fn dispatch(&mut self, el: &ActiveEventLoop, ev: PlatformEvent) {
        let (Some(h), Some(ctx)) = (self.handler.as_mut(), self.ctx.as_mut()) else {
            return;
        };
        let closing = ev == PlatformEvent::CloseRequested;
        ctx.close_cancelled = false;
        h.event(ctx, ev);
        if closing && !ctx.close_cancelled {
            let key = ctx.key;
            ctx.close_queue.push(key);
        }
        self.finish(el);
    }

    fn now_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    /// Deliver events the touch layer produced.
    fn send_input(&mut self, el: &ActiveEventLoop, events: Vec<Event>) {
        for e in events {
            self.dispatch(el, PlatformEvent::Input(e));
        }
    }

    /// Built-in meaning of a gesture, then hand it to the app.
    fn apply_gesture(&mut self, el: &ActiveEventLoop, g: Gesture) {
        let events = self.pointer.gesture(g);
        self.send_input(el, events);
        if let Gesture::PanEnd { velocity } = g {
            self.fling = Fling::new(velocity).map(|f| (f, Instant::now()));
        }
        self.dispatch(el, PlatformEvent::Gesture(g));
    }

    fn touch(&mut self, el: &ActiveEventLoop, t: winit::event::Touch, scale: f64) {
        let l = t.location.to_logical::<f32>(scale);
        let phase = match t.phase {
            winit::event::TouchPhase::Started => TouchPhase::Started,
            winit::event::TouchPhase::Moved => TouchPhase::Moved,
            winit::event::TouchPhase::Ended => TouchPhase::Ended,
            winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
        };
        let te = TouchEvent {
            id: t.id,
            phase,
            pos: Point::new(l.x, l.y),
        };
        let first = self.gestures.finger_count() == 0;
        if phase == TouchPhase::Started {
            self.fling = None;
        }
        self.dispatch(el, PlatformEvent::Touch(te));
        // The first finger drives the pointer, so every existing widget works by touch.
        let events = self.pointer.before(te, first);
        self.send_input(el, events);
        let now = self.now_ms();
        for g in self.gestures.touch(te, now) {
            self.apply_gesture(el, g);
        }
        let events = self.pointer.after(te);
        self.send_input(el, events);
    }

    /// Apply queued window opens/closes and the exit/redraw flags.
    fn finish(&mut self, el: &ActiveEventLoop) {
        loop {
            let Some(ctx) = self.ctx.as_mut() else {
                return;
            };
            if ctx.exit {
                el.exit();
                return;
            }
            let (opens, closes) = (
                std::mem::take(&mut ctx.open_queue),
                std::mem::take(&mut ctx.close_queue),
            );
            if opens.is_empty() && closes.is_empty() {
                break;
            }
            for (key, settings) in opens {
                match el.create_window(attributes(&settings)) {
                    Ok(w) => {
                        let w = Arc::new(w);
                        self.ids.insert(w.id(), key);
                        w.request_redraw();
                        if let Some(ctx) = self.ctx.as_mut() {
                            ctx.windows.insert(key, w);
                        }
                    }
                    Err(e) => log::error!("open_window: {e}"),
                }
            }
            for key in closes {
                let known = self
                    .ctx
                    .as_ref()
                    .is_some_and(|c| c.windows.contains_key(&key));
                if !known {
                    continue;
                }
                // Tell the handler while the window is still alive, then drop it.
                let prev = self.ctx.as_ref().map(|c| c.key);
                self.focus_ctx(key);
                if let (Some(h), Some(ctx)) = (self.handler.as_mut(), self.ctx.as_mut()) {
                    h.event(ctx, PlatformEvent::WindowClosed);
                }
                if let Some(ctx) = self.ctx.as_mut() {
                    ctx.access.remove(&key);
                    if let Some(w) = ctx.windows.remove(&key) {
                        self.ids.remove(&w.id());
                    }
                    if ctx.windows.is_empty() {
                        ctx.exit = true;
                    } else if prev != Some(key) {
                        let back = prev.unwrap_or(WindowKey::MAIN);
                        self.focus_ctx(back);
                    } else if let Some(&next) =
                        self.ctx.as_ref().and_then(|c| c.windows.keys().min())
                    {
                        self.focus_ctx(next);
                    }
                }
            }
        }
        if let Some(ctx) = self.ctx.as_mut() {
            if std::mem::take(&mut ctx.redraw) {
                ctx.window.request_redraw();
            }
        }
    }
}

impl<H: ShellHandler> ApplicationHandler<Wake> for App<H> {
    fn suspended(&mut self, el: &ActiveEventLoop) {
        // Only Android destroys the native window here; elsewhere nothing to tell.
        if cfg!(target_os = "android") {
            self.fling = None;
            self.dispatch(el, PlatformEvent::Suspended);
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if !self.dropped.is_empty() {
            let files = std::mem::take(&mut self.dropped);
            self.dispatch(el, PlatformEvent::FilesDropped(files));
        }
        // Timers for touch: a held finger becomes a long press, a flick keeps scrolling.
        let now = self.now_ms();
        if self.gestures.next_deadline().is_some_and(|d| now >= d) {
            for g in self.gestures.poll(now) {
                self.apply_gesture(el, g);
            }
        }
        if let Some((mut f, last)) = self.fling.take() {
            let t = Instant::now();
            let dt = t.duration_since(last).as_secs_f32();
            if dt >= 1.0 / 120.0 {
                if let Some((dx, dy)) = f.step(dt) {
                    self.fling = Some((f, t));
                    self.dispatch(
                        el,
                        PlatformEvent::Input(Event::Scrolled { dx: -dx, dy: -dy }),
                    );
                }
            } else {
                self.fling = Some((f, last));
            }
        }
        let next = [
            self.gestures
                .next_deadline()
                .map(|d| self.t0 + Duration::from_millis(d)),
            self.fling
                .as_ref()
                .map(|(_, last)| *last + Duration::from_millis(16)),
        ]
        .into_iter()
        .flatten()
        .min();
        el.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.ctx.is_some() {
            // Android hands out a new native window on every return to the
            // foreground; swap it in for the main window and tell the app.
            if cfg!(target_os = "android") {
                match el.create_window(with_min(attributes(&self.settings))) {
                    Ok(w) => {
                        let w = Arc::new(w);
                        self.ids.retain(|_, k| *k != WindowKey::MAIN);
                        self.ids.insert(w.id(), WindowKey::MAIN);
                        if let Some(ctx) = self.ctx.as_mut() {
                            ctx.access.remove(&WindowKey::MAIN);
                            if ctx.key == WindowKey::MAIN {
                                ctx.window = w.clone();
                            }
                            ctx.windows.insert(WindowKey::MAIN, w.clone());
                        }
                        self.focus_ctx(WindowKey::MAIN);
                        self.dispatch(el, PlatformEvent::Resumed);
                        w.request_redraw();
                    }
                    Err(e) => log::error!("resume: {e}"),
                }
            }
            return;
        }
        #[cfg(target_os = "macos")]
        crate::os::install_open_handler();
        match el.create_window(dev_geometry::restore(with_min(attributes(&self.settings)))) {
            Ok(w) => {
                let w = Arc::new(w);
                self.ids.insert(w.id(), WindowKey::MAIN);
                let mut ctx = ShellContext {
                    window: w.clone(),
                    key: WindowKey::MAIN,
                    windows: HashMap::from([(WindowKey::MAIN, w)]),
                    open_queue: Vec::new(),
                    close_queue: Vec::new(),
                    stats: FrameStats::default(),
                    exit: false,
                    redraw: true,
                    close_cancelled: false,
                    proxy: self.proxy.clone(),
                    shortcuts: Vec::new(),
                    access: HashMap::new(),
                };
                let mut h = (self.make.take().expect("resumed once"))(self.proxy.clone());
                h.started(&mut ctx);
                self.handler = Some(h);
                self.ctx = Some(ctx);
                self.finish(el);
            }
            Err(e) => {
                self.error = Some(PlatformError(format!("create window: {e}")));
                el.exit();
            }
        }
    }

    fn user_event(&mut self, el: &ActiveEventLoop, _: Wake) {
        // Global events are about the main window.
        self.focus_ctx(WindowKey::MAIN);
        for e in crate::os::take_pending() {
            match &e {
                PlatformEvent::Accessibility { window, .. } => self.focus_ctx(*window),
                _ => self.focus_ctx(WindowKey::MAIN),
            }
            self.dispatch(el, e);
        }
        self.dispatch(el, PlatformEvent::Wake);
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some(ctx) = &self.ctx {
            if perf_log() {
                let s = ctx.stats;
                eprintln!(
                    "alloy-perf frames={} elapsed_ms={:.1} avg_fps={:.1}",
                    s.frames,
                    s.since_start.as_secs_f64() * 1e3,
                    s.frames as f64 / s.since_start.as_secs_f64().max(1e-9),
                );
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, ev: WindowEvent) {
        let Some(&key) = self.ids.get(&id) else {
            return;
        };
        self.focus_ctx(key);
        let scale = self.ctx.as_ref().map_or(1.0, |c| c.scale_factor());
        use PlatformEvent::Input as I;
        if let WindowEvent::Focused(f) = ev {
            self.dispatch(el, PlatformEvent::Focused(f));
        }
        let out = match ev {
            WindowEvent::CloseRequested => Some(PlatformEvent::CloseRequested),
            WindowEvent::RedrawRequested => {
                if let (Some(h), Some(ctx)) = (self.handler.as_mut(), self.ctx.as_mut()) {
                    h.redraw(ctx);
                    let was_first = ctx.stats.frames == 0;
                    ctx.stats.record(self.t0.elapsed());
                    if was_first && perf_log() {
                        eprintln!(
                            "alloy-perf first_paint_ms={:.1}",
                            self.t0.elapsed().as_secs_f64() * 1e3
                        );
                    }
                    self.finish(el);
                }
                None
            }
            WindowEvent::Resized(s) => {
                if key == WindowKey::MAIN {
                    if let Some(c) = &self.ctx {
                        dev_geometry::save(&c.window);
                    }
                }
                let l = s.to_logical::<f32>(scale);
                self.dispatch(el, I(Event::Resized(Size::new(l.width, l.height))));
                // Rotation, keyboard and bar changes all arrive as a resize.
                let now = self.ctx.as_ref().map(|c| c.safe_area());
                match now {
                    Some(i) if i != self.safe => {
                        self.safe = i;
                        Some(PlatformEvent::SafeAreaChanged(i))
                    }
                    _ => None,
                }
            }
            WindowEvent::Moved(_) => {
                if key == WindowKey::MAIN {
                    if let Some(c) = &self.ctx {
                        dev_geometry::save(&c.window);
                    }
                }
                None
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Some(PlatformEvent::ScaleFactorChanged(scale_factor))
            }
            WindowEvent::ThemeChanged(t) => Some(PlatformEvent::ThemeChanged {
                dark: t == Theme::Dark,
            }),
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                // Mouse events carry no modifiers; tell widgets (Shift/Ctrl-click) through
                // a pseudo key release that only `Root` looks at.
                Some(I(Event::KeyReleased(
                    Key::Named("Modifiers".into()),
                    convert::modifiers(self.mods),
                )))
            }
            WindowEvent::CursorMoved { position, .. } => {
                let l = position.to_logical::<f32>(scale);
                self.cursor = Point::new(l.x, l.y);
                Some(I(Event::CursorMoved(self.cursor)))
            }
            WindowEvent::CursorLeft { .. } => Some(I(Event::CursorLeft)),
            WindowEvent::MouseInput { state, button, .. } => {
                convert::mouse_button(button).map(|b| {
                    I(match state {
                        ElementState::Pressed => Event::MousePressed(b),
                        ElementState::Released => Event::MouseReleased(b),
                    })
                })
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = convert::scroll(delta, scale);
                Some(I(Event::Scrolled { dx, dy }))
            }
            // While the IME shows a preedit string it owns the keyboard. winit
            // may still deliver the raw key events on some backends (Wayland
            // without a text-input grab, Windows/X11 for keys the IME passes
            // through), so drop them: the commit arrives as `TextInput`.
            WindowEvent::KeyboardInput { .. } if self.composing => None,
            WindowEvent::KeyboardInput { event, .. } => {
                let m = convert::modifiers(self.mods);
                let pressed = event.state == ElementState::Pressed;
                // Printable text first (so a widget sees the key press, then the text).
                let text = event
                    .text
                    .as_ref()
                    .filter(|t| pressed && !m.ctrl && !m.logo && !t.chars().any(char::is_control));
                let key = convert::key(&event.logical_key);
                if let Some(k) = &key {
                    let hit = pressed
                        .then(|| {
                            self.ctx.as_ref().and_then(|c| {
                                c.shortcuts
                                    .iter()
                                    .find(|(s, _)| s.matches(k, &m))
                                    .map(|(_, id)| id.clone())
                            })
                        })
                        .flatten();
                    if let Some(id) = hit {
                        self.dispatch(el, PlatformEvent::MenuAction(id));
                        return;
                    }
                }
                if let Some(k) = key {
                    let ev = if pressed {
                        Event::KeyPressed(k, m)
                    } else {
                        Event::KeyReleased(k, m)
                    };
                    self.dispatch(el, I(ev));
                }
                text.map(|t| I(Event::TextInput(t.to_string())))
            }
            WindowEvent::Ime(Ime::Preedit(s, _)) => {
                self.composing = !s.is_empty();
                Some(I(Event::ImePreedit(s)))
            }
            WindowEvent::Ime(Ime::Commit(s)) => {
                self.composing = false;
                Some(I(Event::TextInput(s)))
            }
            WindowEvent::Ime(_) => {
                self.composing = false;
                None
            }
            WindowEvent::Touch(t) => {
                self.touch(el, t, scale);
                None
            }
            WindowEvent::HoveredFile(p) => Some(PlatformEvent::FileHovered(p)),
            WindowEvent::HoveredFileCancelled => Some(PlatformEvent::FileHoverCancelled),
            WindowEvent::DroppedFile(p) => {
                self.dropped.push(p);
                None
            }
            // Settings can only change while another app is in front, so on
            // OSes without a change notification we re-read on focus.
            #[cfg(not(target_os = "linux"))]
            WindowEvent::Focused(true) => {
                let now = crate::settings::system_settings();
                if self.settings_seen != Some(now) {
                    self.settings_seen = Some(now);
                    Some(PlatformEvent::SettingsChanged(now))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(e) = out {
            self.dispatch(el, e);
        }
    }
}

/// True when `ALLOY_PERF` is set: the shell then prints `alloy-perf` lines
/// (first paint, frame totals at exit) to stderr for benchmark scripts.
fn perf_log() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ALLOY_PERF").is_some())
}

/// Create a window and run the event loop until the handler exits.
/// `make` builds the handler once the window system is ready.
///
/// Must be called on the main thread (a requirement of macOS and winit).
pub fn run<H: ShellHandler + 'static>(
    settings: WindowSettings,
    make: impl FnOnce(ShellProxy) -> H + 'static,
) -> Result<(), PlatformError> {
    let t0 = Instant::now();
    // A link that launches the app arrives as an Apple Event while the app starts, so the
    // handler goes in before the event loop exists; `resumed` installs it again in case AppKit
    // replaced it while finishing the launch.
    #[cfg(target_os = "macos")]
    crate::os::install_open_handler();
    let mut builder = EventLoop::<Wake>::with_user_event();
    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_msg_hook(crate::os::msg_hook);
    }
    #[cfg(target_os = "android")]
    {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        let app = crate::os::android_app().ok_or_else(|| {
            PlatformError("android: call alloy_ui_platform::android::init(app) first".into())
        })?;
        builder.with_android_app(app);
    }
    let el = builder
        .build()
        .map_err(|e| PlatformError(format!("event loop: {e}")))?;
    el.set_control_flow(ControlFlow::Wait);
    let proxy = ShellProxy(el.create_proxy());
    crate::os::set_waker(proxy.clone());
    #[cfg(target_os = "linux")]
    crate::os::watch_theme();
    let mut app = App {
        t0,
        ids: HashMap::new(),
        settings,
        make: Some(Box::new(make)),
        handler: None,
        ctx: None,
        mods: ModifiersState::empty(),
        cursor: Point::default(),
        error: None,
        proxy,
        composing: false,
        dropped: Vec::new(),
        #[cfg(not(target_os = "linux"))]
        settings_seen: None,
        gestures: GestureRecognizer::new(),
        pointer: PointerEmulation::new(),
        fling: None,
        safe: Insets::ZERO,
    };
    // What this launch itself was asked to open (a deep link, a file association).
    let launch: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    if !crate::deeplink::classify(&launch).is_empty() {
        crate::os::push_pending(PlatformEvent::Open(launch));
    }
    // Read the OS settings off the UI thread (it can spawn helper processes);
    // the answer arrives as `SettingsChanged` right after start-up.
    let _ = std::thread::Builder::new()
        .name("alloy-settings".into())
        .spawn(|| {
            crate::os::push_pending(PlatformEvent::SettingsChanged(
                crate::settings::system_settings(),
            ))
        });
    el.run_app(&mut app)
        .map_err(|e| PlatformError(format!("run: {e}")))?;
    app.error.map_or(Ok(()), Err)
}
