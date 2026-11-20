//! Hand-rolled argument parsing (the CLI surface is tiny).

use crate::bundle::Format;
use crate::error::{Error, Result};
use crate::project::Selector;
use std::path::PathBuf;

/// Usage text.
pub const USAGE: &str = "\
alloy - build and bundle Alloy apps

USAGE:
    alloy <COMMAND> [OPTIONS] [-- <cargo build args>]

COMMANDS:
    build     Build the release binary
    bundle    Build, then package (default formats depend on the host OS)
    new       Create a starter app: alloy new <name>
    dev       Run the app (debug build) with its capability config
    check     Validate [package.metadata.alloy], icons and the capability config
    help      Show this text
    version   Show the version

OPTIONS:
    --manifest-path <PATH>   Cargo.toml of the app (default: current directory)
    -p, --package <NAME>     Package to use in a workspace
    --bin <NAME>             Binary target to bundle
    --target <TRIPLE>        Cross-compile for this target
    --format <LIST>          Comma list of: app, dmg, deb, rpm, appimage, exe, nsis, msi
    --out <DIR>              Output directory (default: <target>/alloy-bundle)
    --binary <PATH>          Bundle this prebuilt binary instead of building
    --identifier <ID>        (new) reverse-DNS app identifier
    --alloy-path <DIR>       (new) depend on a local Alloy checkout instead of git
    --release                (dev) run the release build
    --skip-build             Bundle the existing release binary without building

FORMATS:
    app        macOS .app bundle (ad-hoc signed when built on macOS)
    dmg        macOS disk image (macOS only, needs hdiutil)
    deb        Debian/Ubuntu package (built in pure Rust)
    rpm        Fedora/RHEL/openSUSE package (built in pure Rust)
    appimage   Linux AppImage (needs `appimagetool`, or set ALLOY_APPIMAGETOOL)
    exe        Windows portable executable (icon embedded)
    nsis       Windows installer (needs `makensis`)
    msi        Windows MSI installer (needs WiX 4+: `wix`, or ALLOY_WIX)
";

/// A parsed command.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// `alloy build`
    Build,
    /// `alloy bundle`
    Bundle,
    /// `alloy check`
    Check,
    /// `alloy new`
    New,
    /// `alloy dev`
    Dev,
    /// `alloy help`
    Help,
    /// `alloy version`
    Version,
}

/// Parsed command line.
#[derive(Debug)]
pub struct Args {
    /// The subcommand.
    pub command: Command,
    /// Package selection.
    pub selector: Selector,
    /// `--target`.
    pub target: Option<String>,
    /// `--format`, in order given.
    pub formats: Vec<Format>,
    /// `--out`.
    pub out: Option<PathBuf>,
    /// `--binary`.
    pub binary: Option<PathBuf>,
    /// `--skip-build`.
    pub skip_build: bool,
    /// `alloy new`: the app name.
    pub name: Option<String>,
    /// `--identifier`.
    pub identifier: Option<String>,
    /// `--alloy-path`.
    pub alloy_path: Option<PathBuf>,
    /// `--release` (dev).
    pub release: bool,
    /// Arguments after `--`: forwarded to cargo (`build`, `bundle`) or to the
    /// app itself (`dev`).
    pub cargo_args: Vec<String>,
}

/// Parse `args` (without the program name).
pub fn parse(args: &[String]) -> Result<Args> {
    let mut it = args.iter();
    let command = match it.next().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => Command::Help,
        Some("version" | "--version" | "-V") => Command::Version,
        Some("build") => Command::Build,
        Some("bundle") => Command::Bundle,
        Some("check") => Command::Check,
        Some("new") => Command::New,
        Some("dev") => Command::Dev,
        Some(other) => {
            return Err(Error::Usage(format!(
                "unknown command `{other}`; try `alloy help`"
            )))
        }
    };
    let mut a = Args {
        command,
        selector: Selector::default(),
        target: None,
        formats: vec![],
        out: None,
        binary: None,
        skip_build: false,
        name: None,
        identifier: None,
        alloy_path: None,
        release: false,
        cargo_args: vec![],
    };
    let value = |it: &mut std::slice::Iter<String>, flag: &str| -> Result<String> {
        it.next()
            .cloned()
            .ok_or_else(|| Error::Usage(format!("`{flag}` needs a value")))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--manifest-path" => a.selector.manifest_path = Some(value(&mut it, arg)?.into()),
            "-p" | "--package" => a.selector.package = Some(value(&mut it, arg)?),
            "--bin" => a.selector.bin = Some(value(&mut it, arg)?),
            "--target" => a.target = Some(value(&mut it, arg)?),
            "--out" => a.out = Some(value(&mut it, arg)?.into()),
            "--binary" => a.binary = Some(value(&mut it, arg)?.into()),
            "--skip-build" => a.skip_build = true,
            "--release" => a.release = true,
            "--identifier" => a.identifier = Some(value(&mut it, arg)?),
            "--alloy-path" => a.alloy_path = Some(value(&mut it, arg)?.into()),
            "--format" => {
                for f in value(&mut it, arg)?.split(',') {
                    let f = Format::parse(f.trim())?;
                    if !a.formats.contains(&f) {
                        a.formats.push(f);
                    }
                }
            }
            "--" => {
                a.cargo_args = it.cloned().collect();
                break;
            }
            other if a.command == Command::New && a.name.is_none() && !other.starts_with('-') => {
                a.name = Some(other.to_string())
            }
            other => return Err(Error::Usage(format!("unknown option `{other}`"))),
        }
    }
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Args> {
        parse(&s.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    #[test]
    fn commands() {
        assert_eq!(p("").unwrap().command, Command::Help);
        assert_eq!(p("bundle").unwrap().command, Command::Bundle);
        assert_eq!(p("--version").unwrap().command, Command::Version);
        assert!(p("frobnicate").is_err());
    }

    #[test]
    fn options() {
        let a = p("bundle -p app --bin x --target t --format deb,app,deb --out o --skip-build -- --features f")
            .unwrap();
        assert_eq!(a.selector.package.as_deref(), Some("app"));
        assert_eq!(a.selector.bin.as_deref(), Some("x"));
        assert_eq!(a.target.as_deref(), Some("t"));
        assert_eq!(a.formats, vec![Format::Deb, Format::App]);
        assert!(a.skip_build);
        assert_eq!(a.cargo_args, vec!["--features", "f"]);
    }

    #[test]
    fn new_and_dev() {
        let a = p("new my-app --identifier dev.x.y --alloy-path /a").unwrap();
        assert_eq!(a.command, Command::New);
        assert_eq!(a.name.as_deref(), Some("my-app"));
        assert_eq!(a.identifier.as_deref(), Some("dev.x.y"));
        let d = p("dev --release -- --flag").unwrap();
        assert!(d.release);
        assert_eq!(d.cargo_args, vec!["--flag"]);
        assert!(p("bundle stray").is_err());
        assert!(p("new a b").is_err());
    }

    #[test]
    fn errors() {
        assert!(p("bundle --target").is_err());
        assert!(p("bundle --wat").is_err());
        assert!(p("bundle --format zip").is_err());
    }
}
