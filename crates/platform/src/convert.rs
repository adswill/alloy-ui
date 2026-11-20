//! Pure translation from `winit` types to Alloy's `core::Event` types.
//! Kept free of window state so it is unit-testable on any OS.

use alloy_ui_core::{Key, Modifiers, MouseButton};
use winit::event::MouseButton as WMouse;
use winit::keyboard::{Key as WKey, ModifiersState, NamedKey};

/// Convert modifier state.
pub fn modifiers(m: ModifiersState) -> Modifiers {
    Modifiers {
        shift: m.shift_key(),
        ctrl: m.control_key(),
        alt: m.alt_key(),
        logo: m.super_key(),
    }
}

/// Convert a mouse button; extra buttons are ignored.
pub fn mouse_button(b: WMouse) -> Option<MouseButton> {
    match b {
        WMouse::Left => Some(MouseButton::Left),
        WMouse::Right => Some(MouseButton::Right),
        WMouse::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}

/// Convert a logical key. Returns `None` for keys with no meaning to widgets
/// (modifier-only keys, dead keys, unidentified).
pub fn key(k: &WKey) -> Option<Key> {
    Some(match k {
        WKey::Character(s) => Key::Character(s.to_string()),
        WKey::Named(NamedKey::Tab) => Key::Tab,
        WKey::Named(NamedKey::Enter) => Key::Enter,
        WKey::Named(NamedKey::Escape) => Key::Escape,
        WKey::Named(NamedKey::Backspace) => Key::Backspace,
        WKey::Named(NamedKey::Space) => Key::Character(" ".into()),
        WKey::Named(
            NamedKey::Shift
            | NamedKey::Control
            | NamedKey::Alt
            | NamedKey::Super
            | NamedKey::Meta
            | NamedKey::CapsLock
            | NamedKey::NumLock,
        ) => return None,
        WKey::Named(n) => Key::Named(format!("{n:?}")),
        WKey::Dead(_) | WKey::Unidentified(_) => return None,
    })
}

/// Pixels per line used when a device reports line-based scrolling.
pub const LINE_HEIGHT: f32 = 40.0;

/// Convert a scroll delta to logical pixels. Positive `dy` scrolls content
/// down (toward later content), matching the natural-scrolling convention
/// reported by the OS.
pub fn scroll(delta: winit::event::MouseScrollDelta, scale: f64) -> (f32, f32) {
    use winit::event::MouseScrollDelta::*;
    match delta {
        LineDelta(x, y) => (x * LINE_HEIGHT, y * LINE_HEIGHT),
        PixelDelta(p) => ((p.x / scale) as f32, (p.y / scale) as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseScrollDelta;

    #[test]
    fn modifiers_map() {
        let m = modifiers(ModifiersState::SHIFT | ModifiersState::SUPER);
        assert!(m.shift && m.logo && !m.ctrl && !m.alt);
    }

    #[test]
    fn buttons_map() {
        assert_eq!(mouse_button(WMouse::Left), Some(MouseButton::Left));
        assert_eq!(mouse_button(WMouse::Back), None);
    }

    #[test]
    fn keys_map() {
        assert_eq!(key(&WKey::Named(NamedKey::Tab)), Some(Key::Tab));
        assert_eq!(
            key(&WKey::Character("a".into())),
            Some(Key::Character("a".into()))
        );
        assert_eq!(key(&WKey::Named(NamedKey::Shift)), None);
        assert_eq!(
            key(&WKey::Named(NamedKey::Space)),
            Some(Key::Character(" ".into()))
        );
        assert_eq!(
            key(&WKey::Named(NamedKey::ArrowLeft)),
            Some(Key::Named("ArrowLeft".into()))
        );
    }

    #[test]
    fn scroll_units() {
        assert_eq!(
            scroll(MouseScrollDelta::LineDelta(0.0, 2.0), 1.0),
            (0.0, 80.0)
        );
        let d = MouseScrollDelta::PixelDelta(PhysicalPosition::new(20.0, 40.0));
        assert_eq!(scroll(d, 2.0), (10.0, 20.0));
    }
}
