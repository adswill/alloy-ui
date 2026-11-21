//! Global hotkeys: shortcuts that fire even while another application has the
//! focus. A press arrives as [`PlatformEvent::GlobalHotkey`](crate::PlatformEvent)
//! carrying the string it was registered with.
//!
//! Support: Windows (`RegisterHotKey`), macOS (Carbon `RegisterEventHotKey`, no
//! permission prompt), X11 (`XGrabKey`) and Wayland through the XDG
//! `org.freedesktop.portal.GlobalShortcuts` portal (GNOME 48+, KDE Plasma 5.27+).
//! On Wayland the portal may ask the user to approve or rebind the shortcut, so
//! the key that fires can differ from the one requested, and `register` returns
//! once the request is sent, not once the user has agreed. Compositors without the
//! portal (Sway, most wlroots desktops) make `register` fail; keep in-window
//! shortcuts as the fallback.

use crate::{shortcut::Shortcut, PlatformError};

/// Grab `keys` (like `"ctrl+alt+space"`; `mod` is Cmd on macOS, Ctrl elsewhere) for
/// the whole desktop. Fails if the combination is invalid, already taken by another
/// program, or unavailable on this display server.
pub fn register(keys: &str) -> Result<(), PlatformError> {
    let sc = Shortcut::parse(keys)
        .ok_or_else(|| PlatformError(format!("global hotkey: cannot parse {keys:?}")))?;
    crate::os::register_hotkey(keys, &sc)
}

/// Release every hotkey registered with [`register`].
pub fn unregister_all() {
    crate::os::unregister_hotkeys();
}

fn fkey(name: &str) -> Option<u32> {
    let n = name.strip_prefix(['f', 'F'])?.parse::<u32>().ok()?;
    (1..=12).contains(&n).then_some(n)
}

/// X11 keysym for a shortcut key name.
#[allow(dead_code)] // only called by the Linux backend
pub(crate) fn x11_keysym(key: &str) -> Option<u32> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c.is_ascii_graphic().then(|| c.to_ascii_lowercase() as u32);
    }
    if let Some(n) = fkey(key) {
        return Some(0xffbe + n - 1);
    }
    Some(match key.to_ascii_lowercase().as_str() {
        "space" => 0x20,
        "enter" | "return" => 0xff0d,
        "escape" | "esc" => 0xff1b,
        "tab" => 0xff09,
        "backspace" => 0xff08,
        "delete" => 0xffff,
        "home" => 0xff50,
        "left" | "arrowleft" => 0xff51,
        "up" | "arrowup" => 0xff52,
        "right" | "arrowright" => 0xff53,
        "down" | "arrowdown" => 0xff54,
        "pageup" => 0xff55,
        "pagedown" => 0xff56,
        "end" => 0xff57,
        _ => return None,
    })
}

/// Windows virtual-key code for a shortcut key name.
#[allow(dead_code)] // only called by the Windows backend
pub(crate) fn windows_vk(key: &str) -> Option<u32> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return c
            .is_ascii_alphanumeric()
            .then(|| c.to_ascii_uppercase() as u32);
    }
    if let Some(n) = fkey(key) {
        return Some(0x70 + n - 1);
    }
    Some(match key.to_ascii_lowercase().as_str() {
        "space" => 0x20,
        "enter" | "return" => 0x0d,
        "escape" | "esc" => 0x1b,
        "tab" => 0x09,
        "backspace" => 0x08,
        "delete" => 0x2e,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" | "arrowleft" => 0x25,
        "up" | "arrowup" => 0x26,
        "right" | "arrowright" => 0x27,
        "down" | "arrowdown" => 0x28,
        _ => return None,
    })
}

/// macOS virtual key code (ANSI layout) for a shortcut key name.
#[allow(dead_code)] // only called by the macOS backend
pub(crate) fn macos_keycode(key: &str) -> Option<u32> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Some(match c.to_ascii_lowercase() {
            'a' => 0,
            's' => 1,
            'd' => 2,
            'f' => 3,
            'h' => 4,
            'g' => 5,
            'z' => 6,
            'x' => 7,
            'c' => 8,
            'v' => 9,
            'b' => 11,
            'q' => 12,
            'w' => 13,
            'e' => 14,
            'r' => 15,
            'y' => 16,
            't' => 17,
            '1' => 18,
            '2' => 19,
            '3' => 20,
            '4' => 21,
            '6' => 22,
            '5' => 23,
            '=' => 24,
            '9' => 25,
            '7' => 26,
            '-' => 27,
            '8' => 28,
            '0' => 29,
            ']' => 30,
            'o' => 31,
            'u' => 32,
            '[' => 33,
            'i' => 34,
            'p' => 35,
            'l' => 37,
            'j' => 38,
            '\'' => 39,
            'k' => 40,
            ';' => 41,
            '\\' => 42,
            ',' => 43,
            '/' => 44,
            'n' => 45,
            'm' => 46,
            '.' => 47,
            '`' => 50,
            _ => return None,
        });
    }
    if let Some(n) = fkey(key) {
        return Some([122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111][n as usize - 1]);
    }
    Some(match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => 36,
        "tab" => 48,
        "space" => 49,
        "backspace" => 51,
        "escape" | "esc" => 53,
        "delete" => 117,
        "home" => 115,
        "end" => 119,
        "pageup" => 116,
        "pagedown" => 121,
        "left" | "arrowleft" => 123,
        "right" | "arrowright" => 124,
        "down" | "arrowdown" => 125,
        "up" | "arrowup" => 126,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysyms() {
        assert_eq!(x11_keysym("A"), Some('a' as u32));
        assert_eq!(x11_keysym("space"), Some(0x20));
        assert_eq!(x11_keysym("F1"), Some(0xffbe));
        assert_eq!(x11_keysym("f12"), Some(0xffc9));
        assert_eq!(x11_keysym("nonsense"), None);
    }

    #[test]
    fn virtual_keys() {
        assert_eq!(windows_vk("k"), Some(0x4b));
        assert_eq!(windows_vk("F5"), Some(0x74));
        assert_eq!(windows_vk("Enter"), Some(0x0d));
        assert_eq!(macos_keycode("s"), Some(1));
        assert_eq!(macos_keycode("space"), Some(49));
        assert_eq!(macos_keycode("F5"), Some(96));
        assert_eq!(macos_keycode("f13"), None);
    }

    #[test]
    fn bad_spec_is_an_error() {
        assert!(register("").is_err());
        assert!(register("ctrl+").is_err());
    }
}
