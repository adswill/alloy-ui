//! Mobile stand-ins for the desktop extras (settings, window effects, rich clipboard, URL
//! schemes): none of them has a mobile backend yet, so each reports "not available".

use crate::clipboard::ClipContent;
use crate::settings::SystemSettings;
use crate::EffectOutcome;
use alloy_ui_core::WindowEffect;

/// No OS settings query on mobile yet.
pub fn read_settings() -> SystemSettings {
    SystemSettings::default()
}
/// Rich clipboard formats are not bridged on mobile.
pub fn rich_read() -> Option<ClipContent> {
    None
}
/// See [`rich_read`].
pub fn rich_write(_c: &ClipContent) -> bool {
    false
}
/// Mobile windows are always opaque.
pub fn set_effect(_w: &winit::window::Window, _e: WindowEffect) -> EffectOutcome {
    EffectOutcome::Unsupported
}
/// Apps declare their URL schemes in the manifest or Info.plist, not at run time.
pub fn register_scheme(
    _app_id: &str,
    _scheme: &str,
    _exe: &str,
) -> Result<(), crate::PlatformError> {
    Err(crate::PlatformError(
        "URL schemes are declared in the app manifest on mobile".into(),
    ))
}
