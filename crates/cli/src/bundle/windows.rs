//! Windows: portable `.exe` folder and an NSIS installer.

use super::{remove_if_exists, run, which, write_file, Ctx, CAPABILITIES_FILE};
use crate::error::{Error, Result};
use crate::icon::parse_ico;
use crate::pe::embed_icon;
use std::path::PathBuf;
use std::process::Command;

pub(super) fn require_windows(ctx: &Ctx<'_>, format: &'static str) -> Result<()> {
    if ctx.triple.contains("windows") {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "`{format}` bundles Windows binaries but the target is `{}`; pass --target <windows triple>",
            ctx.triple
        )))
    }
}

/// The executable's bytes with the app icon embedded when one is configured.
/// Falls back to the untouched binary (with a warning) if the PE layout is
/// not one `pe::embed_icon` supports.
pub fn exe_with_icon(ctx: &Ctx<'_>) -> Result<Vec<u8>> {
    let exe = ctx.binary_bytes()?;
    let Some(images) = ctx.icons.ico_bytes().and_then(|b| parse_ico(&b)) else {
        return Ok(exe);
    };
    match embed_icon(&exe, &images) {
        Ok(patched) => Ok(patched),
        Err(e) => {
            eprintln!("warning: could not embed icon in the executable ({e}); using the unmodified binary");
            Ok(exe)
        }
    }
}

/// Copy the exe and capability config into `<stem>-portable/`.
pub fn portable(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_windows(ctx, "exe")?;
    let dir = ctx.out_dir.join(format!("{}-portable", ctx.stem()));
    remove_if_exists(&dir)?;
    let exe = dir.join(format!("{}.exe", ctx.project.bin_name));
    write_file(&exe, &exe_with_icon(ctx)?)?;
    if let Some(c) = &ctx.capabilities {
        write_file(&dir.join(CAPABILITIES_FILE), c.as_bytes())?;
    }
    Ok(exe)
}

/// Escape a string for an NSIS double-quoted literal.
pub fn nsis_str(s: &str) -> String {
    s.replace('$', "$$").replace('"', "$\\\"")
}

/// `1.2.3-beta` -> `1.2.3.0`, or `None` if the version has no numeric head.
pub fn version_quad(v: &str) -> Option<String> {
    let head = v.split(['-', '+']).next()?;
    let mut parts: Vec<u16> = Vec::new();
    for p in head.split('.') {
        parts.push(p.parse().ok()?);
    }
    if parts.is_empty() || parts.len() > 4 {
        return None;
    }
    parts.resize(4, 0);
    Some(
        parts
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join("."),
    )
}

/// Render the NSIS script. Paths must be absolute.
pub fn nsis_script(
    ctx: &Ctx<'_>,
    exe: &str,
    caps: Option<&str>,
    icon: Option<&str>,
    out: &str,
) -> String {
    let p = ctx.project;
    let name = nsis_str(&p.product_name);
    let bin = format!("{}.exe", p.bin_name);
    let key = format!(
        "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        p.identifier
    );
    let mut s = String::new();
    s.push_str("Unicode true\nRequestExecutionLevel user\n");
    s.push_str(&format!("Name \"{name}\"\nOutFile \"{}\"\n", nsis_str(out)));
    s.push_str(&format!("InstallDir \"$LOCALAPPDATA\\Programs\\{name}\"\n"));
    s.push_str("InstallDirRegKey HKCU \"");
    s.push_str(&format!("{}\" \"InstallLocation\"\n", nsis_str(&key)));
    if let Some(i) = icon {
        s.push_str(&format!(
            "Icon \"{0}\"\nUninstallIcon \"{0}\"\n",
            nsis_str(i)
        ));
    }
    if let Some(q) = version_quad(&p.version) {
        s.push_str(&format!(
            "VIProductVersion \"{q}\"\nVIAddVersionKey \"ProductName\" \"{name}\"\nVIAddVersionKey \"FileDescription\" \"{name} installer\"\nVIAddVersionKey \"FileVersion\" \"{}\"\n",
            nsis_str(&p.version)
        ));
        // NSIS warns when a version block has no LegalCopyright.
        let legal = p
            .copyright
            .clone()
            .or_else(|| p.publisher.clone())
            .unwrap_or_else(|| p.product_name.clone());
        s.push_str(&format!(
            "VIAddVersionKey \"LegalCopyright\" \"{}\"\n",
            nsis_str(&legal)
        ));
    }
    s.push_str(
        "Page directory\nPage instfiles\nUninstPage uninstConfirm\nUninstPage instfiles\n\n",
    );
    s.push_str("Section \"Install\"\n  SetOutPath \"$INSTDIR\"\n");
    s.push_str(&format!("  File \"/oname={bin}\" \"{}\"\n", nsis_str(exe)));
    if let Some(c) = caps {
        s.push_str(&format!(
            "  File \"/oname={CAPABILITIES_FILE}\" \"{}\"\n",
            nsis_str(c)
        ));
    }
    s.push_str("  WriteUninstaller \"$INSTDIR\\Uninstall.exe\"\n");
    s.push_str(&format!(
        "  CreateShortcut \"$SMPROGRAMS\\{name}.lnk\" \"$INSTDIR\\{bin}\"\n"
    ));
    let reg = |k: &str, v: &str| {
        format!(
            "  WriteRegStr HKCU \"{}\" \"{k}\" \"{v}\"\n",
            nsis_str(&key)
        )
    };
    s.push_str(&reg("DisplayName", &name));
    s.push_str(&reg("DisplayVersion", &nsis_str(&p.version)));
    if let Some(publisher) = &p.publisher {
        s.push_str(&reg("Publisher", &nsis_str(publisher)));
    }
    s.push_str(&reg("InstallLocation", "$INSTDIR"));
    s.push_str(&reg("DisplayIcon", &format!("$INSTDIR\\{bin}")));
    s.push_str(&reg("UninstallString", "$\\\"$INSTDIR\\Uninstall.exe$\\\""));
    s.push_str(&format!(
        "  WriteRegDWORD HKCU \"{0}\" \"NoModify\" 1\n  WriteRegDWORD HKCU \"{0}\" \"NoRepair\" 1\n",
        nsis_str(&key)
    ));
    s.push_str("SectionEnd\n\n");
    s.push_str("Section \"Uninstall\"\n");
    s.push_str(&format!("  Delete \"$INSTDIR\\{bin}\"\n"));
    s.push_str(&format!("  Delete \"$INSTDIR\\{CAPABILITIES_FILE}\"\n"));
    s.push_str("  Delete \"$INSTDIR\\Uninstall.exe\"\n  RMDir \"$INSTDIR\"\n");
    s.push_str(&format!("  Delete \"$SMPROGRAMS\\{name}.lnk\"\n"));
    s.push_str(&format!(
        "  DeleteRegKey HKCU \"{}\"\nSectionEnd\n",
        nsis_str(&key)
    ));
    s
}

fn find_makensis() -> Option<PathBuf> {
    std::env::var_os("ALLOY_MAKENSIS")
        .map(PathBuf::from)
        .or_else(|| which("makensis"))
        .or_else(|| {
            [
                r"C:\Program Files (x86)\NSIS\makensis.exe",
                r"C:\Program Files\NSIS\makensis.exe",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file())
        })
}

/// Build the per-user NSIS installer, returning its path.
pub fn nsis(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_windows(ctx, "nsis")?;
    let tool = find_makensis().ok_or(Error::ToolMissing {
        tool: "makensis",
        hint: "install NSIS (https://nsis.sourceforge.io) or set ALLOY_MAKENSIS",
    })?;
    let stage = ctx.out_dir.join(format!("{}-nsis", ctx.stem()));
    remove_if_exists(&stage)?;
    let exe = stage.join(format!("{}.exe", ctx.project.bin_name));
    write_file(&exe, &exe_with_icon(ctx)?)?;
    let caps = match &ctx.capabilities {
        Some(c) => {
            let f = stage.join(CAPABILITIES_FILE);
            write_file(&f, c.as_bytes())?;
            Some(f)
        }
        None => None,
    };
    let icon = match ctx.icons.ico_bytes() {
        Some(b) => {
            let f = stage.join("app.ico");
            write_file(&f, &b)?;
            Some(f)
        }
        None => None,
    };
    let out = ctx.out_dir.join(format!("{}-setup.exe", ctx.stem()));
    remove_if_exists(&out)?;
    let script = nsis_script(
        ctx,
        &exe.to_string_lossy(),
        caps.as_deref().map(|p| p.to_string_lossy()).as_deref(),
        icon.as_deref().map(|p| p.to_string_lossy()).as_deref(),
        &out.to_string_lossy(),
    );
    let nsi = stage.join("installer.nsi");
    write_file(&nsi, script.as_bytes())?;
    run(Command::new(tool).arg("-V2").arg(&nsi))?;
    remove_if_exists(&stage)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::{fixture, fixture_for};

    #[test]
    fn escaping() {
        assert_eq!(nsis_str("a$b\"c"), "a$$b$\\\"c");
    }

    #[test]
    fn versions() {
        assert_eq!(version_quad("1.2.3").as_deref(), Some("1.2.3.0"));
        assert_eq!(version_quad("1.2.3-beta.1").as_deref(), Some("1.2.3.0"));
        assert_eq!(version_quad("2").as_deref(), Some("2.0.0.0"));
        assert_eq!(version_quad("x.y"), None);
        assert_eq!(version_quad("1.2.3.4.5"), None);
    }

    #[test]
    fn script_contents() {
        let f = fixture_for("x86_64-pc-windows-msvc");
        let s = nsis_script(
            &f.ctx(),
            r"C:\s\demo.exe",
            Some(r"C:\s\alloy.capabilities.toml"),
            Some(r"C:\s\app.ico"),
            r"C:\o\demo-setup.exe",
        );
        for needle in [
            "RequestExecutionLevel user",
            "Name \"Demo App\"",
            "OutFile \"C:\\o\\demo-setup.exe\"",
            "InstallDir \"$LOCALAPPDATA\\Programs\\Demo App\"",
            "Icon \"C:\\s\\app.ico\"",
            "VIProductVersion \"1.2.3.0\"",
            "File \"/oname=demo.exe\" \"C:\\s\\demo.exe\"",
            "File \"/oname=alloy.capabilities.toml\"",
            "CreateShortcut \"$SMPROGRAMS\\Demo App.lnk\" \"$INSTDIR\\demo.exe\"",
            "\"Publisher\" \"Example\"",
            "UninstallString\" \"$\\\"$INSTDIR\\Uninstall.exe$\\\"\"",
            "DeleteRegKey HKCU",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\dev.example.demo",
        ] {
            assert!(s.contains(needle), "missing {needle}\n{s}");
        }
    }

    #[test]
    fn script_without_optional_parts() {
        let f = fixture_for("x86_64-pc-windows-msvc");
        let s = nsis_script(&f.ctx(), "e.exe", None, None, "o.exe");
        assert!(!s.contains("Icon "));
        assert!(!s.contains(CAPABILITIES_FILE) || s.contains("Delete"));
        assert!(!s.contains("File \"/oname=alloy"));
    }

    #[test]
    fn portable_layout() {
        let f = fixture_for("x86_64-pc-windows-msvc");
        let exe = portable(&f.ctx()).unwrap();
        assert!(exe.ends_with("demo.exe"));
        assert!(exe.with_file_name(CAPABILITIES_FILE).exists());
    }

    #[test]
    fn portable_exe_gets_the_icon() {
        let mut f = fixture_for("x86_64-pc-windows-msvc");
        f.with_icons = true;
        std::fs::write(f.dir.join("demo.bin"), crate::pe::tests::tiny_pe()).unwrap();
        let exe = portable(&f.ctx()).unwrap();
        let res = crate::pe::tests::read_resources(&std::fs::read(exe).unwrap());
        assert_eq!(res.len(), 3); // 32 and 256 px icons (ico max) and the group
    }

    #[test]
    fn unpatchable_exe_falls_back_to_original() {
        let mut f = fixture_for("x86_64-pc-windows-msvc");
        f.with_icons = true; // demo.bin is not a PE file
        let exe = portable(&f.ctx()).unwrap();
        assert_eq!(std::fs::read(exe).unwrap(), b"\x7fELF fake binary");
    }

    #[test]
    fn rejects_non_windows_target() {
        let f = fixture();
        assert!(portable(&f.ctx()).is_err());
        assert!(nsis(&f.ctx()).is_err());
    }
}
