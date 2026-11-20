//! Release build: drives `cargo build --release` and locates the binary.

use crate::error::{io, Error, Result};
use crate::project::{cargo, Project};
use std::path::PathBuf;
use std::process::Command;

/// The host target triple, from `rustc -vV`.
pub fn host_triple() -> Result<String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = Command::new(rustc)
        .arg("-vV")
        .output()
        .map_err(io("running rustc -vV"))?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(str::to_string)
        .ok_or_else(|| Error::Metadata("rustc -vV printed no host triple".into()))
}

/// Where cargo puts the release binary for `triple`.
/// `explicit_target` is true when `--target` was passed (cargo then nests
/// output under the triple).
pub fn binary_path(p: &Project, triple: &str, explicit_target: bool) -> PathBuf {
    let mut d = p.target_dir.clone();
    if explicit_target {
        d.push(triple);
    }
    d.push("release");
    d.push(if triple.contains("windows") {
        format!("{}.exe", p.bin_name)
    } else {
        p.bin_name.clone()
    });
    d
}

/// Run `cargo build --release` for the project and return the binary path.
pub fn build_release(
    p: &Project,
    triple: &str,
    explicit_target: bool,
    extra: &[String],
) -> Result<PathBuf> {
    let mut cmd = Command::new(cargo());
    cmd.arg("build")
        .arg("--release")
        .arg("--manifest-path")
        .arg(&p.manifest_path)
        .args(["-p", &p.package_name, "--bin", &p.bin_name]);
    if explicit_target {
        cmd.args(["--target", triple]);
    }
    cmd.args(extra);
    eprintln!("   building {} (release, {triple})", p.bin_name);
    let status = cmd.status().map_err(io("running cargo build"))?;
    if !status.success() {
        return Err(Error::CommandFailed {
            cmd: "cargo build --release".into(),
            status: status.to_string(),
        });
    }
    let bin = binary_path(p, triple, explicit_target);
    if !bin.is_file() {
        return Err(Error::Config(format!(
            "build succeeded but {} does not exist (custom build.target-dir or CARGO_TARGET_DIR layout?)",
            bin.display()
        )));
    }
    Ok(bin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Selector;
    use serde_json::json;

    fn proj() -> Project {
        let m = json!({"target_directory": "/t", "packages": [{
            "name": "c", "version": "0.1.0", "manifest_path": "/a/Cargo.toml",
            "targets": [{"name": "c", "kind": ["bin"]}],
            "metadata": {"alloy": {"identifier": "dev.x.c"}}}]});
        Project::from_metadata(
            &m,
            &Selector {
                package: Some("c".into()),
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn binary_paths() {
        let p = proj();
        assert_eq!(
            binary_path(&p, "x86_64-unknown-linux-gnu", false),
            PathBuf::from("/t/release/c")
        );
        assert_eq!(
            binary_path(&p, "x86_64-pc-windows-msvc", true),
            PathBuf::from("/t/x86_64-pc-windows-msvc/release/c.exe")
        );
    }
}
