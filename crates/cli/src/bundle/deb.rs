//! Debian package builder. Writes the `ar` + `tar.gz` structure directly, so
//! `.deb` files can be produced on any host OS.

use super::archive::{ar, TarGz};
use super::linux::{desktop_entry, require_linux, share_dir};
use super::{write_file, Ctx, CAPABILITIES_FILE};
use crate::error::{Error, Result};
use std::path::PathBuf;

/// Debian architecture name for a Rust arch.
pub fn deb_arch(arch: &str) -> Result<&'static str> {
    Ok(match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "i686" | "i586" => "i386",
        a if a.starts_with("armv7") => "armhf",
        "riscv64gc" | "riscv64" => "riscv64",
        other => {
            return Err(Error::Config(format!(
                "no Debian architecture known for `{other}`"
            )))
        }
    })
}

/// Debian package name derived from the binary name.
pub fn package_name(bin: &str) -> Result<String> {
    let n: String = bin
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c == '_' { '-' } else { c })
        .collect();
    let ok = n.len() >= 2
        && n.starts_with(|c: char| c.is_ascii_alphanumeric())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+-.".contains(c));
    if ok {
        Ok(n)
    } else {
        Err(Error::Config(format!(
            "binary name `{bin}` cannot be turned into a Debian package name"
        )))
    }
}

/// Convert a semver string to a Debian version (`-` pre-release marker
/// becomes `~` so pre-releases sort before the release).
pub fn deb_version(v: &str) -> String {
    match v.split_once('-') {
        Some((a, b)) => format!("{a}~{b}"),
        None => v.to_string(),
    }
}

/// The `control` file.
pub fn control(ctx: &Ctx<'_>, installed_kib: u64) -> Result<String> {
    let p = ctx.project;
    let maintainer = p
        .authors
        .first()
        .cloned()
        .or_else(|| {
            p.publisher
                .as_ref()
                .map(|n| format!("{n} <noreply@invalid>"))
        })
        .unwrap_or_else(|| "Unknown <noreply@invalid>".into());
    let desc = p
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut c = format!(
        "Package: {}\nVersion: {}\nArchitecture: {}\nMaintainer: {}\nInstalled-Size: {}\nSection: utils\nPriority: optional\n",
        package_name(&p.bin_name)?,
        deb_version(&p.version),
        deb_arch(&ctx.arch)?,
        maintainer,
        installed_kib,
    );
    if !p.deb_depends.is_empty() {
        c.push_str(&format!("Depends: {}\n", p.deb_depends.join(", ")));
    }
    if let Some(h) = &p.homepage {
        c.push_str(&format!("Homepage: {h}\n"));
    }
    c.push_str(&format!("Description: {desc}\n"));
    Ok(c)
}

/// Build the `.deb`, returning its path.
pub fn deb(ctx: &Ctx<'_>) -> Result<PathBuf> {
    require_linux(ctx, "deb")?;
    let p = ctx.project;
    let bin = &p.bin_name;
    let mut data = TarGz::new(ctx.mtime());
    data.file(&format!("usr/bin/{bin}"), &ctx.binary_bytes()?, 0o755)?;
    data.file(
        &format!("usr/share/applications/{}.desktop", p.identifier),
        desktop_entry(ctx).as_bytes(),
        0o644,
    )?;
    for icon in &ctx.icons.pngs {
        data.file(
            &format!("usr/share/icons/hicolor/{0}x{0}/apps/{bin}.png", icon.size),
            &icon.data,
            0o644,
        )?;
    }
    if let Some(caps) = &ctx.capabilities {
        data.file(
            &format!("{}/{CAPABILITIES_FILE}", share_dir(ctx)),
            caps.as_bytes(),
            0o644,
        )?;
    }
    let kib = data.payload.div_ceil(1024);
    let data_tgz = data.finish()?;

    let mut control_tar = TarGz::new(ctx.mtime());
    control_tar.file("control", control(ctx, kib)?.as_bytes(), 0o644)?;
    let control_tgz = control_tar.finish()?;

    let file = ar(
        &[
            ("debian-binary", b"2.0\n"),
            ("control.tar.gz", &control_tgz),
            ("data.tar.gz", &data_tgz),
        ],
        ctx.mtime(),
    );
    let out = ctx.out_dir.join(format!(
        "{}_{}_{}.deb",
        package_name(bin)?,
        deb_version(&p.version),
        deb_arch(&ctx.arch)?
    ));
    write_file(&out, &file)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::{fixture, fixture_for};

    #[test]
    fn arch_and_version_mapping() {
        assert_eq!(deb_arch("x86_64").unwrap(), "amd64");
        assert_eq!(deb_arch("aarch64").unwrap(), "arm64");
        assert!(deb_arch("sparc").is_err());
        assert_eq!(deb_version("1.2.3"), "1.2.3");
        assert_eq!(deb_version("1.2.3-beta.1"), "1.2.3~beta.1");
    }

    #[test]
    fn package_names() {
        assert_eq!(package_name("My_App").unwrap(), "my-app");
        assert!(package_name("a").is_err());
        assert!(package_name("-x").is_err());
        assert!(package_name("a b").is_err());
    }

    #[test]
    fn control_file() {
        let f = fixture();
        let c = control(&f.ctx(), 12).unwrap();
        assert!(c.contains("Package: demo\n"));
        assert!(c.contains("Version: 1.2.3~beta.1\n"));
        assert!(c.contains("Architecture: amd64\n"));
        assert!(c.contains("Maintainer: Ann <ann@example.com>\n"));
        assert!(c.contains("Installed-Size: 12\n"));
        assert!(c.contains("Depends: libc6, libxkbcommon0, libvulkan1 | libgl1\n"));
        assert!(c.ends_with("Description: Demo thing.\n"));
    }

    #[test]
    fn builds_a_deb_with_expected_members() {
        let mut f = fixture();
        f.with_icons = true;
        let out = deb(&f.ctx()).unwrap();
        assert_eq!(out.file_name().unwrap(), "demo_1.2.3~beta.1_amd64.deb");
        let bytes = std::fs::read(&out).unwrap();
        assert_eq!(&bytes[..8], b"!<arch>\n");
        assert!(bytes.windows(13).any(|w| w == b"debian-binary"));
        assert!(bytes.windows(14).any(|w| w == b"control.tar.gz"));
        assert!(bytes.windows(11).any(|w| w == b"data.tar.gz"));
    }

    #[test]
    fn refuses_non_linux_targets() {
        let f = fixture_for("aarch64-apple-darwin");
        assert!(deb(&f.ctx()).is_err());
    }
}
