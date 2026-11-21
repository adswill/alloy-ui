//! Rich clipboard (HTML, image, files) round trips on X11 and Wayland. Needs a
//! display; skipped otherwise. Cross-checks against `xclip` / `wl-clipboard`
//! when installed, so a foreign app really sees what we put there.
#![cfg(target_os = "linux")]

use alloy_ui_platform::{default_clipboard, ClipContent};
use std::io::Write;
use std::process::{Command, Stdio};

fn have(prog: &str) -> bool {
    Command::new(prog)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
        || Command::new(prog)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
}

fn picture(w: u32, h: u32) -> ClipContent {
    let rgba = (0..w * h)
        .flat_map(|i| [(i % 251) as u8, (i / 7 % 253) as u8, 90, 255])
        .collect();
    ClipContent::image(w, h, rgba).unwrap()
}

#[test]
fn rich_roundtrip() {
    let x = std::env::var_os("DISPLAY").is_some() && std::env::var_os("WAYLAND_DISPLAY").is_none();
    let wl = std::env::var_os("WAYLAND_DISPLAY").is_some();
    if !x && !wl {
        eprintln!("skipped: needs an X11 or Wayland session");
        return;
    }
    let mut cb = default_clipboard();

    // HTML with a plain-text fallback.
    cb.write_rich(&ClipContent::html("<b>bold</b> ü", "bold ü"))
        .unwrap();
    let got = cb.read_rich().unwrap();
    assert_eq!(got.html.as_deref(), Some("<b>bold</b> ü"));
    assert_eq!(got.text.as_deref(), Some("bold ü"));

    // An image big enough to need INCR on X11 (> 64 KiB of PNG, noisy on purpose).
    let mut big = picture(600, 400);
    let img = big.image.as_mut().unwrap();
    let mut s = 0x1234_5678u32;
    for b in img.rgba.iter_mut() {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        *b = (s >> 24) as u8;
    }
    cb.write_rich(&big).unwrap();
    let got = cb.read_rich().unwrap();
    assert_eq!(got.image, big.image, "image survived the clipboard");

    // Files, as a file manager copies them.
    let files = ClipContent::files(["/tmp/a b.txt", "/tmp/ü.png"]);
    cb.write_rich(&files).unwrap();
    assert_eq!(cb.read_rich().unwrap().files, files.files);

    // A foreign client sees the formats we offer.
    if x && have("xclip") {
        cb.write_rich(&ClipContent::html("<i>hi</i>", "hi")).unwrap();
        let out = |target: &str| {
            Command::new("xclip")
                .args(["-selection", "clipboard", "-t", target, "-o"])
                .output()
                .unwrap()
                .stdout
        };
        assert_eq!(out("text/html"), b"<i>hi</i>");
        assert_eq!(out("UTF8_STRING"), b"hi");
        cb.write_rich(&big).unwrap();
        let png = out("image/png");
        assert_eq!(&png[..4], b"\x89PNG");
        assert_eq!(
            alloy_ui_platform::clipboard::decode_png(&png).map(|i| i.rgba),
            big.image.as_ref().map(|i| i.rgba.clone())
        );

        // And we read what a foreign client owns: HTML put there by xclip.
        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "text/html", "-i"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"<u>from xclip</u>")
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(
            cb.read_rich().unwrap().html.as_deref(),
            Some("<u>from xclip</u>")
        );
        let _ = child.kill();
    }
    if wl && have("wl-paste") {
        cb.write_rich(&ClipContent::html("<i>hi</i>", "hi")).unwrap();
        let out = Command::new("wl-paste")
            .args(["--no-newline", "--type", "text/html"])
            .output()
            .unwrap()
            .stdout;
        assert_eq!(out, b"<i>hi</i>");
    }
}
