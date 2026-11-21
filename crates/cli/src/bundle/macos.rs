//! macOS `.app` and `.dmg`.

use super::{copy_executable, remove_if_exists, run, write_file, Ctx, CAPABILITIES_FILE};
use crate::error::{io, Error, Result};
use std::path::PathBuf;
use std::process::Command;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Map a freedesktop category to an Apple `LSApplicationCategoryType`.
pub fn mac_category(cat: &str) -> &'static str {
    match cat {
        "Development" => "public.app-category.developer-tools",
        "Graphics" => "public.app-category.graphics-design",
        "Office" => "public.app-category.productivity",
        "AudioVideo" | "Audio" => "public.app-category.music",
        "Video" => "public.app-category.entertainment",
        "Game" => "public.app-category.games",
        "Education" | "Science" => "public.app-category.education",
        _ => "public.app-category.utilities",
    }
}

/// Render `Info.plist`.
pub fn info_plist(ctx: &Ctx<'_>, has_icon: bool) -> String {
    let p = ctx.project;
    let mut kv = vec![
        ("CFBundleDevelopmentRegion", "en".to_string()),
        ("CFBundleExecutable", p.bin_name.clone()),
        ("CFBundleIdentifier", p.identifier.clone()),
        ("CFBundleInfoDictionaryVersion", "6.0".into()),
        ("CFBundleName", p.product_name.clone()),
        ("CFBundleDisplayName", p.product_name.clone()),
        ("CFBundlePackageType", "APPL".into()),
        ("CFBundleShortVersionString", p.version.clone()),
        ("CFBundleVersion", p.version.clone()),
        (
            "LSApplicationCategoryType",
            mac_category(&p.category).into(),
        ),
        ("LSMinimumSystemVersion", p.min_macos.clone()),
    ];
    if has_icon {
        kv.push(("CFBundleIconFile", "icon".into()));
    }
    if let Some(c) = &p.copyright {
        kv.push(("NSHumanReadableCopyright", c.clone()));
    }
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n",
    );
    for (k, v) in kv {
        s.push_str(&format!(
            "\t<key>{k}</key>\n\t<string>{}</string>\n",
            esc(&v)
        ));
    }
    s.push_str("\t<key>NSHighResolutionCapable</key>\n\t<true/>\n</dict>\n</plist>\n");
    s
}

/// Build `<Product>.app`, returning its path.
pub fn app(ctx: &Ctx<'_>) -> Result<PathBuf> {
    if !ctx.triple.contains("apple") {
        return Err(Error::Config(format!(
            "`app` bundles macOS binaries but the target is `{}`; pass --target <apple triple>",
            ctx.triple
        )));
    }
    let p = ctx.project;
    let app = ctx.out_dir.join(format!("{}.app", p.product_name));
    remove_if_exists(&app)?;
    let contents = app.join("Contents");
    copy_executable(&ctx.binary, &contents.join("MacOS").join(&p.bin_name))?;
    let icns = ctx.icons.icns_bytes();
    if let Some(i) = &icns {
        write_file(&contents.join("Resources/icon.icns"), i)?;
    } else if !ctx.icons.is_empty() {
        eprintln!("warning: no PNG of size 16/32/64/128/256/512/1024 or .icns given; app will have no icon");
    }
    if let Some(caps) = &ctx.capabilities {
        write_file(
            &contents.join("Resources").join(CAPABILITIES_FILE),
            caps.as_bytes(),
        )?;
    }
    write_file(
        &contents.join("Info.plist"),
        info_plist(ctx, icns.is_some()).as_bytes(),
    )?;
    write_file(&contents.join("PkgInfo"), b"APPL????")?;
    super::sign::mac_sign_and_notarize(ctx, &app)?;
    Ok(app)
}

/// Build a compressed `.dmg` containing the app and an Applications link.
pub fn dmg(ctx: &Ctx<'_>) -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err(Error::WrongHost {
            format: "dmg",
            host: "macOS (needs hdiutil)",
        });
    }
    let app_path = app(ctx)?;
    let stage = ctx.out_dir.join(format!("{}-dmg-stage", ctx.stem()));
    remove_if_exists(&stage)?;
    std::fs::create_dir_all(&stage).map_err(io("creating dmg staging dir"))?;
    // `cp -R` preserves the bundle's modes and any code signature.
    run(Command::new("cp").arg("-R").arg(&app_path).arg(&stage))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("/Applications", stage.join("Applications"))
        .map_err(io("linking /Applications"))?;
    let out = ctx.out_dir.join(format!("{}.dmg", ctx.stem()));
    remove_if_exists(&out)?;
    let r = run(Command::new("hdiutil")
        .args(["create", "-volname", &ctx.project.product_name])
        .arg("-srcfolder")
        .arg(&stage)
        .args(["-ov", "-format", "UDZO"])
        .arg(&out));
    remove_if_exists(&stage)?;
    r?;
    // The notarised, stapled .app inside is what Gatekeeper checks; the disk
    // image itself only needs a signature.
    super::sign::mac_sign_and_notarize(ctx, &out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::{fixture, fixture_for};

    #[test]
    fn plist_contents() {
        let f = fixture_for("aarch64-apple-darwin");
        let s = info_plist(&f.ctx(), true);
        for needle in [
            "<key>CFBundleIdentifier</key>\n\t<string>dev.example.demo</string>",
            "<key>CFBundleExecutable</key>\n\t<string>demo</string>",
            "<key>CFBundleName</key>\n\t<string>Demo App</string>",
            "<key>CFBundleShortVersionString</key>\n\t<string>1.2.3-beta.1</string>",
            "<key>CFBundleIconFile</key>",
            "public.app-category.graphics-design",
            "<key>NSHighResolutionCapable</key>\n\t<true/>",
        ] {
            assert!(s.contains(needle), "missing {needle}");
        }
        assert!(!info_plist(&f.ctx(), false).contains("CFBundleIconFile"));
    }

    #[test]
    fn plist_escapes_xml() {
        let mut f = fixture_for("aarch64-apple-darwin");
        f.project.product_name = "A & B".into();
        assert!(info_plist(&f.ctx(), false).contains("A &amp; B"));
    }

    #[test]
    fn app_layout() {
        let mut f = fixture_for("aarch64-apple-darwin");
        f.with_icons = true;
        let ctx = f.ctx();
        let a = app(&ctx).unwrap();
        assert_eq!(a.file_name().unwrap(), "Demo App.app");
        for p in [
            "Contents/MacOS/demo",
            "Contents/Info.plist",
            "Contents/PkgInfo",
            "Contents/Resources/icon.icns",
            "Contents/Resources/alloy.capabilities.toml",
        ] {
            assert!(a.join(p).exists(), "missing {p}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let m = std::fs::metadata(a.join("Contents/MacOS/demo")).unwrap();
            assert_eq!(m.permissions().mode() & 0o111, 0o111);
        }
        // Re-bundling replaces the old bundle instead of failing.
        app(&ctx).unwrap();
    }

    #[test]
    fn rejects_non_apple_target() {
        let f = fixture();
        assert!(app(&f.ctx()).is_err());
    }

    #[test]
    fn category_mapping() {
        assert_eq!(mac_category("Utility"), "public.app-category.utilities");
        assert_eq!(mac_category("Game"), "public.app-category.games");
    }
}
