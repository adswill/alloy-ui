//! End-to-end: create a real app crate, build it in release mode and bundle
//! it with the formats that work on the host OS. Bundlers that need external
//! tools (appimagetool, makensis, hdiutil) are exercised in CI instead.

use alloy_ui_cli::run;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
        }
    }
    !crc
}

/// A valid solid-colour RGBA PNG of `size` x `size`.
fn png(size: u32) -> Vec<u8> {
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;
    let mut raw = Vec::new();
    for _ in 0..size {
        // Filter byte 0 (none), then one RGBA pixel per column.
        raw.extend(std::iter::once(0u8));
        for _ in 0..size {
            raw.extend_from_slice(&[0x2d, 0x6c, 0xdf, 0xff]);
        }
    }
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&raw).unwrap();
    let idat = z.finish().unwrap();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&size.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    for (ty, data) in [
        (&b"IHDR"[..], &ihdr[..]),
        (b"IDAT", &idat[..]),
        (b"IEND", &[][..]),
    ] {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = ty.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    out
}

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn make_app(caps: &str) -> (Sandbox, PathBuf) {
    let dir = std::env::temp_dir().join(format!("alloy-e2e-{}-{}", std::process::id(), caps.len()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        r#"[package]
name = "hello-alloy"
version = "0.3.1"
edition = "2021"
description = "End to end fixture"
authors = ["Test <t@example.com>"]

[package.metadata.alloy]
identifier = "dev.example.hello"
product-name = "Hello Alloy"
publisher = "Example"
icons = ["assets/icon-256.png", "assets/icon-512.png"]
capabilities = "alloy.capabilities.toml"
"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/main.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
    std::fs::write(dir.join("assets/icon-256.png"), png(256)).unwrap();
    std::fs::write(dir.join("assets/icon-512.png"), png(512)).unwrap();
    std::fs::write(dir.join("alloy.capabilities.toml"), caps).unwrap();
    let manifest = dir.join("Cargo.toml");
    (Sandbox(dir), manifest)
}

fn args(manifest: &Path, out: &Path, extra: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = ["bundle", "--manifest-path"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    v.push(manifest.display().to_string());
    v.push("--out".into());
    v.push(out.display().to_string());
    v.extend(extra.iter().map(|s| s.to_string()));
    v
}

#[test]
fn png_helper_is_valid() {
    assert_eq!(alloy_ui_cli::icon::png_size(&png(64)), Some((64, 64)));
}

#[test]
fn check_reports_invalid_capabilities() {
    let (_sb, manifest) = make_app("capabilities = [\"teleport\"]");
    let e = run(&[
        "check".into(),
        "--manifest-path".into(),
        manifest.display().to_string(),
    ])
    .unwrap_err();
    assert!(e.to_string().contains("capability config"), "{e}");
}

#[cfg(target_os = "linux")]
#[test]
fn builds_and_bundles_deb_on_linux() {
    let (sb, manifest) = make_app("capabilities = [\"clipboard\", \"dialogs\"]\n");
    let out = sb.0.join("dist");
    run(&args(&manifest, &out, &["--format", "deb"])).unwrap();
    let deb = out.join("hello-alloy_0.3.1_amd64.deb");
    assert!(deb.is_file(), "{deb:?}");

    // If dpkg-deb exists (Debian/Ubuntu), it is the authority on validity.
    if Command::new("dpkg-deb").arg("--version").output().is_ok() {
        let info = Command::new("dpkg-deb")
            .arg("-I")
            .arg(&deb)
            .output()
            .unwrap();
        assert!(
            info.status.success(),
            "{}",
            String::from_utf8_lossy(&info.stderr)
        );
        let info = String::from_utf8_lossy(&info.stdout);
        assert!(info.contains("Package: hello-alloy"), "{info}");
        assert!(info.contains("Architecture: amd64"));

        let list = Command::new("dpkg-deb")
            .arg("-c")
            .arg(&deb)
            .output()
            .unwrap();
        let list = String::from_utf8_lossy(&list.stdout);
        for want in [
            "./usr/bin/hello-alloy",
            "./usr/share/hello-alloy/alloy.capabilities.toml",
            "./usr/share/applications/dev.example.hello.desktop",
            "./usr/share/icons/hicolor/256x256/apps/hello-alloy.png",
        ] {
            assert!(list.contains(want), "missing {want} in\n{list}");
        }

        // Extract and run the packaged binary.
        let x = sb.0.join("extract");
        let st = Command::new("dpkg-deb")
            .arg("-x")
            .arg(&deb)
            .arg(&x)
            .status()
            .unwrap();
        assert!(st.success());
        let run_out = Command::new(x.join("usr/bin/hello-alloy"))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&run_out.stdout).trim(), "hi");
        let caps = std::fs::read_to_string(x.join("usr/share/hello-alloy/alloy.capabilities.toml"))
            .unwrap();
        assert!(caps.contains("clipboard"));
    }

    // AppDir layout is checked even without appimagetool installed; CI sets
    // ALLOY_REQUIRE_APPIMAGE so the full AppImage must build there.
    let r = run(&args(
        &manifest,
        &out,
        &["--skip-build", "--format", "appimage"],
    ));
    assert!(out.join("hello-alloy-0.3.1-x86_64.AppDir/AppRun").is_file());
    if std::env::var_os("ALLOY_REQUIRE_APPIMAGE").is_some() {
        r.unwrap();
        let img = out.join("hello-alloy-x86_64.AppImage");
        assert!(img.is_file());
        // An AppImage is an ELF followed by a squashfs payload.
        let head = std::fs::read(&img).unwrap();
        assert_eq!(&head[..4], b"\x7fELF");
        let o = Command::new(&img)
            .env("APPIMAGE_EXTRACT_AND_RUN", "1")
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "hi");
    } else if let Err(e) = r {
        assert!(e.to_string().contains("appimagetool"), "{e}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn builds_and_bundles_app_and_dmg_on_macos() {
    let (sb, manifest) = make_app("capabilities = [\"clipboard\"]\n");
    let out = sb.0.join("dist");
    run(&args(&manifest, &out, &["--format", "app,dmg"])).unwrap();
    let app = out.join("Hello Alloy.app");
    let bin = app.join("Contents/MacOS/hello-alloy");
    let o = Command::new(&bin).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "hi");
    assert!(app.join("Contents/Resources/icon.icns").is_file());
    let v = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&app)
        .status()
        .unwrap();
    assert!(v.success(), "ad-hoc signature must verify");
    let plist = Command::new("plutil")
        .arg("-lint")
        .arg(app.join("Contents/Info.plist"))
        .status()
        .unwrap();
    assert!(plist.success());
    assert!(std::fs::read_dir(&out).unwrap().any(|e| e
        .unwrap()
        .path()
        .extension()
        .is_some_and(|x| x == "dmg")));
}

#[cfg(target_os = "windows")]
#[test]
fn builds_and_bundles_exe_on_windows() {
    let (sb, manifest) = make_app("capabilities = [\"clipboard\"]\n");
    let out = sb.0.join("dist");
    run(&args(&manifest, &out, &["--format", "exe"])).unwrap();
    let exe = out
        .join("hello-alloy-0.3.1-x86_64-portable")
        .join("hello-alloy.exe");
    let o = Command::new(&exe).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "hi");
    // Installer needs makensis; CI installs it, so require it there.
    if std::env::var_os("ALLOY_REQUIRE_NSIS").is_some() {
        run(&args(
            &manifest,
            &out,
            &["--skip-build", "--format", "nsis"],
        ))
        .unwrap();
        assert!(out.join("hello-alloy-0.3.1-x86_64-setup.exe").is_file());
    }
}
