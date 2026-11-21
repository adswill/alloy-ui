//! System appearance and accessibility settings: dark mode, reduced motion,
//! high contrast and the user's accent colour. Pure parsers for each OS's
//! answers live here and are tested on every OS; the backends in `os/` fetch
//! the raw values.

/// What the OS says about appearance and accessibility right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SystemSettings {
    /// The OS is in dark mode.
    pub dark: bool,
    /// The user asked for less motion (macOS "Reduce motion", Windows
    /// "Animation effects" off, GNOME "Animations" off).
    pub reduced_motion: bool,
    /// The user turned on a high contrast mode.
    pub high_contrast: bool,
    /// The user's accent colour as `[r, g, b]`, where the OS has one.
    pub accent: Option<[u8; 3]>,
}

/// Read the current settings. Spawns a few short helper processes on Linux and
/// macOS, so call it off the UI thread or rarely (the shell does both).
pub fn system_settings() -> SystemSettings {
    crate::os::read_settings()
}

/// GNOME's named accent colours (`org.gnome.desktop.interface accent-color`).
pub fn parse_gnome_accent(out: Option<&str>) -> Option<[u8; 3]> {
    Some(match out?.trim().trim_matches('\'') {
        "blue" => [0x35, 0x84, 0xe4],
        "teal" => [0x21, 0x90, 0xa4],
        "green" => [0x3a, 0x94, 0x4a],
        "yellow" => [0xc8, 0x88, 0x00],
        "orange" => [0xed, 0x5b, 0x00],
        "red" => [0xe6, 0x2d, 0x42],
        "pink" => [0xd5, 0x61, 0x99],
        "purple" => [0x91, 0x41, 0xac],
        "slate" => [0x6f, 0x83, 0x96],
        _ => return None,
    })
}

/// A `gsettings get` boolean (`true` / `false`).
pub fn parse_gsettings_bool(out: Option<&str>) -> Option<bool> {
    match out?.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Windows `DWM\AccentColor`: a `0xAABBGGRR` DWORD.
pub fn parse_windows_accent(abgr: u32) -> [u8; 3] {
    [abgr as u8, (abgr >> 8) as u8, (abgr >> 16) as u8]
}

/// macOS `defaults read -g AppleAccentColor`. The key is absent for the
/// default multicolour accent, which we show as blue.
pub fn parse_macos_accent(out: Option<&str>) -> Option<[u8; 3]> {
    let n: i32 = match out {
        None => return Some([0x00, 0x7a, 0xff]),
        Some(s) => s.trim().parse().ok()?,
    };
    Some(match n {
        -1 => [0x98, 0x98, 0x98], // graphite
        0 => [0xff, 0x52, 0x57],  // red
        1 => [0xf7, 0x82, 0x1b],  // orange
        2 => [0xff, 0xc6, 0x00],  // yellow
        3 => [0x62, 0xba, 0x46],  // green
        4 => [0xa5, 0x50, 0xa7],  // purple
        5 => [0xf7, 0x4f, 0x9e],  // pink
        _ => [0x00, 0x7a, 0xff],  // blue
    })
}

/// macOS `defaults read com.apple.universalaccess reduceMotion` (`1` when on).
pub fn parse_macos_flag(out: Option<&str>) -> bool {
    out.is_some_and(|s| matches!(s.trim(), "1" | "true" | "YES"))
}

/// One line of `gsettings monitor org.gnome.desktop.interface`: true when it
/// is about a key that changes [`SystemSettings`].
pub fn is_relevant_monitor_line(line: &str) -> bool {
    ["color-scheme:", "accent-color:", "enable-animations:"]
        .iter()
        .any(|k| line.starts_with(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome() {
        assert_eq!(parse_gnome_accent(Some("'purple'\n")), Some([0x91, 0x41, 0xac]));
        assert_eq!(parse_gnome_accent(Some("'nope'")), None);
        assert_eq!(parse_gnome_accent(None), None);
        assert_eq!(parse_gsettings_bool(Some("false\n")), Some(false));
        assert_eq!(parse_gsettings_bool(Some("true")), Some(true));
        assert_eq!(parse_gsettings_bool(Some("'x'")), None);
    }

    #[test]
    fn windows() {
        // 0xFF (alpha) B=0xD4 G=0x78 R=0x00 is the classic Windows blue 0078D4.
        assert_eq!(parse_windows_accent(0xFFD4_7800), [0x00, 0x78, 0xD4]);
    }

    #[test]
    fn macos() {
        assert_eq!(parse_macos_accent(None), Some([0x00, 0x7a, 0xff]));
        assert_eq!(parse_macos_accent(Some("3\n")), Some([0x62, 0xba, 0x46]));
        assert_eq!(parse_macos_accent(Some("-1")), Some([0x98, 0x98, 0x98]));
        assert!(parse_macos_flag(Some("1\n")));
        assert!(!parse_macos_flag(Some("0")));
        assert!(!parse_macos_flag(None));
    }

    #[test]
    fn monitor_lines() {
        assert!(is_relevant_monitor_line("accent-color: 'red'"));
        assert!(is_relevant_monitor_line("enable-animations: false"));
        assert!(!is_relevant_monitor_line("font-name: 'Cantarell 11'"));
    }
}
