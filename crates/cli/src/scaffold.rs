//! `alloy new`: write a starter app (a counter) that builds and bundles.

use crate::error::{io, Error, Result};
use crate::icon::rounded_square_png;
use std::path::{Path, PathBuf};

/// How the generated app depends on the Alloy crates.
#[derive(Debug, Clone)]
pub enum Source {
    /// Use the git repository (no crates.io release yet).
    Git,
    /// Use a local checkout of the Alloy workspace.
    Path(PathBuf),
}

/// Options for [`create`].
#[derive(Debug, Clone)]
pub struct NewApp {
    /// Directory (and package) name.
    pub name: String,
    /// Reverse-DNS identifier; derived from the name when `None`.
    pub identifier: Option<String>,
    /// Where the Alloy crates come from.
    pub source: Source,
}

/// Check a crate-style name: lowercase letters, digits, `-` and `_`,
/// starting with a letter.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(Error::Usage(format!(
            "`{name}` is not a valid app name: use lowercase letters, digits, `-` and `_`, starting with a letter"
        )))
    }
}

/// `my-cool_app` -> `My Cool App`.
pub fn title_case(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const MAIN_RS: &str = include_str!("template/main.rs.txt");
const CAPS: &str = "\
# What this app may ask the platform for. Everything not listed is denied.
# Options: clipboard, dialogs, fs_read, fs_write, network, notifications,
# open_external. fs_* and network need a scope table; see alloy-ui-ipc docs.
capabilities = [\"clipboard\"]
";

fn cargo_toml(a: &NewApp, identifier: &str) -> String {
    let deps = match &a.source {
        Source::Git => "alloy-ui = { git = \"https://github.com/adswill/alloy-ui\" }\n".to_string(),
        Source::Path(p) => {
            let root = p.display().to_string().replace('\\', "/");
            format!("alloy-ui = {{ path = \"{root}/crates/alloy\" }}\n")
        }
    };
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\ndescription = \"An Alloy app\"\n\n[dependencies]\n{deps}\n[package.metadata.alloy]\nidentifier = \"{identifier}\"\nproduct-name = \"{title}\"\nicons = [\"assets/icon-256.png\", \"assets/icon-512.png\"]\ncapabilities = \"alloy.capabilities.toml\"\n\n[profile.release]\nopt-level = \"z\"\nlto = \"fat\"\ncodegen-units = 1\npanic = \"abort\"\nstrip = true\n",
        name = a.name,
        title = title_case(&a.name),
    )
}

fn write(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(io(format!("creating {}", p.display())))?;
    }
    std::fs::write(path, data).map_err(io(format!("writing {}", path.display())))
}

/// Create the app under `parent/<name>` and return its directory.
pub fn create(parent: &Path, a: &NewApp) -> Result<PathBuf> {
    validate_name(&a.name)?;
    let dir = parent.join(&a.name);
    if dir.exists() {
        return Err(Error::Usage(format!("{} already exists", dir.display())));
    }
    let identifier = a
        .identifier
        .clone()
        .unwrap_or_else(|| format!("dev.example.{}", a.name.replace(['_', '-'], "")));
    // Fail before writing anything if the identifier is unusable.
    let probe = serde_json::json!({
        "target_directory": "/t",
        "packages": [{"name": a.name, "version": "0.1.0", "manifest_path": "/x/Cargo.toml",
            "targets": [{"name": a.name, "kind": ["bin"]}],
            "metadata": {"alloy": {"identifier": identifier}}}]
    });
    crate::project::Project::from_metadata(
        &probe,
        &crate::project::Selector {
            package: Some(a.name.clone()),
            ..Default::default()
        },
    )?;
    write(
        &dir.join("Cargo.toml"),
        cargo_toml(a, &identifier).as_bytes(),
    )?;
    write(
        &dir.join("src/main.rs"),
        MAIN_RS.replace("@TITLE@", &title_case(&a.name)).as_bytes(),
    )?;
    write(&dir.join("alloy.capabilities.toml"), CAPS.as_bytes())?;
    write(&dir.join(".gitignore"), b"/target\n")?;
    let accent = [0x2d, 0x6c, 0xdf, 0xff];
    for size in [256, 512] {
        write(
            &dir.join(format!("assets/icon-{size}.png")),
            &rounded_square_png(size, accent),
        )?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("alloy-new-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn names() {
        assert!(validate_name("my-app_2").is_ok());
        for bad in ["", "2app", "My", "a b", "a/b", "-a"] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
        assert_eq!(title_case("my-cool_app"), "My Cool App");
    }

    #[test]
    fn creates_project_files() {
        let t = tmp("ok");
        let d = create(
            &t,
            &NewApp {
                name: "my-app".into(),
                identifier: None,
                source: Source::Path(PathBuf::from("/src/alloy")),
            },
        )
        .unwrap();
        let toml = std::fs::read_to_string(d.join("Cargo.toml")).unwrap();
        assert!(toml.contains("identifier = \"dev.example.myapp\""));
        assert!(toml.contains("alloy-ui = { path = \"/src/alloy/crates/alloy\" }"));
        assert!(toml.contains("product-name = \"My App\""));
        let main = std::fs::read_to_string(d.join("src/main.rs")).unwrap();
        assert!(main.contains("\"My App\"") && !main.contains("@TITLE@"));
        for f in [
            "alloy.capabilities.toml",
            ".gitignore",
            "assets/icon-256.png",
            "assets/icon-512.png",
        ] {
            assert!(d.join(f).exists(), "{f}");
        }
        let caps = std::fs::read_to_string(d.join("alloy.capabilities.toml")).unwrap();
        alloy_ui_ipc::CapabilitySet::from_toml(&caps).unwrap();
        // Refuses to overwrite.
        let again = create(
            &t,
            &NewApp {
                name: "my-app".into(),
                identifier: None,
                source: Source::Git,
            },
        );
        assert!(again.is_err());
        std::fs::remove_dir_all(t).ok();
    }

    #[test]
    fn bad_identifier_writes_nothing() {
        let t = tmp("badid");
        let r = create(
            &t,
            &NewApp {
                name: "x".into(),
                identifier: Some("nodots".into()),
                source: Source::Git,
            },
        );
        assert!(r.is_err());
        assert!(!t.join("x").exists());
        std::fs::remove_dir_all(t).ok();
    }
}
