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

use crate::EffectOutcome;
use alloy_ui_core::WindowEffect;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
use x11rb::wrapper::ConnectionExt as _;

/// Window effects on Linux. See-through windows need a compositor (any
/// Wayland session has one; on X11 we check for `_NET_WM_CM_S<n>`). Blur is
/// asked of KDE's KWin through `_KDE_NET_WM_BLUR_BEHIND_REGION` on X11; other
/// desktops, and Wayland, get see-through only.
pub fn set_effect(window: &winit::window::Window, effect: WindowEffect) -> EffectOutcome {
    let Ok(handle) = window.window_handle() else {
        return EffectOutcome::Unsupported;
    };
    let id = match handle.as_raw() {
        RawWindowHandle::Xlib(h) => Some(h.window as u32),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        RawWindowHandle::Wayland(_) => None,
        _ => return EffectOutcome::Unsupported,
    };
    match id {
        None => match effect {
            WindowEffect::None | WindowEffect::Transparent => EffectOutcome::Applied,
            _ => EffectOutcome::SeeThrough,
        },
        Some(win) => x11_effect(win, effect, is_kde()).unwrap_or(EffectOutcome::Unsupported),
    }
}

fn is_kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.split(':').any(|p| p == "KDE"))
}

fn x11_effect(win: u32, effect: WindowEffect, kde: bool) -> Option<EffectOutcome> {
    let (c, screen) = x11rb::rust_connection::RustConnection::connect(None).ok()?;
    let atom = |name: &str| {
        Some(
            c.intern_atom(false, name.as_bytes())
                .ok()?
                .reply()
                .ok()?
                .atom,
        )
    };
    let blur = atom("_KDE_NET_WM_BLUR_BEHIND_REGION")?;
    let want_blur = matches!(effect, WindowEffect::Acrylic | WindowEffect::Blur);
    if want_blur && kde {
        // An empty region means "the whole window".
        c.change_property32(PropMode::REPLACE, win, blur, AtomEnum::CARDINAL, &[])
            .ok()?;
    } else {
        c.delete_property(win, blur).ok()?;
    }
    c.flush().ok()?;
    let composited = {
        let cm = atom(&format!("_NET_WM_CM_S{screen}"))?;
        c.get_selection_owner(cm).ok()?.reply().ok()?.owner != x11rb::NONE
    };
    Some(match effect {
        WindowEffect::None => EffectOutcome::Applied,
        _ if !composited => EffectOutcome::Unsupported,
        WindowEffect::Transparent => EffectOutcome::Applied,
        _ if want_blur && kde => EffectOutcome::Applied,
        _ => EffectOutcome::SeeThrough,
    })
}

/// Install a per-user `.desktop` file that makes this executable the handler
/// of `scheme`, and tell `xdg-mime` about it.
pub fn register_scheme(app_id: &str, scheme: &str, exe: &str) -> Result<(), crate::PlatformError> {
    let err = |e: std::io::Error| crate::PlatformError(format!("register_scheme: {e}"));
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .ok_or_else(|| crate::PlatformError("register_scheme: no home directory".into()))?;
    let dir = home.join("applications");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let file = format!("{app_id}.desktop");
    let path = dir.join(&file);
    // Several schemes for one app: keep the ones already registered.
    let mut schemes: Vec<String> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("MimeType=").map(str::to_owned))
        })
        .map(|m| {
            m.split(';')
                .filter_map(|t| t.strip_prefix("x-scheme-handler/"))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if !schemes.iter().any(|s| s == scheme) {
        schemes.push(scheme.to_owned());
    }
    let refs: Vec<&str> = schemes.iter().map(String::as_str).collect();
    std::fs::write(&path, crate::deeplink::desktop_entry(app_id, exe, &refs)).map_err(err)?;
    // Both tools are optional niceties: without them the desktop file is still picked up
    // by most launchers, so their failure is not an error.
    capture(
        "xdg-mime",
        &["default", &file, &format!("x-scheme-handler/{scheme}")],
    );
    capture("update-desktop-database", &[dir.to_str().unwrap_or("")]);
    Ok(())
}
