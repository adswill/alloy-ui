//! `system_settings()` on Linux against a stand-in `gsettings`, so the answer is known.
#![cfg(target_os = "linux")]

use alloy_ui_platform::settings::system_settings;
use std::os::unix::fs::PermissionsExt;

fn fake_gsettings(dir: &std::path::Path, body: &str) {
    let path = dir.join("gsettings");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn reads_settings_through_gsettings() {
    let dir = std::env::temp_dir().join(format!("alloy-gs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("PATH", &dir);
    std::env::remove_var("GTK_THEME");

    // Dark, animations off, purple accent, high contrast on.
    fake_gsettings(
        &dir,
        r#"case "$3" in
 color-scheme) echo "'prefer-dark'";;
 enable-animations) echo false;;
 accent-color) echo "'purple'";;
 high-contrast) echo true;;
 *) exit 1;;
esac"#,
    );
    let s = system_settings();
    assert!(s.dark && s.reduced_motion && s.high_contrast);
    assert_eq!(s.accent, Some([0x91, 0x41, 0xac]));

    // Light, animations on, no accent key (older GNOME), contrast off.
    fake_gsettings(
        &dir,
        r#"case "$3" in
 color-scheme) echo "'default'";;
 enable-animations) echo true;;
 high-contrast) echo false;;
 *) exit 1;;
esac"#,
    );
    let s = system_settings();
    assert!(!s.dark && !s.reduced_motion && !s.high_contrast);
    assert_eq!(s.accent, None);

    // No gsettings at all: everything stays at its default, nothing panics.
    std::fs::remove_file(dir.join("gsettings")).unwrap();
    assert_eq!(system_settings(), Default::default());
    let _ = std::fs::remove_dir_all(&dir);
}
