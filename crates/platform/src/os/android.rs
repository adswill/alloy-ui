//! Android specifics. Windowing, touch and the soft keyboard come from `winit`
//! (NativeActivity backend); this file holds the process-wide `AndroidApp`
//! handle and the system-bar insets.

use crate::safe_area::Insets;
use std::sync::OnceLock;
use winit::platform::android::activity::AndroidApp;

static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Remember the `AndroidApp` that `android_main` received. Call this first
/// thing in `android_main`; [`crate::run`] needs it to build the event loop.
pub fn init(app: AndroidApp) {
    let _ = APP.set(app);
}

/// The stored app handle, if `init` ran.
pub(crate) fn app() -> Option<AndroidApp> {
    APP.get().cloned()
}

/// Insets from the free area of the window that Android reports
/// (`content_rect`: the window minus system bars and, with
/// `adjustResize`, the soft keyboard).
pub(crate) fn safe_area(window: &winit::window::Window) -> Insets {
    let Some(app) = APP.get() else {
        return Insets::ZERO;
    };
    let r = app.content_rect();
    let size = window.inner_size();
    Insets::from_content_rect(
        size.width,
        size.height,
        (r.left, r.top, r.right, r.bottom),
        window.scale_factor(),
    )
}
