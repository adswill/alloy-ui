//! `alloy dev`: build and run the app with its capability config.

use crate::args::Args;
use crate::error::{io, Error, Result};
use crate::project::{cargo, describe_capabilities, Project};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};

/// Environment variable pointing a running app at its capability config
/// (bundles ship the file instead; see `alloy bundle`).
pub const CAPABILITIES_ENV: &str = "ALLOY_CAPABILITIES";

/// Validate the capability config, then build and run the app, forwarding the
/// arguments after `--` to it. Unless `--once`, watch the sources: every save
/// rebuilds and restarts the app (apps using `.persist()` keep their state),
/// and a failed build prints cargo's errors and waits for the next save.
pub fn run_dev(a: &Args) -> Result<()> {
    let p = Project::load(&a.selector)?;
    let caps = p.load_capabilities()?;
    if let Some((set, _)) = &caps {
        eprintln!("  capabilities: {}", describe_capabilities(set));
    }
    if a.once {
        let exe = build(&p, a)?.ok_or_else(|| Error::CommandFailed {
            cmd: "cargo build".into(),
            status: "build failed".into(),
        })?;
        let status = launch(&p, a, &exe)?.wait().map_err(io("running the app"))?;
        return if status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed {
                cmd: "app".into(),
                status: status.to_string(),
            })
        };
    }
    let root = p
        .manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let mut watched = vec![
        root.join("src"),
        root.join("assets"),
        p.manifest_path.clone(),
    ];
    watched.extend(p.capabilities_path.clone());
    eprintln!("  watching {} (ctrl-c to stop)", root.display());
    let mut seen = newest(&watched);
    let mut child: Option<Child> = None;
    loop {
        eprintln!("  building...");
        match build(&p, a)? {
            Some(exe) => {
                if let Some(mut c) = child.take() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
                child = Some(launch(&p, a, &exe)?);
            }
            None => eprintln!("  build failed; fix the error and save to retry"),
        }
        seen = seen.max(newest(&watched));
        loop {
            std::thread::sleep(Duration::from_millis(250));
            if let Some(c) = child.as_mut() {
                if let Ok(Some(status)) = c.try_wait() {
                    // The window was closed (or the app crashed): stop watching.
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(Error::CommandFailed {
                            cmd: "app".into(),
                            status: status.to_string(),
                        })
                    };
                }
            }
            let now = newest(&watched);
            if now > seen {
                // Let a burst of saves (formatters, git checkouts) settle.
                std::thread::sleep(Duration::from_millis(150));
                seen = newest(&watched);
                eprintln!("  change detected, restarting");
                break;
            }
        }
    }
}

/// `cargo build`, returning the binary on success and `None` on a compile error.
fn build(p: &Project, a: &Args) -> Result<Option<PathBuf>> {
    let mut cmd = Command::new(cargo());
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(&p.manifest_path)
        .args(["-p", &p.package_name, "--bin", &p.bin_name])
        .args(["--message-format", "json-render-diagnostics"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if a.release {
        cmd.arg("--release");
    }
    if let Some(t) = &a.target {
        cmd.args(["--target", t]);
    }
    let out = cmd.output().map_err(io("running cargo build"))?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(executable(&String::from_utf8_lossy(&out.stdout)))
}

/// The last `"executable"` path in cargo's JSON message stream.
fn executable(stream: &str) -> Option<PathBuf> {
    stream
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("executable")?.as_str().map(PathBuf::from))
        .next_back()
}

fn launch(p: &Project, a: &Args, exe: &Path) -> Result<Child> {
    let mut cmd = Command::new(exe);
    cmd.env("ALLOY_DEV", "1").env("ALLOY_INSPECT", "1");
    if let Some(path) = &p.capabilities_path {
        cmd.env(CAPABILITIES_ENV, path);
    }
    cmd.args(&a.cargo_args);
    cmd.spawn()
        .map_err(io(format!("starting {}", exe.display())))
}

/// The latest modification time under `paths` (files or directories, recursive).
fn newest(paths: &[PathBuf]) -> SystemTime {
    fn walk(p: &Path, best: &mut SystemTime) {
        let Ok(meta) = std::fs::metadata(p) else {
            return;
        };
        if let Ok(t) = meta.modified() {
            *best = (*best).max(t);
        }
        if meta.is_dir() {
            if let Ok(rd) = std::fs::read_dir(p) {
                for e in rd.flatten() {
                    walk(&e.path(), best);
                }
            }
        }
    }
    let mut best = SystemTime::UNIX_EPOCH;
    for p in paths {
        walk(p, &mut best);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_built_binary() {
        let stream = "{\"reason\":\"compiler-artifact\",\"executable\":null}\n\
                      {\"reason\":\"compiler-artifact\",\"executable\":\"/t/debug/app\"}\nnot json\n";
        assert_eq!(executable(stream), Some(PathBuf::from("/t/debug/app")));
        assert_eq!(executable("{}"), None);
    }

    #[test]
    fn newest_sees_new_files() {
        let d = std::env::temp_dir().join(format!("alloy-watch-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let before = newest(std::slice::from_ref(&d));
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(d.join("a.rs"), "x").unwrap();
        assert!(newest(std::slice::from_ref(&d)) > before);
        let _ = std::fs::remove_dir_all(&d);
    }
}
