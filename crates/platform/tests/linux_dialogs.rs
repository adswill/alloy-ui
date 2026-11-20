//! Drives the Linux dialog backend against a fake `zenity` on PATH, so the
//! argument handling and output parsing run without a display.
#![cfg(target_os = "linux")]

use alloy_ui_platform::{default_dialogs, Dialogs};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[test]
fn fake_zenity_open_multi_save_cancel() {
    let dir = std::env::temp_dir().join(format!("alloy-fake-zenity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("zenity");
    // Echoes a result chosen by the flags it was given; `--cancel` exits 1.
    std::fs::write(
        &script,
        r#"#!/bin/sh
case "$*" in
  *--cancel*) exit 1 ;;
  *--multiple*) printf '/tmp/a.txt\n/tmp/b c.txt\n' ;;
  *--save*) for a in "$@"; do case "$a" in --filename=*) printf '/out/%s\n' "${a#--filename=}" ;; esac; done ;;
  *) printf '/tmp/one.txt\n' ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", &dir);

    let d = default_dialogs();
    assert_eq!(d.open_file().unwrap(), Some(PathBuf::from("/tmp/one.txt")));
    assert_eq!(
        d.open_files().unwrap(),
        vec![PathBuf::from("/tmp/a.txt"), PathBuf::from("/tmp/b c.txt")]
    );
    assert_eq!(
        d.save_file("notes.txt").unwrap(),
        Some(PathBuf::from("/out/notes.txt"))
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn default_trait_methods() {
    struct Only;
    impl alloy_ui_platform::Dialogs for Only {
        fn open_file(&self) -> Result<Option<PathBuf>, alloy_ui_platform::PlatformError> {
            Ok(Some(PathBuf::from("/x")))
        }
        fn message(&self, _: &str, _: &str) -> Result<(), alloy_ui_platform::PlatformError> {
            Ok(())
        }
    }
    assert_eq!(Only.open_files().unwrap(), vec![PathBuf::from("/x")]);
    assert!(Only.save_file("a").is_err());
}
