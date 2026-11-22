//! Input events delivered to widgets by the platform shell.

use crate::geometry::Point;

/// Mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Primary.
    Left,
    /// Secondary.
    Right,
    /// Wheel click.
    Middle,
}

/// Keyboard modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Shift held.
    pub shift: bool,
    /// Control held.
    pub ctrl: bool,
    /// Alt/Option held.
    pub alt: bool,
    /// Cmd/Super/Windows held.
    pub logo: bool,
}

/// A logical key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// A character key.
    Character(String),
    /// Tab.
    Tab,
    /// Enter.
    Enter,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Arrow keys and others, by name.
    Named(String),
}

impl Key {
    /// The key called `name`, in any spelling Alloy accepts: `"down"` and `"ArrowDown"` are the
    /// same key, as are `"esc"`/`"Escape"` and `"return"`/`"Enter"`. A single character is a
    /// [`Key::Character`]; anything else unknown is [`Key::Named`] with the name as given.
    pub fn from_name(name: &str) -> Key {
        let c = canonical_key_name(name);
        match c.as_str() {
            "Enter" => Key::Enter,
            "Escape" => Key::Escape,
            "Tab" => Key::Tab,
            "Backspace" => Key::Backspace,
            _ if c.chars().count() == 1 => Key::Character(c),
            _ => Key::Named(c),
        }
    }
}

/// The one spelling Alloy uses for a key name: `"down"`, `"Down"`, `"arrowdown"` and
/// `"ArrowDown"` all give `"ArrowDown"`; `"esc"` gives `"Escape"`, `"return"` gives `"Enter"`,
/// `"pgdn"` gives `"PageDown"`, `"del"` gives `"Delete"` and `"space"` gives `" "`. Function
/// keys become `"F5"`. Single characters and unknown names are returned unchanged.
pub fn canonical_key_name(name: &str) -> String {
    let lower = name.trim().to_ascii_lowercase();
    let known = match lower.as_str() {
        "down" | "arrowdown" => "ArrowDown",
        "up" | "arrowup" => "ArrowUp",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "esc" | "escape" => "Escape",
        "enter" | "return" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "del" | "delete" => "Delete",
        "pgup" | "pageup" => "PageUp",
        "pgdn" | "pagedown" => "PageDown",
        "home" => "Home",
        "end" => "End",
        "insert" | "ins" => "Insert",
        "space" | "spacebar" => " ",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_string();
    }
    let b = lower.as_bytes();
    if b.len() >= 2 && b[0] == b'f' && lower[1..].chars().all(|c| c.is_ascii_digit()) {
        return lower.to_ascii_uppercase();
    }
    name.to_string()
}

/// A platform-independent input event. Coordinates are logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Cursor moved.
    CursorMoved(Point),
    /// Cursor left the window.
    CursorLeft,
    /// Button pressed.
    MousePressed(MouseButton),
    /// Button released.
    MouseReleased(MouseButton),
    /// Scroll wheel or trackpad, in logical pixels.
    Scrolled {
        /// Horizontal delta.
        dx: f32,
        /// Vertical delta.
        dy: f32,
    },
    /// Key pressed.
    KeyPressed(Key, Modifiers),
    /// Key released.
    KeyReleased(Key, Modifiers),
    /// Committed text input (including IME commits).
    TextInput(String),
    /// IME composition in progress.
    ImePreedit(String),
    /// Window resized to the given logical size.
    Resized(crate::geometry::Size),
    /// Redraw requested.
    RedrawRequested,
    /// Animation/timer tick with monotonic milliseconds since app start.
    /// Delivered only while some widget reports [`Widget::animating`](crate::Widget::animating).
    Tick(u64),
}
