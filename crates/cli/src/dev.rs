//! `alloy dev`: build and run the app with its capability config.

use crate::args::Args;
use crate::error::{io, Error, Result};
use crate::project::{cargo, describe_capabilities, Project};
use std::process::Command;

/// Environment variable pointing a running app at its capability config
/// (bundles ship the file instead; see `alloy bundle`).
pub const CAPABILITIES_ENV: &str = "ALLOY_CAPABILITIES";

/// Validate the capability config, then `cargo run` the app, forwarding the
/// arguments after `--` to it. The app's exit status becomes ours.
pub fn run_dev(a: &Args) -> Result<()> {
    let p = Project::load(&a.selector)?;
    let caps = p.load_capabilities()?;
    if let Some((set, _)) = &caps {
        eprintln!("  capabilities: {}", describe_capabilities(set));
    }
    let mut cmd = Command::new(cargo());
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(&p.manifest_path)
        .args(["-p", &p.package_name, "--bin", &p.bin_name]);
    if a.release {
        cmd.arg("--release");
    }
    if let Some(t) = &a.target {
        cmd.args(["--target", t]);
    }
    if let Some(path) = &p.capabilities_path {
        cmd.env(CAPABILITIES_ENV, path);
    }
    cmd.arg("--").args(&a.cargo_args);
    let status = cmd.status().map_err(io("running cargo run"))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::CommandFailed {
            cmd: "cargo run".into(),
            status: status.to_string(),
        })
    }
}
