//! Keyboard shortcuts for menu items: parsing (`"Mod+Shift+S"`) and matching
//! against key events. `Mod` is Cmd on macOS and Ctrl elsewhere.

use alloy_ui_core::{Key, Modifiers};

use crate::menu::{Menu, MenuItem};

/// A parsed shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    /// Required modifiers (exactly these must be held).
    pub mods: Modifiers,
    /// The non-modifier key, lowercase for characters (`"s"`, `"Enter"`, `"F5"`).
    pub key: String,
}

impl Shortcut {
    /// Parse text like `"Mod+Shift+S"`, `"Ctrl+Enter"`, `"Alt+F4"`. Returns
    /// `None` for empty input, unknown modifiers, or a missing key.
    pub fn parse(s: &str) -> Option<Self> {
        let mut mods = Modifiers::default();
        let mut key = None;
        for part in s.split('+').map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "" => return None,
                "mod" => {
                    if cfg!(target_os = "macos") {
                        mods.logo = true
                    } else {
                        mods.ctrl = true
                    }
                }
                "ctrl" | "control" => mods.ctrl = true,
                "shift" => mods.shift = true,
                "alt" | "option" => mods.alt = true,
                "cmd" | "super" | "win" | "meta" => mods.logo = true,
                _ if key.is_none() => key = Some(part.to_string()),
                _ => return None,
            }
        }
        let key = key?;
        // One spelling per key: "down" and "ArrowDown" are the same shortcut.
        let key = alloy_ui_core::canonical_key_name(&key);
        // The space bar keeps its name: the global-hotkey backends and `trigger` match on "space".
        let key = if key == " " {
            "space".to_string()
        } else if key.chars().count() == 1 {
            key.to_lowercase()
        } else {
            key
        };
        Some(Self { mods, key })
    }

    /// True when `key` pressed with `mods` triggers this shortcut.
    pub fn matches(&self, key: &Key, mods: &Modifiers) -> bool {
        if self.mods != *mods {
            return false;
        }
        match key {
            Key::Character(c) => {
                let c = c.to_lowercase();
                c == self.key || (c == " " && self.key.eq_ignore_ascii_case("space"))
            }
            Key::Enter => self.key.eq_ignore_ascii_case("enter"),
            Key::Escape => {
                self.key.eq_ignore_ascii_case("escape") || self.key.eq_ignore_ascii_case("esc")
            }
            Key::Tab => self.key.eq_ignore_ascii_case("tab"),
            Key::Backspace => self.key.eq_ignore_ascii_case("backspace"),
            Key::Named(n) => alloy_ui_core::canonical_key_name(n)
                .eq_ignore_ascii_case(&alloy_ui_core::canonical_key_name(&self.key)),
        }
    }
}

/// Collect `(shortcut, action id)` pairs from a menu bar, skipping disabled
/// items and shortcuts that fail to parse.
pub fn collect(menus: &[Menu]) -> Vec<(Shortcut, String)> {
    fn walk(items: &[MenuItem], out: &mut Vec<(Shortcut, String)>) {
        for i in items {
            match i {
                MenuItem::Action {
                    id,
                    shortcut: Some(s),
                    enabled: true,
                    ..
                } => {
                    if let Some(sc) = Shortcut::parse(s) {
                        out.push((sc, id.clone()));
                    }
                }
                MenuItem::Submenu(m) => walk(&m.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for m in menus {
        walk(&m.items, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }

    /// The global-hotkey backends and the Linux `trigger` string match on the name "space", so
    /// key-name aliasing must not turn it into " ".
    #[test]
    fn space_keeps_its_name_and_still_matches_the_space_bar() {
        let sc = Shortcut::parse("ctrl+alt+space").unwrap();
        assert_eq!(sc.key, "space");
        let mods = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        };
        assert!(sc.matches(&ch(" "), &mods));
        assert!(sc.matches(&Key::Named("Space".into()), &mods));
    }

    #[test]
    fn parse_and_match() {
        let s = Shortcut::parse("Ctrl+Shift+S").unwrap();
        let m = Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        assert!(s.matches(&ch("S"), &m));
        assert!(s.matches(&ch("s"), &m));
        assert!(!s.matches(
            &ch("s"),
            &Modifiers {
                ctrl: true,
                ..Default::default()
            }
        ));
        assert!(!s.matches(&ch("x"), &m));
    }

    #[test]
    fn named_keys() {
        let s = Shortcut::parse("Alt+F4").unwrap();
        let alt = Modifiers {
            alt: true,
            ..Default::default()
        };
        assert!(s.matches(&Key::Named("F4".into()), &alt));
        let e = Shortcut::parse("Ctrl+Enter").unwrap();
        assert!(e.matches(
            &Key::Enter,
            &Modifiers {
                ctrl: true,
                ..Default::default()
            }
        ));
    }

    #[test]
    fn mod_is_platform_key() {
        let s = Shortcut::parse("Mod+N").unwrap();
        assert_eq!(s.mods.logo, cfg!(target_os = "macos"));
        assert_eq!(s.mods.ctrl, !cfg!(target_os = "macos"));
    }

    #[test]
    fn rejects_bad() {
        assert!(Shortcut::parse("").is_none());
        assert!(Shortcut::parse("Ctrl+").is_none());
        assert!(Shortcut::parse("Ctrl").is_none());
        assert!(Shortcut::parse("A+B").is_none());
    }

    #[test]
    fn collect_skips_disabled() {
        let m = Menu::new("File")
            .action("new", "New", Some("Ctrl+N"))
            .action("x", "X", None);
        let v = collect(&[m]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "new");
    }
}
