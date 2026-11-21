//! What the host (the platform shell) tells an application about, beyond
//! window input: dropped files, links the OS asked it to open, and the
//! see-through window effect it wants.

use std::path::PathBuf;

/// A see-through look for the window background.
///
/// What each effect becomes depends on the OS; an effect the OS cannot
/// provide leaves the window opaque (the app does not need to care).
///
/// | Effect | Windows 11 | macOS | Linux |
/// |---|---|---|---|
/// | `Mica` | Mica backdrop | window-background vibrancy | transparent (compositor) |
/// | `Acrylic` | Acrylic backdrop | HUD vibrancy | KDE blur (X11), else transparent |
/// | `Blur` | Acrylic backdrop | under-window vibrancy | KDE blur (X11), else transparent |
/// | `Transparent` | see-through | see-through | see-through (compositor) |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WindowEffect {
    /// A normal opaque window.
    #[default]
    None,
    /// Subtle desktop-tinted material (Windows 11 Mica).
    Mica,
    /// Strong frosted glass tinted by the app's background colour.
    Acrylic,
    /// Frosted glass: the desktop behind the window, blurred.
    Blur,
    /// No material at all: whatever the app does not paint shows the desktop.
    Transparent,
}

impl WindowEffect {
    /// Opacity of the page background that works best with this effect when
    /// the OS provides it. `None` is fully opaque.
    pub fn page_alpha(self) -> f32 {
        match self {
            WindowEffect::None => 1.0,
            WindowEffect::Mica => 0.0,
            WindowEffect::Acrylic => 0.62,
            WindowEffect::Blur => 0.5,
            WindowEffect::Transparent => 0.0,
        }
    }
    /// Opacity to use when the OS can make the window see-through but cannot
    /// blur or tint it, so text stays readable.
    pub fn fallback_alpha(self) -> f32 {
        match self {
            WindowEffect::None => 1.0,
            _ => 0.9,
        }
    }
}

/// Something the host noticed that is not window input.
#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    /// Files were dropped on the window from another application.
    FilesDropped(Vec<PathBuf>),
    /// Files are being dragged over the window (`true`) or left it (`false`).
    FilesHovering(bool),
    /// The OS asked this app to open a link of a scheme it registered
    /// (`myapp://invite/42`), possibly because a second launch was forwarded.
    OpenUrl(String),
    /// The OS asked this app to open these files (file association, "Open
    /// With", or a second launch forwarded to this single instance).
    OpenFiles(Vec<PathBuf>),
    /// The user asked to close a window (its close button, Alt+F4, Cmd+W). If the app answers
    /// with a message the window stays open and the message is delivered instead.
    CloseRequested,
    /// The window gained (`true`) or lost (`false`) keyboard focus.
    Focus(bool),
}

/// Host-level settings an application can ask for. Read at start-up and
/// after every message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostConfig {
    /// The see-through window effect wanted.
    pub effect: WindowEffect,
    /// Follow the user's system accent colour.
    pub system_accent: bool,
}
