//! MSI installer via the WiX Toolset (v4/v5 `wix build`). The `.wxs` source is
//! generated here; compiling it needs the `wix` CLI
//! (`dotnet tool install --global wix`).

use super::sha256::sha256;
use super::windows::{exe_with_icon, require_windows, version_quad};
use super::{remove_if_exists, run, which, write_file, Ctx, CAPABILITIES_FILE};
use crate::error::{Error, Result};
use std::path::PathBuf;
use std::process::Command;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A stable GUID derived from `seed` (name-based, RFC 4122 version 5 layout),
/// so rebuilding the same app yields the same `UpgradeCode`.
pub fn guid(seed: &str) -> String {
    let h = sha256(seed.as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&h[..16]);
    b[6] = (b[6] & 0x0f) | 0x50;
    b[8] = (b[8] & 0x3f) | 0x80;
    let x: Vec<String> = b.iter().map(|v| format!("{v:02X}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        x[0..4].concat(),
        x[4..6].concat(),
        x[6..8].concat(),
        x[8..10].concat(),
        x[10..16].concat()
    )
}

/// WiX platform name for a Rust arch.
pub fn wix_arch(arch: &str) -> Result<&'static str> {
    Ok(match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "i686" | "i586" => "x86",
        other => return Err(Error::Config(format!("no WiX platform for `{other}`"))),
    })
}

/// Render the `.wxs` document. Source paths must be absolute.
pub fn wxs(ctx: &Ctx<'_>, exe: &str, caps: Option<&str>, icon: Option<&str>) -> Result<String> {
    let p = ctx.project;
    let ver = version_quad(&p.version)
        .map(|q| q.rsplit_once('.').map_or("0.0.0", |x| x.0).to_string())
        .ok_or_else(|| {
            Error::Config(format!(
                "version `{}` must start with numeric major.minor.patch for an MSI",
                p.version
            ))
        })?;
    let name = esc(&p.product_name);
    let publisher = esc(p.publisher.as_deref().unwrap_or(&p.product_name));
    let bin = format!("{}.exe", p.bin_name);
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<Wix xmlns=\"http://wixtoolset.org/schemas/v4/wxs\">\n");
    s.push_str(&format!(
        "  <Package Name=\"{name}\" Manufacturer=\"{publisher}\" Version=\"{ver}\" UpgradeCode=\"{}\" Scope=\"perUser\" Language=\"1033\">\n",
        guid(&format!("upgrade:{}", p.identifier))
    ));
    s.push_str("    <MajorUpgrade DowngradeErrorMessage=\"A newer version of [ProductName] is already installed.\" />\n");
    s.push_str("    <MediaTemplate EmbedCab=\"yes\" />\n");
    if let Some(i) = icon {
        s.push_str(&format!(
            "    <Icon Id=\"AppIcon.ico\" SourceFile=\"{}\" />\n",
            esc(i)
        ));
        s.push_str("    <Property Id=\"ARPPRODUCTICON\" Value=\"AppIcon.ico\" />\n");
    }
    s.push_str("    <StandardDirectory Id=\"LocalAppDataFolder\">\n      <Directory Id=\"ProgramsDir\" Name=\"Programs\">\n");
    s.push_str(&format!(
        "        <Directory Id=\"INSTALLFOLDER\" Name=\"{name}\">\n"
    ));
    s.push_str(&format!(
        "          <Component Id=\"MainExe\" Guid=\"{}\">\n            <File Id=\"MainExeFile\" Name=\"{}\" Source=\"{}\" KeyPath=\"yes\" />\n          </Component>\n",
        guid(&format!("exe:{}", p.identifier)),
        esc(&bin),
        esc(exe)
    ));
    if let Some(c) = caps {
        s.push_str(&format!(
            "          <Component Id=\"Capabilities\" Guid=\"{}\">\n            <File Id=\"CapabilitiesFile\" Name=\"{CAPABILITIES_FILE}\" Source=\"{}\" KeyPath=\"yes\" />\n          </Component>\n",
            guid(&format!("caps:{}", p.identifier)),
            esc(c)
        ));
    }
    s.push_str("        </Directory>\n      </Directory>\n    </StandardDirectory>\n");
    s.push_str(&format!(
        "    <StandardDirectory Id=\"ProgramMenuFolder\">\n      <Component Id=\"StartMenuShortcut\" Guid=\"{}\">\n        <Shortcut Id=\"AppShortcut\" Name=\"{name}\" Target=\"[INSTALLFOLDER]{}\" WorkingDirectory=\"INSTALLFOLDER\" />\n        <RegistryValue Root=\"HKCU\" Key=\"Software\\{}\" Name=\"installed\" Type=\"integer\" Value=\"1\" KeyPath=\"yes\" />\n      </Component>\n    </StandardDirectory>\n",
        guid(&format!("shortcut:{}", p.identifier)),
        esc(&bin),
        esc(&p.identifier)
    ));
    s.push_str("    <Feature Id=\"Main\" Title=\"Application\" Level=\"1\">\n      <ComponentRef Id=\"MainExe\" />\n");
    if caps.is_some() {
        s.push_str("      <ComponentRef Id=\"Capabilities\" />\n");
    }
    s.push_str(
        "      <ComponentRef Id=\"StartMenuShortcut\" />\n    </Feature>\n  </Package>\n</Wix>\n",
    );
    Ok(s)
}

/// Build the per-user MSI, returning its path.
pub fn msi(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_windows(ctx, "msi")?;
    let tool = std::env::var_os("ALLOY_WIX")
        .map(PathBuf::from)
        .or_else(|| which("wix"))
        .ok_or(Error::ToolMissing {
            tool: "wix",
            hint: "install WiX 4+ with `dotnet tool install --global wix` or set ALLOY_WIX",
        })?;
    let stage = ctx.out_dir.join(format!("{}-msi", ctx.stem()));
    remove_if_exists(&stage)?;
    let exe = stage.join(format!("{}.exe", ctx.project.bin_name));
    write_file(&exe, &exe_with_icon(ctx)?)?;
    super::sign::win_sign_file(&exe)?;
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
    let src = wxs(
        ctx,
        &exe.to_string_lossy(),
        caps.as_deref().map(|p| p.to_string_lossy()).as_deref(),
        icon.as_deref().map(|p| p.to_string_lossy()).as_deref(),
    )?;
    let wxs_path = stage.join("installer.wxs");
    write_file(&wxs_path, src.as_bytes())?;
    let out = ctx.out_dir.join(format!("{}.msi", ctx.stem()));
    remove_if_exists(&out)?;
    // WiX 5 has no `-sval` (that was WiX 3 light.exe); validation stays on and
    // the CI job installs and removes the MSI to prove it works.
    run(Command::new(tool)
        .args(["build", "-arch", wix_arch(&ctx.arch)?, "-o"])
        .arg(&out)
        .arg(&wxs_path))?;
    remove_if_exists(&stage)?;
    super::sign::win_sign_file(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::{fixture, fixture_for};

    #[test]
    fn guid_is_stable_and_well_formed() {
        let g = guid("a");
        assert_eq!(g, guid("a"));
        assert_ne!(g, guid("b"));
        assert_eq!(g.len(), 36);
        assert_eq!(&g[14..15], "5");
        assert!(g.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }

    #[test]
    fn wxs_contents() {
        let f = fixture_for("x86_64-pc-windows-msvc");
        let s = wxs(
            &f.ctx(),
            r"C:\s\demo.exe",
            Some(r"C:\s\caps.toml"),
            Some(r"C:\s\app.ico"),
        )
        .unwrap();
        for needle in [
            "xmlns=\"http://wixtoolset.org/schemas/v4/wxs\"",
            "Name=\"Demo App\"",
            "Manufacturer=\"Example\"",
            "Version=\"1.2.3\"",
            "Scope=\"perUser\"",
            "<MajorUpgrade",
            "Source=\"C:\\s\\demo.exe\"",
            "Name=\"alloy.capabilities.toml\"",
            "ARPPRODUCTICON",
            "Target=\"[INSTALLFOLDER]demo.exe\"",
            "<ComponentRef Id=\"Capabilities\" />",
        ] {
            assert!(s.contains(needle), "missing {needle}\n{s}");
        }
        let again = wxs(
            &f.ctx(),
            r"C:\s\demo.exe",
            Some(r"C:\s\caps.toml"),
            Some(r"C:\s\app.ico"),
        )
        .unwrap();
        assert_eq!(s, again, "upgrade code must be stable");
    }

    #[test]
    fn wxs_without_optional_parts_and_escaping() {
        let mut f = fixture_for("x86_64-pc-windows-msvc");
        f.project.product_name = "A & B".into();
        let s = wxs(&f.ctx(), "e.exe", None, None).unwrap();
        assert!(s.contains("Name=\"A &amp; B\""));
        assert!(!s.contains("Icon") && !s.contains("Capabilities"));
    }

    #[test]
    fn rejects_non_numeric_versions_and_hosts() {
        let mut f = fixture_for("x86_64-pc-windows-msvc");
        f.project.version = "nightly".into();
        assert!(wxs(&f.ctx(), "e", None, None).is_err());
        let g = fixture();
        assert!(msi(&g.ctx()).is_err());
        assert_eq!(wix_arch("aarch64").unwrap(), "arm64");
    }
}
