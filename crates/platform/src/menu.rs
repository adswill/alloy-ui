//! Menu bar and context-menu model. The model is plain data so apps can build
//! menus the same way on every OS and tests can inspect them.
//!
//! The model is installed with [`ShellContext::set_menu`](crate::ShellContext::set_menu):
//! NSMenu on macOS, an HMENU on Windows, and the desktop's global menu over D-Bus on Linux
//! (KDE, Unity, MATE; GNOME has no menu bar). [`MenuBackend`] is a separate, unused hook.

use crate::PlatformError;

/// One entry in a menu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    /// A clickable action; `id` is reported back when chosen.
    Action {
        /// Stable id reported on activation.
        id: String,
        /// Label shown to the user.
        label: String,
        /// Optional shortcut such as `"Cmd+S"` (written with `Mod` for the platform key).
        shortcut: Option<String>,
        /// Greyed out when false.
        enabled: bool,
    },
    /// A divider line.
    Separator,
    /// A nested menu.
    Submenu(Menu),
}

/// A titled list of items.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Menu {
    /// Title.
    pub title: String,
    /// Entries.
    pub items: Vec<MenuItem>,
}

impl Menu {
    /// New empty menu.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            items: Vec::new(),
        }
    }
    /// Append an enabled action.
    pub fn action(mut self, id: &str, label: &str, shortcut: Option<&str>) -> Self {
        self.items.push(MenuItem::Action {
            id: id.into(),
            label: label.into(),
            shortcut: shortcut.map(Into::into),
            enabled: true,
        });
        self
    }
    /// Append a separator.
    pub fn separator(mut self) -> Self {
        self.items.push(MenuItem::Separator);
        self
    }
    /// Append a submenu.
    pub fn submenu(mut self, m: Menu) -> Self {
        self.items.push(MenuItem::Submenu(m));
        self
    }
    /// Find an action's label by id, searching submenus.
    pub fn find(&self, id: &str) -> Option<&str> {
        self.items.iter().find_map(|i| match i {
            MenuItem::Action { id: i, label, .. } if i == id => Some(label.as_str()),
            MenuItem::Submenu(m) => m.find(id),
            _ => None,
        })
    }
}

/// Render a shortcut for display on the current OS: `Mod` becomes `Cmd` on
/// macOS and `Ctrl` elsewhere.
pub fn format_shortcut(s: &str) -> String {
    s.replace(
        "Mod",
        if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        },
    )
}

/// A shortcut split into what a native menu item needs: the key it shows and its modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    /// The key as AppKit-style key-equivalent text: one lowercase character, or a control
    /// character for Enter, Tab, Escape, Backspace and Space.
    pub key: String,
    /// Command (macOS) / Super.
    pub cmd: bool,
    /// Control.
    pub ctrl: bool,
    /// Option / Alt.
    pub alt: bool,
    /// Shift.
    pub shift: bool,
}

/// Split `"mod+ctrl+f"` into a [`Chord`]. Every modifier is kept (the macOS menu used to keep
/// only the last key and imply Cmd). `None` for an empty or unparsable shortcut.
pub fn chord(shortcut: &str) -> Option<Chord> {
    let sc = crate::shortcut::Shortcut::parse(shortcut)?;
    let key = match sc.key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "\r".to_string(),
        "tab" => "\t".to_string(),
        "escape" | "esc" => "\u{1b}".to_string(),
        "backspace" => "\u{8}".to_string(),
        "space" => " ".to_string(),
        k if k.chars().count() == 1 => k.to_string(),
        _ => String::new(),
    };
    Some(Chord {
        key,
        cmd: sc.mods.logo,
        ctrl: sc.mods.ctrl,
        alt: sc.mods.alt,
        shift: sc.mods.shift,
    })
}

/// Menu ids that start with this prefix are standard items: the platform may run them natively
/// (Hide, Hide Others, Show All, About on macOS), and the rest are replayed as the key press
/// [`standard_shortcut`] names, so the focused widget handles Cut/Copy/Paste/Select All itself.
pub const STANDARD_PREFIX: &str = "std:";

/// The key press a standard item stands for, if it is one that is replayed to the focused widget.
pub fn standard_shortcut(id: &str) -> Option<&'static str> {
    Some(match id {
        "std:cut" => "mod+x",
        "std:copy" => "mod+c",
        "std:paste" => "mod+v",
        "std:select_all" => "mod+a",
        _ => return None,
    })
}

/// Installs menus into the OS.
pub trait MenuBackend {
    /// Install `menus` as the application menu bar.
    fn install(&mut self, menus: &[Menu]) -> Result<(), PlatformError>;
}

/// Placeholder backend that reports menus as unsupported.
pub struct UnsupportedMenu;

impl MenuBackend for UnsupportedMenu {
    fn install(&mut self, _: &[Menu]) -> Result<(), PlatformError> {
        Err(PlatformError("native menu bar not implemented yet".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_find() {
        let m = Menu::new("File")
            .action("new", "New", Some("Mod+N"))
            .separator()
            .submenu(Menu::new("Recent").action("r1", "a.txt", None));
        assert_eq!(m.find("new"), Some("New"));
        assert_eq!(m.find("r1"), Some("a.txt"));
        assert_eq!(m.find("zz"), None);
    }

    #[test]
    fn chords_keep_every_modifier() {
        let c = chord("mod+ctrl+f").unwrap();
        assert_eq!(c.key, "f");
        assert!(c.ctrl && !c.alt && !c.shift);
        assert_eq!(c.cmd, cfg!(target_os = "macos"));
        let c = chord("alt+shift+Enter").unwrap();
        assert_eq!(c.key, "\r");
        assert!(c.alt && c.shift);
        assert_eq!(chord(""), None);
        assert_eq!(standard_shortcut("std:copy"), Some("mod+c"));
    }

    #[test]
    fn shortcut_platform_key() {
        let s = format_shortcut("Mod+S");
        assert!(s == "Cmd+S" || s == "Ctrl+S");
    }
}
