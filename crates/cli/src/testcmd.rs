//! `alloy test` (also `cargo alloy test`): run the tests, and when a golden snapshot fails,
//! gather the expected/actual/diff images in one folder and say where they are.

use crate::args::Args;
use crate::error::{io, Error, Result};
use crate::project::cargo;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where failing snapshots are collected unless `--diff-dir` says otherwise.
pub const DEFAULT_DIFF_DIR: &str = "target/alloy-diff";

/// The `cargo test` command line for these arguments, without the program name.
pub fn cargo_args(a: &Args) -> Vec<String> {
    let mut v = vec!["test".to_string()];
    if let Some(m) = &a.selector.manifest_path {
        v.push("--manifest-path".into());
        v.push(m.display().to_string());
    }
    if let Some(p) = &a.selector.package {
        v.push("-p".into());
        v.push(p.clone());
    }
    v.extend(a.cargo_args.iter().cloned());
    v
}

/// The `*.compare.png` files in `dir`, sorted: one per failed snapshot.
pub fn failed_comparisons(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".compare.png"))
        })
        .collect();
    v.sort();
    v
}

/// Run the tests. `--update` rewrites snapshots instead of comparing. Exit status is cargo's.
pub fn run_test(a: &Args) -> Result<()> {
    let dir = a
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DIFF_DIR));
    let dir = if dir.is_absolute() {
        dir
    } else {
        std::env::current_dir()
            .map_err(io("current directory"))?
            .join(dir)
    };
    // Old failures must not be mistaken for this run's.
    let _ = std::fs::remove_dir_all(&dir);
    let args = cargo_args(a);
    let mut cmd = Command::new(cargo());
    cmd.args(&args).env("ALLOY_DIFF_DIR", &dir);
    if a.update {
        cmd.env("UPDATE_GOLDEN", "1");
        eprintln!("  updating snapshots (UPDATE_GOLDEN=1)");
    }
    let status = cmd.status().map_err(io("running cargo test"))?;
    let failed = failed_comparisons(&dir);
    if !failed.is_empty() {
        eprintln!(
            "\n{} snapshot(s) differ. Expected | actual | diff:",
            failed.len()
        );
        for f in &failed {
            eprintln!("  {}", f.display());
        }
        eprintln!("accept the new look with `alloy test --update`");
    }
    if status.success() {
        Ok(())
    } else {
        Err(Error::CommandFailed {
            cmd: format!("cargo {}", args.join(" ")),
            status: status.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse;

    fn p(s: &str) -> Args {
        parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn builds_the_cargo_command() {
        let a = p("test -p app --update --out d -- --lib counter");
        assert!(a.update);
        assert_eq!(cargo_args(&a), ["test", "-p", "app", "--lib", "counter"]);
        assert_eq!(cargo_args(&p("test")), ["test"]);
    }

    #[test]
    fn lists_only_compare_images_sorted() {
        let d = std::env::temp_dir().join(format!("alloy-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for n in [
            "b.compare.png",
            "a.compare.png",
            "a.diff.png",
            "a.actual.png",
        ] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        let got: Vec<_> = failed_comparisons(&d)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(got, ["a.compare.png", "b.compare.png"]);
        let _ = std::fs::remove_dir_all(&d);
        assert!(failed_comparisons(&d).is_empty());
    }
}
