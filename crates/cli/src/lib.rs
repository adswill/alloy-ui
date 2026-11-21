//! `alloy` build and bundle tool.
//!
//! Builds an Alloy app in release mode and packages it as a macOS `.app`
//! (and `.dmg`), a Linux `.deb` and AppImage, or a Windows `.exe` and NSIS
//! installer. App metadata comes from `[package.metadata.alloy]` in the app's
//! Cargo.toml (see [`project`]); the capability config referenced there is
//! validated with `alloy-ui-ipc` and shipped inside every bundle as
//! `alloy.capabilities.toml`.

pub mod args;
pub mod bundle;
pub mod compile;
pub mod dev;
pub mod error;
pub mod icon;
pub mod mobile;
pub mod pe;
pub mod project;
pub mod scaffold;

pub use error::{Error, Result};

use args::Command;
use bundle::{Ctx, Format};
use icon::IconSet;
use project::{describe_capabilities, Project};

/// Run the CLI with `argv` (without the program name).
pub fn run(argv: &[String]) -> Result<()> {
    let a = args::parse(argv)?;
    match a.command {
        Command::Help => {
            print!("{}", args::usage());
            Ok(())
        }
        Command::Version => {
            println!("alloy {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::New => {
            let name = a
                .name
                .ok_or_else(|| Error::Usage("usage: alloy new <name>".into()))?;
            let source = match a.alloy_path {
                Some(p) => scaffold::Source::Path(
                    p.canonicalize()
                        .map_err(error::io(format!("{}", p.display())))?,
                ),
                None => scaffold::Source::Git,
            };
            let cwd = std::env::current_dir().map_err(error::io("current directory"))?;
            let dir = scaffold::create(
                &cwd,
                &scaffold::NewApp {
                    name,
                    identifier: a.identifier,
                    source,
                    template: a.template,
                },
            )?;
            println!("created {}", dir.display());
            println!("next: cd {} && alloy dev", dir.display());
            Ok(())
        }
        Command::Dev => dev::run_dev(&a),
        Command::Check => {
            let p = Project::load(&a.selector)?;
            let icons = IconSet::load(&p.icons)?;
            let caps = p.load_capabilities()?;
            println!(
                "app:          {} {} ({})",
                p.product_name, p.version, p.identifier
            );
            println!("binary:       {}", p.bin_name);
            println!("category:     {}", p.category);
            println!(
                "icons:        {} png, icns: {}, ico: {}",
                icons.pngs.len(),
                icons.icns_bytes().is_some(),
                icons.ico_bytes().is_some()
            );
            match caps {
                Some((set, _)) => println!("capabilities: {}", describe_capabilities(&set)),
                None => println!("capabilities: none configured (app is fully sandboxed)"),
            }
            if icons.is_empty() {
                eprintln!("warning: no icons configured; bundles will use the platform default");
            }
            Ok(())
        }
        Command::Build | Command::Bundle => {
            let p = Project::load(&a.selector)?;
            let host = compile::host_triple()?;
            let triple = a
                .target
                .as_deref()
                .map(mobile::expand_target)
                .unwrap_or(host);
            if let Some(os) = mobile::os_of(&triple) {
                // Phones and tablets: build the library, then wrap it in a native project.
                if a.command == Command::Build {
                    let lib = mobile::build_lib(&p, os, &triple, &a.cargo_args)?;
                    println!("{}", lib.display());
                    return Ok(());
                }
                let prebuilt = match (&a.binary, a.skip_build) {
                    (Some(b), _) => Some(b.clone()),
                    (None, true) => Some(mobile::lib_path(&p, os, &triple)?),
                    (None, false) => None,
                };
                let icons = IconSet::load_or_default(&p.icons)?;
                let out_dir = a
                    .out
                    .clone()
                    .unwrap_or_else(|| p.target_dir.join("alloy-bundle").join(&triple));
                let dir = mobile::bundle(&p, &icons, &triple, &out_dir, prebuilt, &a.cargo_args)?;
                println!("{}", dir.display());
                return Ok(());
            }
            let explicit = a.target.is_some();
            let binary = if let Some(b) = &a.binary {
                b.clone()
            } else if a.skip_build {
                compile::binary_path(&p, &triple, explicit)
            } else {
                compile::build_release(&p, &triple, explicit, &a.cargo_args)?
            };
            if a.command == Command::Build {
                println!("{}", binary.display());
                return Ok(());
            }
            let formats = if a.formats.is_empty() {
                Format::defaults_for(&triple)
            } else {
                a.formats.clone()
            };
            let icons = IconSet::load_or_default(&p.icons)?;
            let caps = p.load_capabilities()?;
            if let Some((set, _)) = &caps {
                eprintln!("  capabilities: {}", describe_capabilities(set));
            }
            let out_dir = a
                .out
                .clone()
                .unwrap_or_else(|| p.target_dir.join("alloy-bundle").join(&triple));
            std::fs::create_dir_all(&out_dir)
                .map_err(error::io(format!("creating {}", out_dir.display())))?;
            let ctx = Ctx::new(&p, binary, triple, out_dir, icons, caps.map(|c| c.1))?;
            for f in formats {
                let artifact = bundle::bundle(&ctx, f)?;
                println!("{}", artifact.display());
            }
            Ok(())
        }
    }
}
