//! Release metadata for the auto-updater.
//!
//! `alloy bundle` writes one *fragment* (`<slug>-<os>-<arch>.update.json`) per
//! run when the app sets `update-url`. A release is built on several machines,
//! so `alloy release --dir <dir>` later merges every fragment found under
//! `<dir>` into one `latest.json` and signs it (`latest.json.sig`) with the
//! Ed25519 key in `ALLOY_UPDATE_KEY`. On a single machine `alloy bundle` does
//! the merge itself when the key is present. The format and the signing live
//! in `alloy-ui-ship`, shared with the runtime that verifies them.

use super::{write_file, Ctx, Format};
use crate::error::{io, Error, Result};
use alloy_ui_ship::keys;
use alloy_ui_ship::manifest::{Artifact, Manifest, SCHEMA};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FRAGMENT_SUFFIX: &str = ".update.json";

#[derive(Serialize, Deserialize)]
struct Fragment {
    app: String,
    version: String,
    artifacts: Vec<Artifact>,
}

/// The OS name used in manifests, from a target triple.
pub fn os_of(triple: &str) -> &'static str {
    if triple.contains("apple") {
        "macos"
    } else if triple.contains("windows") {
        "windows"
    } else {
        "linux"
    }
}

/// Formats that are one file an updater can download and install.
fn updatable(f: Format) -> bool {
    matches!(
        f,
        Format::AppImage | Format::Dmg | Format::Msi | Format::Msix | Format::Deb | Format::Rpm
    )
}

/// Write the fragment for `built` artifacts. `Ok(None)` when the app has no
/// `update-url` or nothing built is updatable.
pub fn write_fragment(ctx: &Ctx<'_>, built: &[(Format, PathBuf)]) -> Result<Option<PathBuf>> {
    let Some(base) = &ctx.project.update_url else {
        return Ok(None);
    };
    let mut artifacts = Vec::new();
    for (f, path) in built.iter().filter(|(f, _)| updatable(*f)) {
        let data = std::fs::read(path).map_err(io(format!("reading {}", path.display())))?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        artifacts.push(Artifact {
            os: os_of(&ctx.triple).into(),
            arch: ctx.arch.clone(),
            format: f.name().into(),
            url: format!("{base}/{name}"),
            sha256: keys::hex(&super::sha256::sha256(&data)),
            size: data.len() as u64,
        });
    }
    if artifacts.is_empty() {
        return Ok(None);
    }
    let frag = Fragment {
        app: ctx.project.identifier.clone(),
        version: ctx.project.version.clone(),
        artifacts,
    };
    let path = ctx.out_dir.join(format!(
        "{}-{}-{}{FRAGMENT_SUFFIX}",
        ctx.project.slug(),
        os_of(&ctx.triple),
        ctx.arch
    ));
    write_file(
        &path,
        serde_json::to_string_pretty(&frag)
            .expect("json")
            .as_bytes(),
    )?;
    Ok(Some(path))
}

/// The signing key: the hex itself, or `@path` to a file holding it.
pub fn key_from_env(value: &str) -> Result<String> {
    match value.strip_prefix('@') {
        Some(p) => Ok(std::fs::read_to_string(p)
            .map_err(io(format!("reading update key {p}")))?
            .trim()
            .to_string()),
        None => Ok(value.trim().to_string()),
    }
}

fn find_fragments(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() && depth > 0 {
            find_fragments(&p, depth - 1, out);
        } else if p
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(FRAGMENT_SUFFIX))
        {
            out.push(p);
        }
    }
}

/// Merge fragments under `dir` (two levels deep, for CI artifact folders) into
/// a signed `latest.json` in `dir`. Returns `(manifest path, signature path)`.
pub fn merge_and_sign(dir: &Path, notes: &str, key_hex: &str) -> Result<(PathBuf, PathBuf)> {
    let mut found = Vec::new();
    find_fragments(dir, 2, &mut found);
    found.sort();
    if found.is_empty() {
        return Err(Error::Config(format!(
            "no *{FRAGMENT_SUFFIX} files under {}; run `alloy bundle` with `update-url` set first",
            dir.display()
        )));
    }
    let mut app = None;
    let mut version = None;
    let mut artifacts: Vec<Artifact> = Vec::new();
    for f in &found {
        let frag: Fragment = serde_json::from_slice(
            &std::fs::read(f).map_err(io(format!("reading {}", f.display())))?,
        )
        .map_err(|e| Error::Config(format!("{}: {e}", f.display())))?;
        for (what, have, got) in [
            ("app", &mut app, &frag.app),
            ("version", &mut version, &frag.version),
        ] {
            match have {
                None => *have = Some(got.clone()),
                Some(h) if h != got => {
                    return Err(Error::Config(format!(
                        "{} is for {what} `{got}` but other fragments are for `{h}`; do not mix releases",
                        f.display()
                    )))
                }
                _ => {}
            }
        }
        artifacts.extend(frag.artifacts);
    }
    artifacts.sort_by(|a, b| (&a.os, &a.arch, &a.format).cmp(&(&b.os, &b.arch, &b.format)));
    artifacts.dedup_by(|a, b| (&a.os, &a.arch, &a.format) == (&b.os, &b.arch, &b.format));
    let m = Manifest {
        schema: SCHEMA,
        app: app.unwrap_or_default(),
        version: version.unwrap_or_default(),
        notes: notes.into(),
        artifacts,
    };
    let json = m.to_json();
    let sig = keys::sign(key_hex, json.as_bytes()).map_err(|e| Error::Config(e.to_string()))?;
    let (mp, sp) = (dir.join("latest.json"), dir.join("latest.json.sig"));
    write_file(&mp, json.as_bytes())?;
    write_file(&sp, format!("{sig}\n").as_bytes())?;
    Ok((mp, sp))
}

/// `alloy keygen`: write a new private key to `path` (never overwriting) and
/// return the public key to embed in the app.
pub fn keygen(path: &Path) -> Result<String> {
    if path.exists() {
        return Err(Error::Config(format!(
            "{} already exists; refusing to overwrite a key",
            path.display()
        )));
    }
    let k = keys::generate().map_err(|e| Error::Config(e.to_string()))?;
    write_file(path, format!("{}\n", k.private_hex).as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(io("restricting key permissions"))?;
    }
    Ok(k.public_hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::tests_support::{fixture, fixture_for};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("alloy-rel-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn fragments_merge_into_one_signed_manifest_the_runtime_accepts() {
        let dir = tmp("merge");
        let mut built = Vec::new();
        // Two machines: Linux and Windows builds of the same release.
        for (triple, fmt, file) in [
            (
                "x86_64-unknown-linux-gnu",
                Format::AppImage,
                "demo-x86_64.AppImage",
            ),
            (
                "x86_64-pc-windows-msvc",
                Format::Msi,
                "demo-1.2.3-beta.1-x86_64.msi",
            ),
        ] {
            let mut f = fixture_for(triple);
            f.project.update_url = Some("https://dl.example.com/demo".into());
            let ctx = f.ctx();
            let art = ctx.out_dir.join(file);
            write_file(&art, file.as_bytes()).unwrap();
            built.push((fmt, art.clone()));
            let frag = write_fragment(&ctx, &[(fmt, art.clone()), (Format::Exe, art)])
                .unwrap()
                .unwrap();
            let sub = dir.join(os_of(triple));
            std::fs::create_dir_all(&sub).unwrap();
            std::fs::copy(&frag, sub.join(frag.file_name().unwrap())).unwrap();
        }
        let k = keys::generate().unwrap();
        let (mp, sp) = merge_and_sign(&dir, "Notes", &k.private_hex).unwrap();
        let m = Manifest::parse_verified(
            &std::fs::read(&mp).unwrap(),
            &std::fs::read_to_string(&sp).unwrap(),
            &k.public_hex,
        )
        .unwrap();
        assert_eq!(
            (m.app.as_str(), m.version.as_str(), m.notes.as_str()),
            ("dev.example.demo", "1.2.3-beta.1", "Notes")
        );
        assert_eq!(m.artifacts.len(), 2, "exe is not an updatable format");
        let l = &m.artifacts[0];
        assert_eq!((l.os.as_str(), l.format.as_str()), ("linux", "appimage"));
        assert_eq!(l.url, "https://dl.example.com/demo/demo-x86_64.AppImage");
        assert_eq!(l.size, "demo-x86_64.AppImage".len() as u64);
        assert_eq!(
            l.sha256,
            keys::hex(&super::super::sha256::sha256(b"demo-x86_64.AppImage"))
        );
        assert_eq!(m.artifacts[1].os, "windows");
        // Wrong key fails at the client.
        let other = keys::generate().unwrap();
        assert!(Manifest::parse_verified(
            &std::fs::read(&mp).unwrap(),
            &std::fs::read_to_string(&sp).unwrap(),
            &other.public_hex
        )
        .is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_update_url_means_no_fragment_and_mixed_versions_are_refused() {
        let f = fixture();
        let ctx = f.ctx();
        let art = ctx.out_dir.join("a.AppImage");
        write_file(&art, b"x").unwrap();
        assert!(write_fragment(&ctx, &[(Format::AppImage, art)])
            .unwrap()
            .is_none());

        let dir = tmp("mixed");
        for (i, v) in ["1.0.0", "1.0.1"].iter().enumerate() {
            let frag = Fragment {
                app: "a.b".into(),
                version: (*v).into(),
                artifacts: vec![],
            };
            write_file(
                &dir.join(format!("{i}{FRAGMENT_SUFFIX}")),
                serde_json::to_string(&frag).unwrap().as_bytes(),
            )
            .unwrap();
        }
        let k = keys::generate().unwrap();
        assert!(merge_and_sign(&dir, "", &k.private_hex).is_err());
        assert!(merge_and_sign(&tmp("empty"), "", &k.private_hex).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn keygen_never_overwrites_and_key_can_be_read_back() {
        let dir = tmp("key");
        let p = dir.join("k.key");
        let public = keygen(&p).unwrap();
        assert!(keygen(&p).is_err());
        let private = key_from_env(&format!("@{}", p.display())).unwrap();
        assert_eq!(keys::public_of(&private).unwrap(), public);
        assert_eq!(key_from_env(&private).unwrap(), private);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
