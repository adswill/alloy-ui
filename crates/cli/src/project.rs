//! Reading app metadata: `cargo metadata` plus the `[package.metadata.alloy]`
//! table, and the capability config it points at.
//!
//! ```toml
//! [package.metadata.alloy]
//! identifier   = "dev.example.counter"   # reverse-DNS, required
//! product-name = "Counter"               # defaults to the package name
//! publisher    = "Example Inc."          # optional
//! category     = "Utility"               # freedesktop main category
//! icons        = ["assets/icon-256.png", "assets/icon-512.png"]
//! capabilities = "alloy.capabilities.toml"
//! ```

use crate::error::{io, Error, Result};
use alloy_ui_ipc::{Capability, CapabilitySet};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Freedesktop main categories accepted for `category`.
pub const CATEGORIES: [&str; 13] = [
    "AudioVideo",
    "Audio",
    "Video",
    "Development",
    "Education",
    "Game",
    "Graphics",
    "Network",
    "Office",
    "Science",
    "Settings",
    "System",
    "Utility",
];

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct RawMeta {
    identifier: Option<String>,
    product_name: Option<String>,
    publisher: Option<String>,
    description: Option<String>,
    category: Option<String>,
    copyright: Option<String>,
    #[serde(default)]
    icons: Vec<String>,
    capabilities: Option<String>,
    min_macos: Option<String>,
    #[serde(default)]
    deb_depends: Option<Vec<String>>,
}

/// Validated app metadata plus package facts from Cargo.
#[derive(Debug, Clone)]
pub struct Project {
    /// Path to the app's Cargo.toml.
    pub manifest_path: PathBuf,
    /// Cargo package name.
    pub package_name: String,
    /// Name of the binary target.
    pub bin_name: String,
    /// Package version.
    pub version: String,
    /// One-line description.
    pub description: String,
    /// Cargo `authors`.
    pub authors: Vec<String>,
    /// SPDX licence expression, if any.
    pub license: Option<String>,
    /// Homepage or repository URL, if any.
    pub homepage: Option<String>,
    /// Cargo target directory.
    pub target_dir: PathBuf,
    /// Reverse-DNS app identifier.
    pub identifier: String,
    /// Human-facing app name.
    pub product_name: String,
    /// Publisher / vendor name.
    pub publisher: Option<String>,
    /// Freedesktop main category (canonical casing).
    pub category: String,
    /// Copyright line.
    pub copyright: Option<String>,
    /// Absolute icon paths.
    pub icons: Vec<PathBuf>,
    /// Absolute path of the capability config, if configured.
    pub capabilities_path: Option<PathBuf>,
    /// Minimum macOS version.
    pub min_macos: String,
    /// Debian `Depends` entries.
    pub deb_depends: Vec<String>,
}

/// Which package/binary to act on.
#[derive(Debug, Clone, Default)]
pub struct Selector {
    /// `--manifest-path`.
    pub manifest_path: Option<PathBuf>,
    /// `-p/--package`.
    pub package: Option<String>,
    /// `--bin`.
    pub bin: Option<String>,
}

impl Project {
    /// Locate and load the project described by `sel`.
    pub fn load(sel: &Selector) -> Result<Self> {
        let mut cmd = Command::new(cargo());
        cmd.args(["metadata", "--no-deps", "--format-version", "1"]);
        if let Some(m) = &sel.manifest_path {
            cmd.arg("--manifest-path").arg(m);
        }
        let out = cmd.output().map_err(io("running cargo metadata"))?;
        if !out.status.success() {
            return Err(Error::Metadata(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        let json: serde_json::Value =
            serde_json::from_slice(&out.stdout).map_err(|e| Error::Metadata(e.to_string()))?;
        Self::from_metadata(&json, sel)
    }

    /// Build a project from parsed `cargo metadata` output.
    pub fn from_metadata(json: &serde_json::Value, sel: &Selector) -> Result<Self> {
        let pkgs = json["packages"]
            .as_array()
            .ok_or_else(|| Error::Metadata("no `packages` array".into()))?;
        let want_manifest = match &sel.manifest_path {
            Some(m) => Some(m.canonicalize().map_err(io(format!("{}", m.display())))?),
            None => std::env::current_dir()
                .ok()
                .map(|d| d.join("Cargo.toml"))
                .and_then(|p| p.canonicalize().ok()),
        };
        let pkg = if let Some(name) = &sel.package {
            pkgs.iter()
                .find(|p| p["name"] == name.as_str())
                .ok_or_else(|| Error::Config(format!("no package named `{name}` in workspace")))?
        } else if let Some(p) = want_manifest.as_ref().and_then(|w| {
            pkgs.iter().find(|p| {
                p["manifest_path"]
                    .as_str()
                    .and_then(|s| Path::new(s).canonicalize().ok())
                    .as_ref()
                    == Some(w)
            })
        }) {
            p
        } else {
            let cands: Vec<_> = pkgs
                .iter()
                .filter(|p| p["metadata"]["alloy"].is_object())
                .collect();
            match cands.as_slice() {
                [one] => *one,
                [] => {
                    return Err(Error::Config(
                        "no package has a `[package.metadata.alloy]` table".into(),
                    ))
                }
                many => {
                    let names: Vec<_> = many.iter().filter_map(|p| p["name"].as_str()).collect();
                    return Err(Error::Usage(format!(
                        "several Alloy apps found ({}); choose one with -p",
                        names.join(", ")
                    )));
                }
            }
        };
        let name = str_of(pkg, "name")?;
        let raw: RawMeta = match pkg["metadata"].get("alloy") {
            Some(v) if !v.is_null() => serde_json::from_value(v.clone())
                .map_err(|e| Error::Config(format!("[package.metadata.alloy] in `{name}`: {e}")))?,
            _ => {
                return Err(Error::Config(format!(
                "package `{name}` has no `[package.metadata.alloy]` table (identifier is required)"
            )))
            }
        };
        let manifest_path = PathBuf::from(str_of(pkg, "manifest_path")?);
        let base = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let bins: Vec<&str> = pkg["targets"]
            .as_array()
            .map(|t| {
                t.iter()
                    .filter(|t| {
                        t["kind"]
                            .as_array()
                            .is_some_and(|k| k.iter().any(|k| k == "bin"))
                    })
                    .filter_map(|t| t["name"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        let bin_name = match &sel.bin {
            Some(b) if bins.contains(&b.as_str()) => b.clone(),
            Some(b) => {
                return Err(Error::Config(format!(
                    "package `{name}` has no binary `{b}` (has: {})",
                    bins.join(", ")
                )))
            }
            None => match bins.as_slice() {
                [] => {
                    return Err(Error::Config(format!(
                        "package `{name}` has no binary target"
                    )))
                }
                [one] => one.to_string(),
                many => {
                    if many.contains(&name.as_str()) {
                        name.clone()
                    } else {
                        return Err(Error::Usage(format!(
                            "several binaries ({}); choose one with --bin",
                            many.join(", ")
                        )));
                    }
                }
            },
        };
        let identifier = raw.identifier.ok_or_else(|| {
            Error::Config("`identifier` is required in [package.metadata.alloy]".into())
        })?;
        validate_identifier(&identifier)?;
        let product_name = raw.product_name.unwrap_or_else(|| name.clone());
        validate_product_name(&product_name)?;
        let category = match raw.category {
            None => "Utility".to_string(),
            Some(c) => CATEGORIES
                .iter()
                .find(|k| k.eq_ignore_ascii_case(&c))
                .map(|k| k.to_string())
                .ok_or_else(|| {
                    Error::Config(format!(
                        "unknown category `{c}`; use one of {}",
                        CATEGORIES.join(", ")
                    ))
                })?,
        };
        let description = raw
            .description
            .or_else(|| pkg["description"].as_str().map(str::to_string))
            .unwrap_or_else(|| product_name.clone());
        let authors = pkg["authors"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let homepage = pkg["homepage"]
            .as_str()
            .or_else(|| pkg["repository"].as_str())
            .map(str::to_string);
        Ok(Project {
            package_name: name,
            bin_name,
            version: str_of(pkg, "version")?,
            description,
            authors,
            license: pkg["license"].as_str().map(str::to_string),
            homepage,
            target_dir: PathBuf::from(str_of(json, "target_directory")?),
            identifier,
            product_name,
            publisher: raw.publisher,
            category,
            copyright: raw.copyright,
            icons: raw.icons.iter().map(|i| base.join(i)).collect(),
            capabilities_path: raw.capabilities.map(|c| base.join(c)),
            min_macos: raw.min_macos.unwrap_or_else(|| "11.0".into()),
            deb_depends: raw.deb_depends.unwrap_or_else(|| {
                ["libc6", "libxkbcommon0", "libvulkan1 | libgl1"]
                    .map(String::from)
                    .to_vec()
            }),
            manifest_path,
        })
    }

    /// Read and validate the capability config, returning the set and its
    /// verbatim source text. `Ok(None)` when none is configured.
    pub fn load_capabilities(&self) -> Result<Option<(CapabilitySet, String)>> {
        let Some(path) = &self.capabilities_path else {
            return Ok(None);
        };
        let text = std::fs::read_to_string(path)
            .map_err(io(format!("reading capability config {}", path.display())))?;
        let set = CapabilitySet::from_toml(&text).map_err(|source| Error::Capabilities {
            path: path.clone(),
            source,
        })?;
        Ok(Some((set, text)))
    }

    /// A filesystem-safe name for artifacts, e.g. `my-app`.
    pub fn slug(&self) -> String {
        self.product_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_ascii_lowercase()
    }
}

/// One-line human summary of a granted set, for build output.
pub fn describe_capabilities(set: &CapabilitySet) -> String {
    const ALL: [Capability; 7] = [
        Capability::Clipboard,
        Capability::Dialogs,
        Capability::FsRead,
        Capability::FsWrite,
        Capability::Network,
        Capability::Notifications,
        Capability::OpenExternal,
    ];
    let names: Vec<&str> = ALL
        .iter()
        .filter(|c| set.has(**c))
        .map(|c| c.name())
        .collect();
    if names.is_empty() {
        "none (app is fully sandboxed)".into()
    } else {
        names.join(", ")
    }
}

fn str_of(v: &serde_json::Value, k: &str) -> Result<String> {
    v[k].as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Metadata(format!("missing `{k}`")))
}

pub(crate) fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

fn validate_identifier(id: &str) -> Result<()> {
    let parts: Vec<&str> = id.split('.').collect();
    let ok = parts.len() >= 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !p.starts_with('-')
        });
    if ok {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "identifier `{id}` must be reverse-DNS with at least two dot-separated parts of letters, digits and `-` (e.g. dev.example.app)"
        )))
    }
}

fn validate_product_name(n: &str) -> Result<()> {
    if n.trim().is_empty()
        || n != n.trim()
        || n.chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|$".contains(c))
    {
        return Err(Error::Config(format!(
            "product-name `{n}` must be non-empty, have no leading/trailing spaces, and avoid / \\ : * ? \" < > | $"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn meta(alloy: serde_json::Value) -> serde_json::Value {
        json!({
            "target_directory": "/t",
            "packages": [{
                "name": "counter", "version": "1.2.3", "description": "A counter",
                "authors": ["Ann <ann@example.com>"], "license": "MIT",
                "manifest_path": "/app/Cargo.toml",
                "targets": [{"name": "counter", "kind": ["bin"]}],
                "metadata": {"alloy": alloy}
            }]
        })
    }

    fn sel() -> Selector {
        Selector {
            package: Some("counter".into()),
            ..Default::default()
        }
    }

    #[test]
    fn loads_minimal() {
        let p = Project::from_metadata(&meta(json!({"identifier": "dev.example.counter"})), &sel())
            .unwrap();
        assert_eq!(p.bin_name, "counter");
        assert_eq!(p.product_name, "counter");
        assert_eq!(p.category, "Utility");
        assert_eq!(p.min_macos, "11.0");
        assert_eq!(p.description, "A counter");
        assert_eq!(p.slug(), "counter");
    }

    #[test]
    fn resolves_relative_paths_against_manifest() {
        let p = Project::from_metadata(
            &meta(json!({"identifier": "dev.x.y", "icons": ["a/i.png"], "capabilities": "c.toml"})),
            &sel(),
        )
        .unwrap();
        assert_eq!(p.icons, vec![PathBuf::from("/app/a/i.png")]);
        assert_eq!(p.capabilities_path, Some(PathBuf::from("/app/c.toml")));
    }

    #[test]
    fn category_is_case_insensitive() {
        let p = Project::from_metadata(
            &meta(json!({"identifier": "dev.x.y", "category": "graphics"})),
            &sel(),
        )
        .unwrap();
        assert_eq!(p.category, "Graphics");
    }

    #[test]
    fn rejects_bad_config() {
        let bad = [
            json!({}),
            json!({"identifier": "nodots"}),
            json!({"identifier": "a..b"}),
            json!({"identifier": "dev.x.y", "product-name": "a/b"}),
            json!({"identifier": "dev.x.y", "product-name": " x"}),
            json!({"identifier": "dev.x.y", "category": "Nonsense"}),
            json!({"identifier": "dev.x.y", "unknown-key": 1}),
        ];
        for b in bad {
            assert!(
                matches!(
                    Project::from_metadata(&meta(b.clone()), &sel()),
                    Err(Error::Config(_))
                ),
                "{b}"
            );
        }
    }

    #[test]
    fn missing_table_and_unknown_package() {
        let mut m = meta(json!(null));
        assert!(Project::from_metadata(&m, &sel()).is_err());
        m["packages"][0]["metadata"] = json!({});
        assert!(Project::from_metadata(&m, &sel()).is_err());
        let s = Selector {
            package: Some("nope".into()),
            ..Default::default()
        };
        assert!(Project::from_metadata(&meta(json!({"identifier":"a.b"})), &s).is_err());
    }

    #[test]
    fn bin_selection() {
        let mut m = meta(json!({"identifier": "dev.x.y"}));
        m["packages"][0]["targets"] =
            json!([{"name": "a", "kind": ["bin"]}, {"name": "b", "kind": ["bin"]}]);
        assert!(matches!(
            Project::from_metadata(&m, &sel()),
            Err(Error::Usage(_))
        ));
        let s = Selector {
            bin: Some("b".into()),
            ..sel()
        };
        assert_eq!(Project::from_metadata(&m, &s).unwrap().bin_name, "b");
        let s = Selector {
            bin: Some("zz".into()),
            ..sel()
        };
        assert!(Project::from_metadata(&m, &s).is_err());
    }

    #[test]
    fn capability_summary() {
        let set = CapabilitySet::from_toml(r#"capabilities=["clipboard","dialogs"]"#).unwrap();
        assert_eq!(describe_capabilities(&set), "clipboard, dialogs");
        assert!(describe_capabilities(&CapabilitySet::none()).starts_with("none"));
    }

    #[test]
    fn invalid_capability_file_is_reported() {
        let dir = std::env::temp_dir().join(format!("alloy-caps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("c.toml");
        std::fs::write(&f, "capabilities=[\"teleport\"]").unwrap();
        let mut p =
            Project::from_metadata(&meta(json!({"identifier": "dev.x.y"})), &sel()).unwrap();
        p.capabilities_path = Some(f.clone());
        assert!(matches!(
            p.load_capabilities(),
            Err(Error::Capabilities { .. })
        ));
        std::fs::write(&f, "capabilities=[\"clipboard\"]").unwrap();
        assert!(p.load_capabilities().unwrap().is_some());
        std::fs::remove_dir_all(dir).ok();
    }
}
