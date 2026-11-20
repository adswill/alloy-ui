//! Packaging: one module per platform family.

mod appimage;
mod archive;
mod deb;
mod linux;
mod macos;
mod msi;
mod rpm;
mod sha256;
mod windows;

use crate::error::{io, Error, Result};
use crate::icon::IconSet;
use crate::project::Project;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Name of the capability config inside every bundle.
pub const CAPABILITIES_FILE: &str = "alloy.capabilities.toml";

/// A bundle format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// macOS `.app`.
    App,
    /// macOS `.dmg`.
    Dmg,
    /// Debian package.
    Deb,
    /// Linux AppImage.
    AppImage,
    /// RPM package.
    Rpm,
    /// Windows portable folder with the `.exe`.
    Exe,
    /// Windows NSIS installer.
    Nsis,
    /// Windows MSI installer (WiX).
    Msi,
}

impl Format {
    /// Parse a `--format` value.
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "app" => Format::App,
            "dmg" => Format::Dmg,
            "deb" => Format::Deb,
            "appimage" => Format::AppImage,
            "rpm" => Format::Rpm,
            "exe" => Format::Exe,
            "nsis" => Format::Nsis,
            "msi" => Format::Msi,
            other => {
                return Err(Error::Usage(format!(
                    "unknown format `{other}`; use app, dmg, deb, rpm, appimage, exe, nsis or msi"
                )))
            }
        })
    }

    /// The name used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Format::App => "app",
            Format::Dmg => "dmg",
            Format::Deb => "deb",
            Format::AppImage => "appimage",
            Format::Rpm => "rpm",
            Format::Exe => "exe",
            Format::Nsis => "nsis",
            Format::Msi => "msi",
        }
    }

    /// Formats built when `--format` is omitted, chosen by target triple.
    pub fn defaults_for(triple: &str) -> Vec<Format> {
        if triple.contains("apple") {
            vec![Format::App, Format::Dmg]
        } else if triple.contains("windows") {
            vec![Format::Exe, Format::Nsis]
        } else {
            vec![Format::Deb, Format::AppImage]
        }
    }
}

/// Everything a bundler needs.
pub struct Ctx<'a> {
    /// App metadata.
    pub project: &'a Project,
    /// The compiled binary.
    pub binary: PathBuf,
    /// Target triple.
    pub triple: String,
    /// CPU architecture (first triple component).
    pub arch: String,
    /// Where artifacts go.
    pub out_dir: PathBuf,
    /// App icons.
    pub icons: IconSet,
    /// Verbatim, already-validated capability config.
    pub capabilities: Option<String>,
}

impl<'a> Ctx<'a> {
    /// Create a context, checking that the binary exists.
    pub fn new(
        project: &'a Project,
        binary: PathBuf,
        triple: String,
        out_dir: PathBuf,
        icons: IconSet,
        capabilities: Option<String>,
    ) -> Result<Self> {
        if !binary.is_file() {
            return Err(Error::Config(format!(
                "binary {} not found; run without --skip-build or pass --binary",
                binary.display()
            )));
        }
        let arch = triple.split('-').next().unwrap_or("unknown").to_string();
        Ok(Self {
            project,
            binary,
            triple,
            arch,
            out_dir,
            icons,
            capabilities,
        })
    }

    /// `<slug>-<version>-<arch>`, the stem of artifact file names.
    pub fn stem(&self) -> String {
        format!(
            "{}-{}-{}",
            self.project.slug(),
            self.project.version,
            self.arch
        )
    }

    /// Modification time for archive entries: `SOURCE_DATE_EPOCH` or 0, so
    /// repeated builds are byte-identical.
    pub fn mtime(&self) -> u64 {
        std::env::var("SOURCE_DATE_EPOCH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    pub(crate) fn binary_bytes(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.binary).map_err(io(format!("reading {}", self.binary.display())))
    }
}

/// Produce one artifact; returns its path.
pub fn bundle(ctx: &Ctx<'_>, format: Format) -> Result<PathBuf> {
    eprintln!("   bundling {}", format.name());
    match format {
        Format::App => macos::app(ctx),
        Format::Dmg => macos::dmg(ctx),
        Format::Deb => deb::deb(ctx),
        Format::AppImage => appimage::appimage(ctx),
        Format::Rpm => rpm::rpm(ctx),
        Format::Exe => windows::portable(ctx),
        Format::Nsis => windows::nsis(ctx),
        Format::Msi => msi::msi(ctx),
    }
}

// ---- shared helpers -------------------------------------------------------

pub(crate) fn write_file(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io(format!("creating {}", parent.display())))?;
    }
    std::fs::write(path, data).map_err(io(format!("writing {}", path.display())))
}

pub(crate) fn copy_executable(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(io(format!("creating {}", parent.display())))?;
    }
    std::fs::copy(from, to).map_err(io(format!("copying to {}", to.display())))?;
    set_executable(to)
}

#[cfg(unix)]
pub(crate) fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(io(format!("chmod {}", path.display())))
}

#[cfg(not(unix))]
pub(crate) fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

pub(crate) fn remove_if_exists(path: &Path) -> Result<()> {
    let r = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    };
    r.map_err(io(format!("removing {}", path.display())))
}

/// Find `name` on `PATH` (trying `.exe` on Windows).
pub(crate) fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
        let exe = dir.join(format!("{name}.exe"));
        exe.is_file().then_some(exe)
    })
}

pub(crate) fn run(cmd: &mut Command) -> Result<()> {
    let desc = format!("{:?}", cmd).replace('"', "");
    let status = cmd.status().map_err(io(format!("running {desc}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::CommandFailed {
            cmd: desc,
            status: status.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_roundtrip() {
        for f in [
            Format::App,
            Format::Dmg,
            Format::Deb,
            Format::AppImage,
            Format::Rpm,
            Format::Exe,
            Format::Nsis,
            Format::Msi,
        ] {
            assert_eq!(Format::parse(f.name()).unwrap(), f);
        }
        assert!(Format::parse("zip").is_err());
    }

    #[test]
    fn defaults_by_triple() {
        assert_eq!(
            Format::defaults_for("aarch64-apple-darwin"),
            vec![Format::App, Format::Dmg]
        );
        assert_eq!(
            Format::defaults_for("x86_64-pc-windows-msvc"),
            vec![Format::Exe, Format::Nsis]
        );
        assert_eq!(
            Format::defaults_for("x86_64-unknown-linux-gnu"),
            vec![Format::Deb, Format::AppImage]
        );
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use crate::icon::{test_png, PngIcon};
    use crate::project::Selector;
    use serde_json::json;

    /// A throwaway project with a fake binary in a temp directory.
    pub struct Fixture {
        pub dir: PathBuf,
        pub project: Project,
        pub triple: String,
        pub with_icons: bool,
    }

    pub fn fixture() -> Fixture {
        fixture_for("x86_64-unknown-linux-gnu")
    }

    pub fn fixture_for(triple: &str) -> Fixture {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "alloy-bundle-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let m = json!({"target_directory": dir.join("target"), "packages": [{
            "name": "demo", "version": "1.2.3-beta.1", "description": "Demo thing.",
            "authors": ["Ann <ann@example.com>"], "license": "MIT",
            "manifest_path": dir.join("Cargo.toml"),
            "targets": [{"name": "demo", "kind": ["bin"]}],
            "metadata": {"alloy": {"identifier": "dev.example.demo", "product-name": "Demo App",
                "category": "Graphics", "publisher": "Example"}}}]});
        let project = Project::from_metadata(
            &m,
            &Selector {
                package: Some("demo".into()),
                ..Default::default()
            },
        )
        .unwrap();
        std::fs::write(dir.join("demo.bin"), b"\x7fELF fake binary").unwrap();
        Fixture {
            dir,
            project,
            triple: triple.into(),
            with_icons: false,
        }
    }

    impl Fixture {
        pub fn ctx(&self) -> Ctx<'_> {
            let mut icons = IconSet::default();
            if self.with_icons {
                for s in [32u32, 256, 512] {
                    icons.pngs.push(PngIcon {
                        size: s,
                        data: test_png(s),
                    });
                }
            }
            Ctx::new(
                &self.project,
                self.dir.join("demo.bin"),
                self.triple.clone(),
                self.dir.join("out"),
                icons,
                Some("capabilities = [\"clipboard\"]\n".into()),
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }
}
