//! Linux extras: system settings, window effects, rich clipboard, URL scheme
//! registration. Kept apart from `linux.rs` so the pieces stay independent.

use super::capture;
use crate::clipboard::{self, ClipContent};
use crate::settings::{self, SystemSettings};

/// Read dark mode, reduced motion, contrast and accent from GNOME's settings
/// (also published by KDE and others through the same schema where present).
pub fn read_settings() -> SystemSettings {
    let get = |schema: &str, key: &str| capture("gsettings", &["get", schema, key]);
    let iface = "org.gnome.desktop.interface";
    SystemSettings {
        dark: super::linux::is_dark().unwrap_or(false),
        reduced_motion: settings::parse_gsettings_bool(get(iface, "enable-animations").as_deref())
            .is_some_and(|animations_on| !animations_on),
        high_contrast: settings::parse_gsettings_bool(
            get("org.gnome.desktop.a11y.interface", "high-contrast").as_deref(),
        )
        .unwrap_or(false),
        accent: settings::parse_gnome_accent(get(iface, "accent-color").as_deref()),
    }
}

fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

fn x11() -> bool {
    std::env::var_os("DISPLAY").is_some()
}

/// Read every format the selection offers (text, HTML, PNG, file list).
pub fn rich_read() -> Option<ClipContent> {
    if wayland() {
        if let Some(mimes) = super::wl_clip::mimes() {
            let c = clipboard::from_formats(&mimes, &mut super::wl_clip::read_mime);
            if !c.is_empty() {
                return Some(c);
            }
        }
    }
    if x11() {
        if let Some(targets) = super::x11_clip::read_targets() {
            let c = clipboard::from_formats(&targets, &mut super::x11_clip::read_target);
            if !c.is_empty() {
                return Some(c);
            }
        }
    }
    None
}

/// Offer every format of `c` at once. False when no clipboard service took it.
pub fn rich_write(c: &ClipContent) -> bool {
    let formats = clipboard::to_formats(c);
    (wayland() && super::wl_clip::write_formats(formats.clone()))
        || (x11() && super::x11_clip::write_formats(formats))
}
