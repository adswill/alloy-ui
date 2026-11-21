//! iOS specifics: pasteboard and safe-area insets through UIKit. Windowing and
//! touch come from `winit`'s UIKit backend. Only meaningful when built for an
//! iOS target; it cannot be run in the cloud sandbox (see docs/MOBILE.md).

use crate::safe_area::Insets;
use objc2::rc::Retained;
use objc2_foundation::NSString;
use objc2_ui_kit::{UIPasteboard, UIView};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Text on the general pasteboard.
pub fn clipboard_read() -> Option<String> {
    let _main = objc2::MainThreadMarker::new()?;
    // SAFETY: `generalPasteboard` and `string` are plain getters, called on the main thread.
    let s: Option<Retained<NSString>> = unsafe { UIPasteboard::generalPasteboard().string() };
    s.map(|s| s.to_string())
}

/// Put text on the general pasteboard. False when not on the main thread.
pub fn clipboard_write(text: &str) -> bool {
    let Some(_main) = objc2::MainThreadMarker::new() else {
        return false;
    };
    let s = NSString::from_str(text);
    // SAFETY: `setString:` takes an NSString, called on the main thread.
    unsafe { UIPasteboard::generalPasteboard().setString(Some(&s)) };
    true
}

/// `safeAreaInsets` of the window's root view (points, which equal logical px).
pub(crate) fn safe_area(window: &winit::window::Window) -> Insets {
    let Some(_main) = objc2::MainThreadMarker::new() else {
        return Insets::ZERO;
    };
    let Ok(handle) = window.window_handle() else {
        return Insets::ZERO;
    };
    let RawWindowHandle::UiKit(h) = handle.as_raw() else {
        return Insets::ZERO;
    };
    // SAFETY: winit documents `ui_view` as a valid `UIView*` for the lifetime of the
    // window, and we hold `window` (hence the view) for this whole call on the main thread.
    let view: &UIView = unsafe { h.ui_view.cast::<UIView>().as_ref() };
    let i = view.safeAreaInsets();
    Insets {
        top: i.top as f32,
        left: i.left as f32,
        bottom: i.bottom as f32,
        right: i.right as f32,
    }
}
