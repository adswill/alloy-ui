//! Check for, download and install an update.
//!
//! ```no_run
//! use alloy_ui_ship::update::{Config, Updater};
//! use alloy_ui_ship::UreqFetch;
//!
//! let cfg = Config::new(
//!     "dev.example.app",
//!     env!("CARGO_PKG_VERSION"),
//!     "https://example.com/app/latest.json",
//!     "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
//! );
//! let updater = Updater::new(cfg, UreqFetch::default());
//! if let Some(update) = updater.check()? {
//!     let file = updater.download(&update, &std::env::temp_dir())?;
//!     alloy_ui_ship::update::apply(&file, &update.artifact)?;
//! }
//! # Ok::<(), alloy_ui_ship::Error>(())
//! ```
//!
//! Policy: the manifest's signature is verified before it is parsed; its
//! `app` must match; only strictly newer versions are offered (no downgrade);
//! the artifact must match the signed size and SHA-256 before it is moved out
//! of its `.part` file.

use crate::manifest::{Artifact, Manifest};
use crate::net::Fetch;
use crate::version::is_newer;
use crate::{io, Error, Result};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

/// What to check and how to trust it.
#[derive(Debug, Clone)]
pub struct Config {
    /// App identifier (must equal the manifest's `app`).
    pub app: String,
    /// Running version, normally `env!("CARGO_PKG_VERSION")`.
    pub current_version: String,
    /// URL of `latest.json`; the signature is read from `<url>.sig`.
    pub endpoint: String,
    /// Ed25519 public key, hex.
    pub public_key: String,
    /// Operating system, default `std::env::consts::OS`.
    pub os: String,
    /// CPU architecture, default `std::env::consts::ARCH`.
    pub arch: String,
    /// Acceptable formats, best first. Default: [`default_formats`].
    pub formats: Vec<String>,
}

impl Config {
    /// A config for the running OS and CPU.
    pub fn new(app: &str, current_version: &str, endpoint: &str, public_key: &str) -> Self {
        Self {
            app: app.into(),
            current_version: current_version.into(),
            endpoint: endpoint.into(),
            public_key: public_key.into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            formats: default_formats(),
        }
    }
}

/// Formats an in-app update can use on this install.
///
/// Linux: only an AppImage can replace itself (`$APPIMAGE` is set by its
/// runtime); `.deb`, `.rpm` and Flatpak installs are updated by their package
/// manager, so the list is empty and [`Updater::check`] reports nothing.
/// macOS: a `.dmg` is handed to Finder. Windows: an `.msi` is handed to
/// Windows Installer (its `MajorUpgrade` replaces the old version).
pub fn default_formats() -> Vec<String> {
    let v: &[&str] = match std::env::consts::OS {
        "linux" if std::env::var_os("APPIMAGE").is_some() => &["appimage"],
        "linux" => &[],
        "macos" => &["dmg"],
        "windows" => &["msi"],
        _ => &[],
    };
    v.iter().map(|s| s.to_string()).collect()
}

/// An available update.
#[derive(Debug, Clone)]
pub struct Update {
    /// The new version.
    pub version: String,
    /// Release notes.
    pub notes: String,
    /// The build chosen for this machine.
    pub artifact: Artifact,
}

/// Checks and downloads through a [`Fetch`].
pub struct Updater<F: Fetch> {
    cfg: Config,
    fetch: F,
}

impl<F: Fetch> Updater<F> {
    /// Create an updater.
    pub fn new(cfg: Config, fetch: F) -> Self {
        Self { cfg, fetch }
    }

    /// Fetch and verify the manifest. `Ok(None)` when already up to date or
    /// when no acceptable build exists for this install.
    pub fn check(&self) -> Result<Option<Update>> {
        let body = self.fetch.get(&self.cfg.endpoint)?;
        let sig = self.fetch.get(&format!("{}.sig", self.cfg.endpoint))?;
        let sig = String::from_utf8_lossy(&sig).into_owned();
        let m = Manifest::parse_verified(&body, &sig, &self.cfg.public_key)?;
        if m.app != self.cfg.app {
            return Err(Error::BadManifest(format!(
                "manifest is for `{}`, not `{}`",
                m.app, self.cfg.app
            )));
        }
        if !is_newer(&m.version, &self.cfg.current_version) {
            return Ok(None);
        }
        Ok(self.pick(&m).map(|artifact| Update {
            version: m.version.clone(),
            notes: m.notes.clone(),
            artifact,
        }))
    }

    fn pick(&self, m: &Manifest) -> Option<Artifact> {
        self.cfg.formats.iter().find_map(|f| {
            m.artifacts
                .iter()
                .find(|a| a.os == self.cfg.os && a.arch == self.cfg.arch && &a.format == f)
                .cloned()
        })
    }

    /// Download the artifact into `dir`, verifying size and SHA-256. The file
    /// only gets its final name after it verifies; a failed download leaves
    /// nothing behind.
    pub fn download(&self, update: &Update, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir).map_err(io(format!("creating {}", dir.display())))?;
        let name = file_name(&update.artifact.url);
        let part = dir.join(format!("{name}.part"));
        let dest = dir.join(&name);
        let r = (|| {
            let f =
                std::fs::File::create(&part).map_err(io(format!("creating {}", part.display())))?;
            let mut w = HashWriter {
                inner: f,
                hash: Sha256::new(),
                len: 0,
            };
            self.fetch.download(&update.artifact.url, &mut w)?;
            check_digest(
                &update.artifact,
                w.len,
                &crate::keys::hex(&w.hash.finalize()),
            )?;
            std::fs::rename(&part, &dest).map_err(io("finalising download"))
        })();
        if r.is_err() {
            std::fs::remove_file(&part).ok();
        }
        r.map(|()| dest)
    }
}

fn check_digest(a: &Artifact, len: u64, sha_hex: &str) -> Result<()> {
    if len != a.size {
        return Err(Error::Mismatch(format!("size {len}, expected {}", a.size)));
    }
    if !sha_hex.eq_ignore_ascii_case(&a.sha256) {
        return Err(Error::Mismatch("SHA-256 differs".into()));
    }
    Ok(())
}

/// Last URL path segment, reduced to safe file-name characters.
fn file_name(url: &str) -> String {
    let last = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    let n: String = last
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let n = n.trim_start_matches('.').to_string();
    if n.is_empty() {
        "update.bin".into()
    } else {
        n
    }
}

struct HashWriter {
    inner: std::fs::File,
    hash: Sha256,
    len: u64,
}

impl Write for HashWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(b)?;
        self.hash.update(&b[..n]);
        self.len += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// What [`apply`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// The running program file was replaced at this path. Restart to use it.
    Replaced(PathBuf),
    /// An OS installer or viewer was started for the file.
    HandedOff(PathBuf),
}

/// The program to start for a downloaded installer: `(program, args)`.
/// `None` for formats that replace the running file instead.
pub fn handoff_command(os: &str, format: &str, file: &Path) -> Option<(String, Vec<String>)> {
    let f = file.to_string_lossy().into_owned();
    match (os, format) {
        ("macos", "dmg") => Some(("open".into(), vec![f])),
        ("windows", "msi") => Some(("msiexec".into(), vec!["/i".into(), f, "/passive".into()])),
        ("windows", "msix") => Some((
            "powershell".into(),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!("Add-AppxPackage -Path '{}'", f.replace('\'', "''")),
            ],
        )),
        ("linux", "deb" | "rpm") => Some(("xdg-open".into(), vec![f])),
        _ => None,
    }
}

/// Install a downloaded, already verified file for the running OS.
pub fn apply(file: &Path, artifact: &Artifact) -> Result<Applied> {
    if let Some((prog, args)) = handoff_command(std::env::consts::OS, &artifact.format, file) {
        std::process::Command::new(&prog)
            .args(&args)
            .spawn()
            .map_err(io(format!("starting {prog}")))?;
        return Ok(Applied::HandedOff(file.to_path_buf()));
    }
    let target = match artifact.format.as_str() {
        "appimage" => std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .ok_or_else(|| Error::NoArtifact("not running from an AppImage".into()))?,
        _ => std::env::current_exe().map_err(io("locating the running program"))?,
    };
    replace_file(file, &target)?;
    Ok(Applied::Replaced(target))
}

/// Swap `new` in for `target`: stage next to it (same filesystem, so the final
/// rename is atomic), keep the old file as `<target>.old` until the swap
/// succeeds. Windows lets a running `.exe` be renamed but not deleted, so the
/// leftover `.old` is removed by [`cleanup_old`] on the next start.
pub fn replace_file(new: &Path, target: &Path) -> Result<()> {
    let staged = sibling(target, "new");
    let old = sibling(target, "old");
    std::fs::copy(new, &staged).map_err(io("staging the update"))?;
    if let Ok(meta) = std::fs::metadata(target) {
        std::fs::set_permissions(&staged, meta.permissions()).map_err(io("copying permissions"))?;
    }
    std::fs::remove_file(&old).ok();
    let had_old = std::fs::rename(target, &old).is_ok();
    if let Err(e) = std::fs::rename(&staged, target) {
        if had_old {
            std::fs::rename(&old, target).ok();
        }
        std::fs::remove_file(&staged).ok();
        return Err(io("replacing the program")(e));
    }
    std::fs::remove_file(&old).ok();
    Ok(())
}

/// Delete a leftover `<exe>.old` from a previous update. Call once at startup.
pub fn cleanup_old() {
    if let Ok(exe) = std::env::current_exe() {
        std::fs::remove_file(sibling(&exe, "old")).ok();
    }
}

fn sibling(p: &Path, ext: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// In-memory transport that counts requests.
    struct Mock {
        files: HashMap<String, Vec<u8>>,
        hits: RefCell<Vec<String>>,
    }
    impl Fetch for Mock {
        fn get(&self, url: &str) -> Result<Vec<u8>> {
            self.hits.borrow_mut().push(url.into());
            self.files
                .get(url)
                .cloned()
                .ok_or_else(|| Error::Network(format!("404 {url}")))
        }
        fn post(&self, _: &str, _: &str, _: &[u8]) -> Result<()> {
            Ok(())
        }
    }

    const EP: &str = "https://u.example/latest.json";

    fn release(version: &str, payload: &[u8], k: &keys::KeyPair) -> Mock {
        let sha = keys::hex(&Sha256::digest(payload));
        let m = Manifest {
            schema: 1,
            app: "dev.x.app".into(),
            version: version.into(),
            notes: "n".into(),
            artifacts: vec![
                Artifact {
                    os: "linux".into(),
                    arch: "x86_64".into(),
                    format: "appimage".into(),
                    url: "https://u.example/app-2.AppImage".into(),
                    sha256: sha,
                    size: payload.len() as u64,
                },
                Artifact {
                    os: "linux".into(),
                    arch: "x86_64".into(),
                    format: "deb".into(),
                    url: "https://u.example/app.deb".into(),
                    sha256: "0".repeat(64),
                    size: 1,
                },
            ],
        };
        let json = m.to_json();
        let sig = keys::sign(&k.private_hex, json.as_bytes()).unwrap();
        let mut files = HashMap::new();
        files.insert(EP.to_string(), json.into_bytes());
        files.insert(format!("{EP}.sig"), sig.into_bytes());
        files.insert("https://u.example/app-2.AppImage".into(), payload.to_vec());
        Mock {
            files,
            hits: RefCell::new(vec![]),
        }
    }

    fn cfg(k: &keys::KeyPair, current: &str) -> Config {
        let mut c = Config::new("dev.x.app", current, EP, &k.public_hex);
        c.os = "linux".into();
        c.arch = "x86_64".into();
        c.formats = vec!["appimage".into()];
        c
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("alloy-ship-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn offers_only_newer_versions_for_this_platform() {
        let k = keys::generate().unwrap();
        let u = Updater::new(cfg(&k, "1.0.0"), release("2.0.0", b"new", &k));
        let up = u.check().unwrap().expect("update");
        assert_eq!(
            (up.version.as_str(), up.artifact.format.as_str()),
            ("2.0.0", "appimage")
        );
        assert!(Updater::new(cfg(&k, "2.0.0"), release("2.0.0", b"new", &k))
            .check()
            .unwrap()
            .is_none());
        assert!(Updater::new(cfg(&k, "3.0.0"), release("2.0.0", b"new", &k))
            .check()
            .unwrap()
            .is_none());
        // A package-managed install (no acceptable formats) is never offered one.
        let mut c = cfg(&k, "1.0.0");
        c.formats.clear();
        assert!(Updater::new(c, release("2.0.0", b"new", &k))
            .check()
            .unwrap()
            .is_none());
        let mut c = cfg(&k, "1.0.0");
        c.arch = "riscv64".into();
        assert!(Updater::new(c, release("2.0.0", b"new", &k))
            .check()
            .unwrap()
            .is_none());
    }

    #[test]
    fn rejects_wrong_key_wrong_app_and_tampered_manifest() {
        let k = keys::generate().unwrap();
        let other = keys::generate().unwrap();
        assert!(matches!(
            Updater::new(cfg(&other, "1.0.0"), release("2.0.0", b"x", &k)).check(),
            Err(Error::BadSignature)
        ));
        let mut c = cfg(&k, "1.0.0");
        c.app = "dev.other".into();
        assert!(matches!(
            Updater::new(c, release("2.0.0", b"x", &k)).check(),
            Err(Error::BadManifest(_))
        ));
        let mut mock = release("2.0.0", b"x", &k);
        let body = String::from_utf8(mock.files[EP].clone())
            .unwrap()
            .replace("u.example", "evil.test");
        mock.files.insert(EP.into(), body.into_bytes());
        assert!(matches!(
            Updater::new(cfg(&k, "1.0.0"), mock).check(),
            Err(Error::BadSignature)
        ));
    }

    #[test]
    fn download_verifies_and_cleans_up() {
        let k = keys::generate().unwrap();
        let dir = tmp("dl");
        let u = Updater::new(cfg(&k, "1.0.0"), release("2.0.0", b"payload", &k));
        let up = u.check().unwrap().unwrap();
        let f = u.download(&up, &dir).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"payload");
        // Same size, different bytes: signed hash catches it.
        let mut mock = release("2.0.0", b"payload", &k);
        mock.files.insert(
            "https://u.example/app-2.AppImage".into(),
            b"PAYLOAD".to_vec(),
        );
        let u = Updater::new(cfg(&k, "1.0.0"), mock);
        let up = u.check().unwrap().unwrap();
        let dir2 = tmp("dl2");
        assert!(matches!(u.download(&up, &dir2), Err(Error::Mismatch(_))));
        assert_eq!(
            std::fs::read_dir(&dir2).unwrap().count(),
            0,
            "no partial file left"
        );
        // Wrong length.
        let mut mock = release("2.0.0", b"payload", &k);
        mock.files
            .insert("https://u.example/app-2.AppImage".into(), b"pay".to_vec());
        let u = Updater::new(cfg(&k, "1.0.0"), mock);
        let up = u.check().unwrap().unwrap();
        assert!(matches!(u.download(&up, &dir2), Err(Error::Mismatch(m)) if m.contains("size")));
    }

    #[test]
    fn replace_file_swaps_and_keeps_mode() {
        let d = tmp("swap");
        let (target, new) = (d.join("app"), d.join("download"));
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        replace_file(&new, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(!sibling(&target, "old").exists() && !sibling(&target, "new").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }

    #[test]
    fn handoff_commands() {
        let p = Path::new("/t/a.msi");
        let (prog, args) = handoff_command("windows", "msi", p).unwrap();
        assert_eq!((prog.as_str(), args[0].as_str()), ("msiexec", "/i"));
        assert_eq!(
            handoff_command("macos", "dmg", Path::new("/t/a.dmg"))
                .unwrap()
                .0,
            "open"
        );
        assert!(handoff_command("linux", "appimage", p).is_none());
        let (_, a) = handoff_command("windows", "msix", Path::new("C:\\it's.msix")).unwrap();
        assert!(a[2].contains("it''s"), "quote doubled: {}", a[2]);
    }

    #[test]
    fn file_names_are_sanitised() {
        assert_eq!(
            file_name("https://x/y/app-1.AppImage?sig=1"),
            "app-1.AppImage"
        );
        assert_eq!(file_name("https://x/"), "update.bin");
        let n = file_name("https://x/..%2f..%2fetc");
        assert!(!n.contains('/') && !n.starts_with('.'), "{n}");
    }
}
