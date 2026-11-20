//! Native X11 clipboard round trips. Needs an X server (`DISPLAY`); skipped
//! otherwise. Cross-checks against `xclip` when it is installed.
#![cfg(target_os = "linux")]

use alloy_ui_platform::default_clipboard;
use std::io::Write;
use std::process::{Command, Stdio};

fn have(prog: &str) -> bool {
    Command::new(prog)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

#[test]
fn x11_clipboard() {
    if std::env::var_os("DISPLAY").is_none() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        eprintln!("skipped: needs an X11 session");
        return;
    }
    // Make sure helper tools cannot be what answers: hide them from PATH.
    let real_path = std::env::var_os("PATH");
    let xclip = have("xclip");
    std::env::set_var("PATH", "");

    let mut cb = default_clipboard();
    cb.write_text("héllo wörld ✓").unwrap();
    assert_eq!(cb.read_text().unwrap().as_deref(), Some("héllo wörld ✓"));

    // A second write replaces the first (the old owner gets SelectionClear).
    cb.write_text("second").unwrap();
    assert_eq!(cb.read_text().unwrap().as_deref(), Some("second"));

    if xclip {
        std::env::set_var("PATH", real_path.as_ref().unwrap());
        // xclip reads what we own.
        let out = Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "second");

        // We read what xclip owns, including a value large enough to use INCR.
        let big = "x".repeat(2 * 1024 * 1024);
        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard", "-i"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(big.as_bytes())
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::env::set_var("PATH", "");
        assert_eq!(cb.read_text().unwrap().map(|s| s.len()), Some(big.len()));
        child.kill().ok();
        child.wait().ok();
        std::env::set_var("PATH", real_path.unwrap());
    }
}
