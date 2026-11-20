//! Capability/permissions config: parsing, validation and checks.
//!
//! Example file:
//!
//! ```toml
//! capabilities = ["clipboard", "fs_read", "fs_write", "network"]
//!
//! [fs]
//! read = ["/home/me/docs"]
//! write = ["/home/me/docs/out"]
//!
//! [network]
//! hosts = ["api.example.com", "*.cdn.example.org"]
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// A capability an app can be granted in its config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Read and write the clipboard.
    Clipboard,
    /// Open native file dialogs.
    Dialogs,
    /// Read the filesystem (scoped by `[fs] read`).
    FsRead,
    /// Write the filesystem (scoped by `[fs] write`).
    FsWrite,
    /// Make network requests (scoped by `[network] hosts`).
    Network,
    /// Show system notifications.
    Notifications,
    /// Open URLs and files with the system default handler.
    OpenExternal,
}

impl Capability {
    /// The name used in the config file.
    pub fn name(self) -> &'static str {
        match self {
            Capability::Clipboard => "clipboard",
            Capability::Dialogs => "dialogs",
            Capability::FsRead => "fs_read",
            Capability::FsWrite => "fs_write",
            Capability::Network => "network",
            Capability::Notifications => "notifications",
            Capability::OpenExternal => "open_external",
        }
    }
}

/// Errors from parsing, validating or checking capabilities.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CapabilityError {
    /// The TOML was malformed or had unknown fields/capabilities.
    #[error("invalid capability config: {0}")]
    Parse(String),
    /// A capability appears more than once.
    #[error("capability `{}` is listed more than once", .0.name())]
    Duplicate(Capability),
    /// A scoped capability was granted with no scope entries.
    #[error("capability `{}` is granted but its scope list is empty", .0.name())]
    EmptyScope(Capability),
    /// A scope section exists without the matching capability being granted.
    #[error("scope given for `{}` but the capability is not granted", .0.name())]
    ScopeWithoutGrant(Capability),
    /// A filesystem scope path is not absolute or contains `..`.
    #[error("invalid fs path `{0}`: must be absolute and contain no `..`")]
    BadPath(String),
    /// A network host pattern is malformed.
    #[error("invalid host pattern `{0}`")]
    BadHost(String),
    /// The capability was not granted.
    #[error("capability `{}` was not granted", .0.name())]
    Denied(Capability),
    /// The capability is granted but the path is outside the allowed scope.
    #[error("path `{path}` is outside the granted `{}` scope", cap.name())]
    PathDenied {
        /// Which fs capability was checked.
        cap: Capability,
        /// The rejected path.
        path: String,
    },
    /// The capability is granted but the host is not allowed.
    #[error("host `{0}` is not in the granted network scope")]
    HostDenied(String),
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FsSection {
    #[serde(default)]
    read: Vec<String>,
    #[serde(default)]
    write: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct NetSection {
    #[serde(default)]
    hosts: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    capabilities: Vec<Capability>,
    #[serde(default)]
    fs: FsSection,
    #[serde(default)]
    network: NetSection,
}

/// A validated set of granted capabilities with their scopes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilitySet {
    granted: Vec<Capability>,
    fs_read: Vec<PathBuf>,
    fs_write: Vec<PathBuf>,
    hosts: Vec<String>,
}

fn normalize(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => return None,
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Some(out)
}

fn valid_host(h: &str) -> bool {
    let body = h.strip_prefix("*.").unwrap_or(h);
    !body.is_empty()
        && body.contains(|c: char| c.is_ascii_alphanumeric())
        && !body.starts_with(['.', '-'])
        && !body.ends_with(['.', '-'])
        && !body.contains("..")
        && body
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

impl CapabilitySet {
    /// An empty set that grants nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// Parse and validate a TOML config string.
    pub fn from_toml(src: &str) -> Result<Self, CapabilityError> {
        let raw: RawConfig =
            toml::from_str(src).map_err(|e| CapabilityError::Parse(e.message().to_string()))?;
        Self::validate(raw)
    }

    /// Read, parse and validate a TOML config file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, CapabilityError> {
        let src = std::fs::read_to_string(path.as_ref())
            .map_err(|e| CapabilityError::Parse(format!("{}: {e}", path.as_ref().display())))?;
        Self::from_toml(&src)
    }

    fn validate(raw: RawConfig) -> Result<Self, CapabilityError> {
        let mut granted: Vec<Capability> = Vec::new();
        for c in &raw.capabilities {
            if granted.contains(c) {
                return Err(CapabilityError::Duplicate(*c));
            }
            granted.push(*c);
        }
        let scopes = [
            (Capability::FsRead, raw.fs.read.is_empty()),
            (Capability::FsWrite, raw.fs.write.is_empty()),
            (Capability::Network, raw.network.hosts.is_empty()),
        ];
        for (cap, empty) in scopes {
            match (granted.contains(&cap), empty) {
                (true, true) => return Err(CapabilityError::EmptyScope(cap)),
                (false, false) => return Err(CapabilityError::ScopeWithoutGrant(cap)),
                _ => {}
            }
        }
        let paths = |v: &[String]| -> Result<Vec<PathBuf>, CapabilityError> {
            v.iter()
                .map(|s| normalize(Path::new(s)).ok_or_else(|| CapabilityError::BadPath(s.clone())))
                .collect()
        };
        let fs_read = paths(&raw.fs.read)?;
        let fs_write = paths(&raw.fs.write)?;
        let mut hosts = Vec::new();
        for h in &raw.network.hosts {
            let l = h.to_ascii_lowercase();
            if !valid_host(&l) {
                return Err(CapabilityError::BadHost(h.clone()));
            }
            hosts.push(l);
        }
        Ok(Self {
            granted,
            fs_read,
            fs_write,
            hosts,
        })
    }

    /// Whether the capability is granted at all (ignoring scope).
    pub fn has(&self, cap: Capability) -> bool {
        self.granted.contains(&cap)
    }

    /// Kind-level check: `Ok` if the capability is granted.
    pub fn check(&self, cap: &Capability) -> Result<(), CapabilityError> {
        if self.has(*cap) {
            Ok(())
        } else {
            Err(CapabilityError::Denied(*cap))
        }
    }

    /// Check that `path` may be read (granted and under a read scope).
    pub fn check_fs_read(&self, path: &Path) -> Result<(), CapabilityError> {
        self.check_fs(Capability::FsRead, &self.fs_read, path)
    }

    /// Check that `path` may be written (granted and under a write scope).
    pub fn check_fs_write(&self, path: &Path) -> Result<(), CapabilityError> {
        self.check_fs(Capability::FsWrite, &self.fs_write, path)
    }

    fn check_fs(
        &self,
        cap: Capability,
        scope: &[PathBuf],
        path: &Path,
    ) -> Result<(), CapabilityError> {
        self.check(&cap)?;
        let denied = || CapabilityError::PathDenied {
            cap,
            path: path.display().to_string(),
        };
        let norm = normalize(path).ok_or_else(denied)?;
        if scope.iter().any(|root| norm.starts_with(root)) {
            Ok(())
        } else {
            Err(denied())
        }
    }

    /// Check that requests to `host` are allowed. Exact names match exactly;
    /// `*.example.com` matches subdomains but not `example.com` itself.
    pub fn check_host(&self, host: &str) -> Result<(), CapabilityError> {
        self.check(&Capability::Network)?;
        let h = host.to_ascii_lowercase();
        let ok = self.hosts.iter().any(|p| match p.strip_prefix("*.") {
            Some(suffix) => h.len() > suffix.len() + 1 && h.ends_with(&format!(".{suffix}")),
            None => *p == h,
        });
        if ok {
            Ok(())
        } else {
            Err(CapabilityError::HostDenied(host.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
capabilities = ["clipboard", "fs_read", "fs_write", "network"]
[fs]
read = ["/home/u/docs"]
write = ["/home/u/docs/out"]
[network]
hosts = ["api.example.com", "*.cdn.example.org"]
"#;

    #[test]
    fn parses_full() {
        let s = CapabilitySet::from_toml(FULL).unwrap();
        assert!(s.has(Capability::Clipboard));
        assert!(!s.has(Capability::Dialogs));
    }

    #[test]
    fn empty_config_grants_nothing() {
        let s = CapabilitySet::from_toml("").unwrap();
        assert_eq!(s, CapabilitySet::none());
        assert_eq!(
            s.check(&Capability::Clipboard),
            Err(CapabilityError::Denied(Capability::Clipboard))
        );
    }

    #[test]
    fn check_granted_and_denied() {
        let s = CapabilitySet::from_toml(FULL).unwrap();
        assert!(s.check(&Capability::Network).is_ok());
        assert!(s.check(&Capability::Notifications).is_err());
    }

    #[test]
    fn unknown_capability_rejected() {
        let e = CapabilitySet::from_toml(r#"capabilities = ["teleport"]"#).unwrap_err();
        assert!(matches!(e, CapabilityError::Parse(_)));
    }

    #[test]
    fn unknown_field_rejected() {
        assert!(CapabilitySet::from_toml("bogus = 1").is_err());
        assert!(CapabilitySet::from_toml("[fs]\nexec = []").is_err());
    }

    #[test]
    fn malformed_toml() {
        assert!(matches!(
            CapabilitySet::from_toml("capabilities = ["),
            Err(CapabilityError::Parse(_))
        ));
    }

    #[test]
    fn duplicate_rejected() {
        let e =
            CapabilitySet::from_toml(r#"capabilities = ["clipboard","clipboard"]"#).unwrap_err();
        assert_eq!(e, CapabilityError::Duplicate(Capability::Clipboard));
    }

    #[test]
    fn empty_scope_rejected() {
        let e = CapabilitySet::from_toml(r#"capabilities = ["fs_read"]"#).unwrap_err();
        assert_eq!(e, CapabilityError::EmptyScope(Capability::FsRead));
        let e = CapabilitySet::from_toml(r#"capabilities = ["network"]"#).unwrap_err();
        assert_eq!(e, CapabilityError::EmptyScope(Capability::Network));
    }

    #[test]
    fn scope_without_grant_rejected() {
        let e = CapabilitySet::from_toml("[fs]\nwrite = [\"/tmp\"]").unwrap_err();
        assert_eq!(e, CapabilityError::ScopeWithoutGrant(Capability::FsWrite));
    }

    #[test]
    fn relative_and_dotdot_paths_rejected() {
        let rel = "capabilities=[\"fs_read\"]\n[fs]\nread=[\"docs\"]";
        assert!(matches!(
            CapabilitySet::from_toml(rel),
            Err(CapabilityError::BadPath(_))
        ));
        let dd = "capabilities=[\"fs_read\"]\n[fs]\nread=[\"/a/../b\"]";
        assert!(matches!(
            CapabilitySet::from_toml(dd),
            Err(CapabilityError::BadPath(_))
        ));
    }

    #[test]
    fn fs_read_scope() {
        let s = CapabilitySet::from_toml(FULL).unwrap();
        assert!(s.check_fs_read(Path::new("/home/u/docs/a/b.txt")).is_ok());
        assert!(s.check_fs_read(Path::new("/home/u/docs")).is_ok());
        assert!(s.check_fs_read(Path::new("/home/u/other")).is_err());
    }

    #[test]
    fn fs_prefix_is_component_wise() {
        let s = CapabilitySet::from_toml(FULL).unwrap();
        assert!(s.check_fs_read(Path::new("/home/u/docs-secret")).is_err());
    }

    #[test]
    fn fs_traversal_denied() {
        let s = CapabilitySet::from_toml(FULL).unwrap();
        assert!(s
            .check_fs_read(Path::new("/home/u/docs/../.ssh/id"))
            .is_err());
        assert!(s.check_fs_read(Path::new("relative/path")).is_err());
    }

    #[test]
    fn fs_write_scope_is_separate() {
        let s = CapabilitySet::from_toml(FULL).unwrap();
        assert!(s.check_fs_write(Path::new("/home/u/docs/out/x")).is_ok());
        assert!(s.check_fs_write(Path::new("/home/u/docs/x")).is_err());
    }

    #[test]
    fn fs_denied_without_grant() {
        let s = CapabilitySet::none();
        assert_eq!(
            s.check_fs_read(Path::new("/x")),
            Err(CapabilityError::Denied(Capability::FsRead))
        );
    }

    #[test]
    fn host_exact_and_wildcard() {
        let s = CapabilitySet::from_toml(FULL).unwrap();
        assert!(s.check_host("api.example.com").is_ok());
        assert!(s.check_host("API.Example.COM").is_ok());
        assert!(s.check_host("x.cdn.example.org").is_ok());
        assert!(s.check_host("a.b.cdn.example.org").is_ok());
        assert!(s.check_host("cdn.example.org").is_err());
        assert!(s.check_host("evilcdn.example.org").is_err());
        assert!(s.check_host("example.com").is_err());
    }

    #[test]
    fn bad_host_patterns() {
        for h in [
            "https://a.com",
            "a.com/x",
            "",
            "*.",
            "a..com",
            "a.com:80",
            "-a.com",
        ] {
            let src = format!("capabilities=[\"network\"]\n[network]\nhosts=[\"{h}\"]");
            assert!(
                matches!(
                    CapabilitySet::from_toml(&src),
                    Err(CapabilityError::BadHost(_))
                ),
                "{h}"
            );
        }
    }

    #[test]
    fn error_messages_are_clear() {
        let e = CapabilityError::Denied(Capability::FsWrite);
        assert_eq!(e.to_string(), "capability `fs_write` was not granted");
        let s = CapabilitySet::from_toml(FULL).unwrap();
        let e = s.check_host("evil.com").unwrap_err();
        assert!(e.to_string().contains("evil.com"));
    }

    #[test]
    fn from_file_missing() {
        assert!(matches!(
            CapabilitySet::from_file("/definitely/not/here.toml"),
            Err(CapabilityError::Parse(_))
        ));
    }

    #[test]
    fn names_roundtrip_through_serde() {
        let s = CapabilitySet::from_toml(
            r#"capabilities = ["open_external", "notifications", "dialogs"]"#,
        )
        .unwrap();
        for c in [
            Capability::OpenExternal,
            Capability::Notifications,
            Capability::Dialogs,
        ] {
            assert!(s.has(c));
            assert!(!c.name().is_empty());
        }
    }
}
