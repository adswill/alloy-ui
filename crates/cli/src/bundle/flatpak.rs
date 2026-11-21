//! Flatpak: generates a `flatpak-builder` source directory (JSON manifest,
//! desktop file, AppStream metainfo, icons, the binary) and, when
//! `flatpak-builder` is installed, builds a single-file `.flatpak` bundle.
//!
//! The binary is built outside the sandbox and copied in, so it must run on the
//! freedesktop runtime's glibc. Build it on an old enough distribution (the CI
//! recipe in the guide uses Ubuntu 22.04). Filesystem and network access come
//! from the app's capability config; see [`finish_args`].

use super::linux::require_linux;
use super::{copy_executable, remove_if_exists, run, which, write_file, Ctx, CAPABILITIES_FILE};
use crate::error::{Error, Result};
use alloy_ui_ipc::{Capability, CapabilitySet};
use std::path::PathBuf;
use std::process::Command;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `finish-args` (the sandbox permissions). `[package.metadata.alloy]
/// flatpak-finish-args` replaces the derived list entirely.
pub fn finish_args(ctx: &Ctx<'_>) -> Vec<String> {
    if let Some(a) = &ctx.project.flatpak_finish_args {
        return a.clone();
    }
    let set = ctx
        .capabilities
        .as_deref()
        .and_then(|t| CapabilitySet::from_toml(t).ok())
        .unwrap_or_else(CapabilitySet::none);
    let mut a: Vec<String> = [
        "--socket=wayland",
        "--socket=fallback-x11",
        "--share=ipc",
        "--device=dri",
        // The Alloy tray is a StatusNotifierItem.
        "--talk-name=org.kde.StatusNotifierWatcher",
    ]
    .map(String::from)
    .into();
    if set.has(Capability::Network) {
        a.push("--share=network".into());
    }
    if set.has(Capability::Notifications) {
        a.push("--talk-name=org.freedesktop.Notifications".into());
    }
    // File dialogs go through the portal and need no static access. Direct
    // reads and writes outside a dialog do.
    if set.has(Capability::FsWrite) {
        a.push("--filesystem=home".into());
    } else if set.has(Capability::FsRead) {
        a.push("--filesystem=home:ro".into());
    }
    a
}

/// Flatpak requires the desktop file, icons and metainfo to be named by app id.
fn desktop_entry(ctx: &Ctx<'_>) -> String {
    let p = ctx.project;
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nIcon={}\nTerminal=false\nCategories={};\nStartupWMClass={}\n",
        p.product_name,
        p.description.split_whitespace().collect::<Vec<_>>().join(" "),
        p.bin_name,
        p.identifier,
        p.category,
        p.bin_name,
    )
}

/// AppStream metadata (`<id>.metainfo.xml`).
pub fn metainfo(ctx: &Ctx<'_>) -> String {
    let p = ctx.project;
    let license = p.license.as_deref().unwrap_or("LicenseRef-proprietary");
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<component type=\"desktop-application\">\n  <id>{id}</id>\n  <name>{name}</name>\n  <summary>{summary}</summary>\n  <metadata_license>CC0-1.0</metadata_license>\n  <project_license>{license}</project_license>\n  <description>\n    <p>{summary}</p>\n  </description>\n  <launchable type=\"desktop-id\">{id}.desktop</launchable>\n",
        id = esc(&p.identifier),
        name = esc(&p.product_name),
        summary = esc(&p.description.split_whitespace().collect::<Vec<_>>().join(" ")),
        license = esc(license),
    );
    if let Some(h) = &p.homepage {
        s.push_str(&format!("  <url type=\"homepage\">{}</url>\n", esc(h)));
    }
    s.push_str(&format!(
        "  <provides>\n    <binary>{}</binary>\n  </provides>\n  <releases>\n    <release version=\"{}\" />\n  </releases>\n  <content_rating type=\"oars-1.1\" />\n</component>\n",
        esc(&p.bin_name),
        esc(&p.version)
    ));
    s
}

/// The `flatpak-builder` manifest, as JSON. `icons` are `(size, file name)`.
pub fn builder_manifest(ctx: &Ctx<'_>, icons: &[(u32, String)]) -> serde_json::Value {
    let p = ctx.project;
    let id = &p.identifier;
    let bin = &p.bin_name;
    let mut cmds = vec![
        format!("install -Dm755 {bin} /app/bin/{bin}"),
        format!("install -Dm644 {id}.desktop /app/share/applications/{id}.desktop"),
        format!("install -Dm644 {id}.metainfo.xml /app/share/metainfo/{id}.metainfo.xml"),
    ];
    let mut sources = vec![
        serde_json::json!({"type": "file", "path": bin}),
        serde_json::json!({"type": "file", "path": format!("{id}.desktop")}),
        serde_json::json!({"type": "file", "path": format!("{id}.metainfo.xml")}),
    ];
    for (size, file) in icons {
        cmds.push(format!(
            "install -Dm644 {file} /app/share/icons/hicolor/{size}x{size}/apps/{id}.png"
        ));
        sources.push(serde_json::json!({"type": "file", "path": file}));
    }
    if ctx.capabilities.is_some() {
        cmds.push(format!(
            "install -Dm644 {CAPABILITIES_FILE} /app/share/{bin}/{CAPABILITIES_FILE}"
        ));
        sources.push(serde_json::json!({"type": "file", "path": CAPABILITIES_FILE}));
    }
    serde_json::json!({
        "app-id": id,
        "runtime": "org.freedesktop.Platform",
        "runtime-version": p.flatpak_runtime_version,
        "sdk": "org.freedesktop.Sdk",
        "command": bin,
        "finish-args": finish_args(ctx),
        "modules": [{
            "name": bin,
            "buildsystem": "simple",
            "build-commands": cmds,
            "sources": sources,
        }],
    })
}

/// Lay out the builder directory and return it.
pub fn source_dir(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_linux(ctx, "flatpak")?;
    if ctx.icons.largest_png().is_none() {
        return Err(Error::Config(
            "flatpak needs at least one PNG icon (set `icons` in [package.metadata.alloy])".into(),
        ));
    }
    let p = ctx.project;
    let dir = ctx.out_dir.join(format!("{}-flatpak", ctx.stem()));
    remove_if_exists(&dir)?;
    copy_executable(&ctx.binary, &dir.join(&p.bin_name))?;
    write_file(
        &dir.join(format!("{}.desktop", p.identifier)),
        desktop_entry(ctx).as_bytes(),
    )?;
    write_file(
        &dir.join(format!("{}.metainfo.xml", p.identifier)),
        metainfo(ctx).as_bytes(),
    )?;
    let mut icons = Vec::new();
    for i in &ctx.icons.pngs {
        let file = format!("icon-{0}x{0}.png", i.size);
        write_file(&dir.join(&file), &i.data)?;
        icons.push((i.size, file));
    }
    if let Some(c) = &ctx.capabilities {
        write_file(&dir.join(CAPABILITIES_FILE), c.as_bytes())?;
    }
    let json = serde_json::to_string_pretty(&builder_manifest(ctx, &icons)).expect("json");
    write_file(&dir.join(format!("{}.json", p.identifier)), json.as_bytes())?;
    Ok(dir)
}

/// Build `<stem>.flatpak`.
pub fn flatpak(ctx: &Ctx<'_>) -> Result<PathBuf> {
    let dir = source_dir(ctx)?;
    let id = &ctx.project.identifier;
    let tool = std::env::var_os("ALLOY_FLATPAK_BUILDER")
        .map(PathBuf::from)
        .or_else(|| which("flatpak-builder"))
        .ok_or(Error::ToolMissing {
            tool: "flatpak-builder",
            hint: "install flatpak and flatpak-builder (and `flatpak install flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08`); the builder directory was left at the output path",
        })?;
    let build = dir.join("build");
    let repo = dir.join("repo");
    run(Command::new(&tool)
        .current_dir(&dir)
        .args([
            "--force-clean",
            "--disable-rofiles-fuse",
            "--user",
            "--install-deps-from=flathub",
            "--repo=repo",
            "build",
        ])
        .arg(format!("{id}.json")))?;
    let out = ctx.out_dir.join(format!("{}.flatpak", ctx.stem()));
    remove_if_exists(&out)?;
    run(Command::new("flatpak")
        .args(["build-bundle"])
        .arg(&repo)
        .arg(&out)
        .arg(id))?;
    remove_if_exists(&build)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::fixture;

    #[test]
    fn permissions_follow_capabilities() {
        let f = fixture(); // capabilities = ["clipboard"]
        let a = finish_args(&f.ctx());
        assert!(a.contains(&"--socket=wayland".to_string()));
        assert!(!a
            .iter()
            .any(|x| x.contains("network") || x.contains("filesystem")));
        let mut ctx = f.ctx();
        ctx.capabilities = Some("capabilities = [\"network\", \"fs_read\", \"fs_write\", \"notifications\"]\n[fs]\nread = [\"/tmp\"]\nwrite = [\"/tmp\"]\n[network]\nhosts = [\"example.com\"]\n".into());
        let a = finish_args(&ctx);
        assert!(a.contains(&"--share=network".to_string()));
        assert!(a.contains(&"--filesystem=home".to_string()));
        assert!(!a.contains(&"--filesystem=home:ro".to_string()));
        assert!(a.contains(&"--talk-name=org.freedesktop.Notifications".to_string()));
    }

    #[test]
    fn override_replaces_derived_args() {
        let mut f = fixture();
        f.project.flatpak_finish_args = Some(vec!["--share=network".into()]);
        assert_eq!(finish_args(&f.ctx()), ["--share=network"]);
    }

    #[test]
    fn source_dir_layout_and_manifest() {
        let mut f = fixture();
        f.with_icons = true;
        let ctx = f.ctx();
        let d = source_dir(&ctx).unwrap();
        for p in [
            "demo",
            "dev.example.demo.desktop",
            "dev.example.demo.metainfo.xml",
            "icon-256x256.png",
            "alloy.capabilities.toml",
            "dev.example.demo.json",
        ] {
            assert!(d.join(p).exists(), "missing {p}");
        }
        let m: serde_json::Value =
            serde_json::from_slice(&std::fs::read(d.join("dev.example.demo.json")).unwrap())
                .unwrap();
        assert_eq!(m["app-id"], "dev.example.demo");
        assert_eq!(m["runtime-version"], "24.08");
        assert_eq!(m["command"], "demo");
        let cmds = m["modules"][0]["build-commands"].to_string();
        assert!(
            cmds.contains("hicolor/256x256/apps/dev.example.demo.png"),
            "{cmds}"
        );
        // Every build command's input file is declared as a source.
        let srcs = m["modules"][0]["sources"].to_string();
        for p in [
            "demo",
            "dev.example.demo.desktop",
            "icon-512x512.png",
            "alloy.capabilities.toml",
        ] {
            assert!(
                srcs.contains(&format!("\"{p}\"")),
                "{p} not a source: {srcs}"
            );
        }
        let desk = std::fs::read_to_string(d.join("dev.example.demo.desktop")).unwrap();
        assert!(desk.contains("\nIcon=dev.example.demo\n") && desk.contains("\nExec=demo\n"));
    }

    #[test]
    fn metainfo_is_well_formed_enough() {
        let f = fixture();
        let m = metainfo(&f.ctx());
        for n in [
            "<id>dev.example.demo</id>",
            "<launchable type=\"desktop-id\">dev.example.demo.desktop</launchable>",
            "<project_license>MIT</project_license>",
            "<release version=\"1.2.3-beta.1\" />",
        ] {
            assert!(m.contains(n), "missing {n}");
        }
    }

    #[test]
    fn needs_icon_and_linux_target() {
        let f = fixture();
        assert!(source_dir(&f.ctx()).is_err());
        let w = crate::bundle::tests_support::fixture_for("x86_64-pc-windows-msvc");
        assert!(source_dir(&w.ctx()).is_err());
    }
}
