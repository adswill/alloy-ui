//! The windowing event loop. Wraps `winit`, translating its events into
//! [`PlatformEvent`]s and driving a [`ShellHandler`].
//!
//! The loop uses `ControlFlow::Wait`, so an idle app uses no CPU; redraws are
//! only produced on request ([`ShellContext::request_redraw`]) or when the OS
//! asks for one. Background tasks wake the loop through a [`ShellProxy`].

use crate::{convert, PlatformError, WindowSettings};
use alloy_ui_core::{Event, Point, Size};
use std::{path::PathBuf, sync::Arc};
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
    /// A file was dropped on the window.
    FileDropped(PathBuf),
    /// The OS switched between light and dark.
    ThemeChanged {
        /// True when the new theme is dark.
        dark: bool,
    },
    /// The display scale factor changed (e.g. window moved between monitors).
    ScaleFactorChanged(f64),
    /// The user asked to close the window. The default is to exit; call
    /// [`ShellContext::cancel_close`] to keep the window open.
    CloseRequested,
    /// The user chose a menu item (menu bar or tray menu); carries its id.
    MenuAction(String),
    /// The tray icon was clicked (left button).
    TrayClicked,
    /// A [`ShellProxy::wake`] arrived from another thread.
    Wake,
}

/// Application-side hooks driven by the shell.
pub trait ShellHandler {
    /// Called once, after the window exists and before any event.
    fn started(&mut self, _ctx: &mut ShellContext) {}
    /// Called for every platform event.
    fn event(&mut self, ctx: &mut ShellContext, event: PlatformEvent);
    /// Called when the window must be painted. Draw into the surface for
    /// `ctx.window()` here.
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

/// Window access and control, passed to handler callbacks.
pub struct ShellContext {
    window: Arc<Window>,
    exit: bool,
    redraw: bool,
    close_cancelled: bool,
    proxy: ShellProxy,
    /// Shortcuts the shell resolves itself (menus on OSes without native accelerators).
    shortcuts: Vec<(crate::shortcut::Shortcut, String)>,
}

impl ShellContext {
    /// The window. `Arc<Window>` implements `HasWindowHandle` and
    /// `HasDisplayHandle`, which is what the wgpu surface needs.
    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }
    /// A proxy for waking the loop from other threads.
    pub fn proxy(&self) -> ShellProxy {
        self.proxy.clone()
    }
    /// Ask for a redraw on the next loop turn.
    pub fn request_redraw(&mut self) {
        self.redraw = true;
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
    /// Install the application menu bar. Item activations arrive as
    /// [`PlatformEvent::MenuAction`]. Errors where the OS has no menu bar.
    pub fn set_menu(&mut self, menus: &[crate::menu::Menu]) -> Result<(), PlatformError> {
        let result = crate::os::set_menu(&self.window, menus);
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
        crate::os::set_tray(&self.window, tray)
    }
    /// Remove the tray icon.
    pub fn clear_tray(&self) {
        crate::os::clear_tray();
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
    mods: ModifiersState,
    cursor: Point,
    error: Option<PlatformError>,
    proxy: ShellProxy,
}

impl<H: ShellHandler> App<H> {
    fn dispatch(&mut self, el: &ActiveEventLoop, ev: PlatformEvent) {
        let (Some(h), Some(ctx)) = (self.handler.as_mut(), self.ctx.as_mut()) else {
            return;
        };
        let closing = ev == PlatformEvent::CloseRequested;
        ctx.close_cancelled = false;
        h.event(ctx, ev);
        if closing && !ctx.close_cancelled {
            ctx.exit = true;
        }
        self.finish(el);
    }

    fn finish(&mut self, el: &ActiveEventLoop) {
        if let Some(ctx) = self.ctx.as_mut() {
            if ctx.exit {
                el.exit();
            } else if std::mem::take(&mut ctx.redraw) {
                ctx.window.request_redraw();
            }
        }
    }
}

impl<H: ShellHandler> ApplicationHandler<Wake> for App<H> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.ctx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.settings.title.clone())
            .with_inner_size(LogicalSize::new(
                self.settings.size.width as f64,
                self.settings.size.height as f64,
            ));
        match el.create_window(attrs) {
            Ok(w) => {
                let mut ctx = ShellContext {
                    window: Arc::new(w),
                    exit: false,
                    redraw: true,
                    close_cancelled: false,
                    proxy: self.proxy.clone(),
                    shortcuts: Vec::new(),
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
        for e in crate::os::take_pending() {
            self.dispatch(el, e);
        }
        self.dispatch(el, PlatformEvent::Wake);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        let scale = self.ctx.as_ref().map_or(1.0, |c| c.scale_factor());
        use PlatformEvent::Input as I;
        let out = match ev {
            WindowEvent::CloseRequested => Some(PlatformEvent::CloseRequested),
            WindowEvent::RedrawRequested => {
                if let (Some(h), Some(ctx)) = (self.handler.as_mut(), self.ctx.as_mut()) {
                    h.redraw(ctx);
                    self.finish(el);
                }
                None
            }
            WindowEvent::Resized(s) => {
                let l = s.to_logical::<f32>(scale);
                Some(I(Event::Resized(Size::new(l.width, l.height))))
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Some(PlatformEvent::ScaleFactorChanged(scale_factor))
            }
            WindowEvent::ThemeChanged(t) => Some(PlatformEvent::ThemeChanged {
                dark: t == Theme::Dark,
            }),
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m.state();
                None
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
            WindowEvent::Ime(Ime::Preedit(s, _)) => Some(I(Event::ImePreedit(s))),
            WindowEvent::Ime(Ime::Commit(s)) => Some(I(Event::TextInput(s))),
            WindowEvent::Ime(_) => None,
            WindowEvent::HoveredFile(p) => Some(PlatformEvent::FileHovered(p)),
            WindowEvent::HoveredFileCancelled => Some(PlatformEvent::FileHoverCancelled),
            WindowEvent::DroppedFile(p) => Some(PlatformEvent::FileDropped(p)),
            _ => None,
        };
        if let Some(e) = out {
            self.dispatch(el, e);
        }
    }
}

/// Create a window and run the event loop until the handler exits.
/// `make` builds the handler once the window system is ready.
///
/// Must be called on the main thread (a requirement of macOS and winit).
pub fn run<H: ShellHandler + 'static>(
    settings: WindowSettings,
    make: impl FnOnce(ShellProxy) -> H + 'static,
) -> Result<(), PlatformError> {
    let mut builder = EventLoop::<Wake>::with_user_event();
    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_msg_hook(crate::os::msg_hook);
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
        settings,
        make: Some(Box::new(make)),
        handler: None,
        ctx: None,
        mods: ModifiersState::empty(),
        cursor: Point::default(),
        error: None,
        proxy,
    };
    el.run_app(&mut app)
        .map_err(|e| PlatformError(format!("run: {e}")))?;
    app.error.map_or(Ok(()), Err)
}
