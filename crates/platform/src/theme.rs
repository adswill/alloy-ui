//! Parsers for each OS's dark-mode setting. Pure functions, tested on every OS.

/// macOS: `defaults read -g AppleInterfaceStyle` prints `Dark` in dark mode
/// and fails (so `None`) in light mode.
pub fn parse_macos(out: Option<&str>) -> bool {
    out.is_some_and(|s| s.trim().eq_ignore_ascii_case("dark"))
}

/// Windows: `reg query ... /v AppsUseLightTheme` prints `0x0` for dark, `0x1` for light.
pub fn parse_windows(out: Option<&str>) -> Option<bool> {
    let line = out?.lines().find(|l| l.contains("AppsUseLightTheme"))?;
    let value = line.split_whitespace().last()?;
    let n = u32::from_str_radix(value.trim_start_matches("0x"), 16).ok()?;
    Some(n == 0)
}

/// Linux: GNOME `color-scheme` (`'prefer-dark'`, `'default'`, `'prefer-light'`),
/// else a `GTK_THEME` name ending in `:dark` or containing `dark`.
pub fn parse_linux(color_scheme: Option<&str>, gtk_theme: Option<&str>) -> Option<bool> {
    if let Some(s) = color_scheme {
        let s = s.trim().trim_matches('\'');
        match s {
            "prefer-dark" => return Some(true),
            "prefer-light" => return Some(false),
            _ => {}
        }
    }
    gtk_theme.map(|t| t.to_ascii_lowercase().contains("dark"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos() {
        assert!(parse_macos(Some("Dark\n")));
        assert!(!parse_macos(None));
    }

    #[test]
    fn windows() {
        let dark = "\r\nHKEY_CURRENT_USER\\...\\Personalize\r\n    AppsUseLightTheme    REG_DWORD    0x0\r\n";
        assert_eq!(parse_windows(Some(dark)), Some(true));
        assert_eq!(
            parse_windows(Some(&dark.replace("0x0", "0x1"))),
            Some(false)
        );
        assert_eq!(parse_windows(None), None);
    }

    #[test]
    fn linux() {
        assert_eq!(parse_linux(Some("'prefer-dark'\n"), None), Some(true));
        assert_eq!(parse_linux(Some("'prefer-light'"), None), Some(false));
        assert_eq!(
            parse_linux(Some("'default'"), Some("Adwaita:dark")),
            Some(true)
        );
        assert_eq!(parse_linux(None, None), None);
    }
}
