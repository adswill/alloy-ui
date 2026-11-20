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
}
