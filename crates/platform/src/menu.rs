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
    fn shortcut_platform_key() {
        let s = format_shortcut("Mod+S");
        assert!(s == "Cmd+S" || s == "Ctrl+S");
    }
}
