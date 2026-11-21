//! Deep links: telling launch arguments apart and registering a URL scheme.
//!
//! A deep link is a URL like `myapp://invite/42` that the OS hands to the app
//! registered for its scheme. It arrives as a command-line argument on Linux
//! and Windows and as an Apple Event on macOS; either way the shell reports it
//! as [`crate::PlatformEvent::Open`], and the facade splits it with
//! [`classify`].

use crate::PlatformError;
use std::path::PathBuf;

/// What the arguments of a launch (or of a second launch forwarded to this
/// instance) ask to open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Opened {
    /// `scheme://...` links, other than `file://`.
    pub urls: Vec<String>,
    /// Files: `file://` URIs and plain paths that exist.
    pub files: Vec<PathBuf>,
}

fn is_scheme(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic())
        && c.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

impl Opened {
    /// Nothing to open.
    pub fn is_empty(&self) -> bool {
        self.urls.is_empty() && self.files.is_empty()
    }
}

/// Sort launch arguments into links and files. Flags (`-x`, `--y`) and
/// arguments that are neither a URL nor an existing path are ignored.
pub fn classify(args: &[String]) -> Opened {
    let mut out = Opened::default();
    for a in args {
        if a.starts_with('-') {
            continue;
        }
        if let Some(rest) = a.strip_prefix("file://") {
            let rest = rest.strip_prefix("localhost").unwrap_or(rest);
            out.files
                .push(PathBuf::from(crate::clipboard::percent_decode(rest)));
        } else if let Some((scheme, _)) = a.split_once("://") {
            if is_scheme(scheme) {
                out.urls.push(a.clone());
            }
        } else if crate::clipboard::is_existing_path(a) {
            out.files.push(PathBuf::from(a));
        }
    }
    out
}

/// The scheme of `url` (`myapp` for `myapp://x`), lowercased.
pub fn scheme_of(url: &str) -> Option<String> {
    let (s, _) = url.split_once("://")?;
    is_scheme(s).then(|| s.to_ascii_lowercase())
}

/// The text of a freedesktop `.desktop` file that makes `exe` the handler of
/// `schemes` (`x-scheme-handler/<scheme>`).
pub fn desktop_entry(name: &str, exe: &str, schemes: &[&str]) -> String {
    let types: String = schemes
        .iter()
        .map(|s| format!("x-scheme-handler/{s};"))
        .collect();
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec=\"{exe}\" %u\nNoDisplay=true\nTerminal=false\nMimeType={types}\n"
    )
}

/// Make this executable the handler of `scheme` for the current user, so that
/// opening `scheme://...` anywhere launches (or, with
/// [`crate::instance::single_instance`], wakes) it.
///
/// Linux: writes `~/.local/share/applications/<app_id>.desktop` and runs
/// `xdg-mime default`. Windows: writes `HKCU\Software\Classes\<scheme>`.
/// macOS cannot register at run time: the scheme must be in the bundle's
/// `Info.plist`, which `alloy bundle` writes from `url-schemes` in
/// `[package.metadata.alloy]`.
pub fn register_scheme(app_id: &str, scheme: &str) -> Result<(), PlatformError> {
    if !is_scheme(scheme) || app_id.is_empty() || app_id.contains(['/', '\\']) {
        return Err(PlatformError(format!(
            "register_scheme: bad scheme `{scheme}` or app id `{app_id}`"
        )));
    }
    let exe = std::env::current_exe()
        .map_err(|e| PlatformError(format!("register_scheme: {e}")))?
        .to_string_lossy()
        .into_owned();
    crate::os::register_scheme(app_id, scheme, &exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classifies_arguments() {
        let o = classify(&s(&[
            "--headless",
            "myapp://invite/42?x=1",
            "file:///tmp/a%20b.txt",
            "/definitely/not/here",
            "Cargo.toml",
            "ht!tp://bad",
            "https://example.com",
        ]));
        assert_eq!(
            o.urls,
            ["myapp://invite/42?x=1", "https://example.com"].map(String::from)
        );
        assert_eq!(o.files[0], PathBuf::from("/tmp/a b.txt"));
        // `Cargo.toml` only counts when it exists relative to the working directory,
        // which is the crate root under `cargo test`.
        assert_eq!(o.files.len(), 2);
        assert_eq!(o.files[1], PathBuf::from("Cargo.toml"));
    }

    #[test]
    fn schemes() {
        assert_eq!(scheme_of("MyApp://x").as_deref(), Some("myapp"));
        assert_eq!(scheme_of("no scheme"), None);
        assert_eq!(scheme_of("1bad://x"), None);
    }

    #[test]
    fn desktop_file() {
        let d = desktop_entry("Demo", "/opt/demo/demo", &["demo", "demo-dev"]);
        assert!(d.contains("Exec=\"/opt/demo/demo\" %u\n"));
        assert!(d.contains("MimeType=x-scheme-handler/demo;x-scheme-handler/demo-dev;\n"));
    }
}
