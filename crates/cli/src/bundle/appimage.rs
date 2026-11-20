//! AppImage bundler. Lays out an AppDir itself, then hands it to
//! `appimagetool` (squashfs + runtime), which must be installed.

use super::linux::{desktop_entry, require_linux, share_dir};
use super::{
    copy_executable, remove_if_exists, run, set_executable, which, write_file, Ctx,
    CAPABILITIES_FILE,
};
use crate::error::{Error, Result};
use std::path::PathBuf;
use std::process::Command;

const APP_RUN: &str = "#!/bin/sh\nHERE=\"$(dirname \"$(readlink -f \"$0\")\")\"\nexec \"$HERE/usr/bin/@BIN@\" \"$@\"\n";

/// Create the AppDir and return its path.
pub fn app_dir(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_linux(ctx, "appimage")?;
    let bin = &ctx.project.bin_name;
    let icon = ctx.icons.largest_png().ok_or_else(|| {
        Error::Config(
            "appimage needs at least one PNG icon (set `icons` in [package.metadata.alloy])".into(),
        )
    })?;
    let dir = ctx.out_dir.join(format!("{}.AppDir", ctx.stem()));
    remove_if_exists(&dir)?;
    copy_executable(&ctx.binary, &dir.join("usr/bin").join(bin))?;
    let run_sh = dir.join("AppRun");
    write_file(&run_sh, APP_RUN.replace("@BIN@", bin).as_bytes())?;
    set_executable(&run_sh)?;
    let desktop = desktop_entry(ctx);
    write_file(&dir.join(format!("{bin}.desktop")), desktop.as_bytes())?;
    write_file(
        &dir.join("usr/share/applications")
            .join(format!("{bin}.desktop")),
        desktop.as_bytes(),
    )?;
    write_file(&dir.join(format!("{bin}.png")), &icon.data)?;
    write_file(&dir.join(".DirIcon"), &icon.data)?;
    for i in &ctx.icons.pngs {
        write_file(
            &dir.join(format!(
                "usr/share/icons/hicolor/{0}x{0}/apps/{bin}.png",
                i.size
            )),
            &i.data,
        )?;
    }
    if let Some(caps) = &ctx.capabilities {
        write_file(
            &dir.join(share_dir(ctx)).join(CAPABILITIES_FILE),
            caps.as_bytes(),
        )?;
    }
    Ok(dir)
}

/// Build the AppImage, returning its path.
pub fn appimage(ctx: &Ctx<'_>) -> Result<PathBuf> {
    let dir = app_dir(ctx)?;
    let tool = std::env::var_os("ALLOY_APPIMAGETOOL")
        .map(PathBuf::from)
        .or_else(|| which("appimagetool"))
        .ok_or(Error::ToolMissing {
            tool: "appimagetool",
            hint: "install it from https://github.com/AppImage/appimagetool or set ALLOY_APPIMAGETOOL; the AppDir was left at the output path",
        })?;
    let out = ctx
        .out_dir
        .join(format!("{}-{}.AppImage", ctx.project.slug(), ctx.arch));
    remove_if_exists(&out)?;
    run(Command::new(tool)
        .env("ARCH", &ctx.arch)
        // Lets the tool run on CI machines without FUSE.
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .arg(&dir)
        .arg(&out))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::fixture;

    #[test]
    fn appdir_layout() {
        let mut f = fixture();
        f.with_icons = true;
        let ctx = f.ctx();
        let d = app_dir(&ctx).unwrap();
        for p in [
            "AppRun",
            "demo.desktop",
            "demo.png",
            ".DirIcon",
            "usr/bin/demo",
            "usr/share/applications/demo.desktop",
            "usr/share/icons/hicolor/256x256/apps/demo.png",
            "usr/share/demo/alloy.capabilities.toml",
        ] {
            assert!(d.join(p).exists(), "missing {p}");
        }
        let run = std::fs::read_to_string(d.join("AppRun")).unwrap();
        assert!(run.contains("usr/bin/demo"));
        // The root icon is the largest one.
        let png = std::fs::read(d.join("demo.png")).unwrap();
        assert_eq!(crate::icon::png_size(&png), Some((512, 512)));
    }

    #[test]
    fn requires_an_icon() {
        let f = fixture();
        assert!(matches!(app_dir(&f.ctx()), Err(Error::Config(_))));
    }
}
