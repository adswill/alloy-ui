//! Code signing and notarisation.
//!
//! Credentials come from environment variables only, never from
//! `Cargo.toml`, so a repository can be public and CI holds the secrets.
//! The pieces that build command lines are pure functions, unit-tested on
//! every OS; running `codesign`, `notarytool`, `signtool` and friends needs the
//! real platform and certificates and is exercised only on those machines.
//!
//! | Variable | Meaning |
//! |---|---|
//! | `ALLOY_SIGN_IDENTITY` | macOS: `Developer ID Application: Name (TEAMID)`, or `-` for ad hoc |
//! | `ALLOY_NOTARY_PROFILE` | macOS: keychain profile from `xcrun notarytool store-credentials` |
//! | `ALLOY_NOTARY_APPLE_ID`, `_TEAM_ID`, `_PASSWORD` | macOS: Apple ID, team, app-specific password |
//! | `ALLOY_NOTARY_KEY`, `_KEY_ID`, `_ISSUER` | macOS: App Store Connect API key (`.p8`) |
//! | `ALLOY_WIN_CERT`, `ALLOY_WIN_CERT_PASSWORD` | Windows: `.pfx` file and its password |
//! | `ALLOY_WIN_CERT_SHA1` | Windows: thumbprint of a certificate in the store |
//! | `ALLOY_WIN_TIMESTAMP_URL` | RFC 3161 server (default DigiCert) |
//! | `ALLOY_WIN_SIGN_COMMAND` | Windows: any command with `{file}` (Azure Trusted Signing, HSM, ...) |

use super::{run, which, Ctx};
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Look up a setting; the real environment in production, a map in tests.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// The process environment, ignoring empty values.
pub fn process_env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

// ---- macOS ----------------------------------------------------------------

/// How to authenticate to Apple's notary service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notary {
    /// A keychain profile created with `notarytool store-credentials`.
    Profile(String),
    /// Apple ID with an app-specific password.
    AppleId {
        /// Apple ID e-mail.
        apple_id: String,
        /// Developer team ID.
        team_id: String,
        /// App-specific password.
        password: String,
    },
    /// App Store Connect API key.
    ApiKey {
        /// Path of the `.p8` file.
        key: String,
        /// Key ID.
        key_id: String,
        /// Issuer UUID.
        issuer: String,
    },
}

/// macOS signing settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacSign {
    /// Signing identity; `-` means ad hoc.
    pub identity: String,
    /// Entitlements plist, if any.
    pub entitlements: Option<PathBuf>,
    /// Notarisation credentials, if any.
    pub notary: Option<Notary>,
}

impl MacSign {
    /// Read the settings. Always returns something: with no
    /// `ALLOY_SIGN_IDENTITY` the result is an ad hoc signature, which is what
    /// Apple Silicon needs to run a downloaded binary at all.
    pub fn from_env(env: Env<'_>, entitlements: Option<PathBuf>) -> Result<Self> {
        let identity = env("ALLOY_SIGN_IDENTITY").unwrap_or_else(|| "-".into());
        let notary = if let Some(p) = env("ALLOY_NOTARY_PROFILE") {
            Some(Notary::Profile(p))
        } else if let Some(apple_id) = env("ALLOY_NOTARY_APPLE_ID") {
            let need = |k: &str| {
                env(k).ok_or_else(|| {
                    Error::Config(format!("ALLOY_NOTARY_APPLE_ID is set but {k} is not"))
                })
            };
            Some(Notary::AppleId {
                apple_id,
                team_id: need("ALLOY_NOTARY_TEAM_ID")?,
                password: need("ALLOY_NOTARY_PASSWORD")?,
            })
        } else if let Some(key) = env("ALLOY_NOTARY_KEY") {
            let need = |k: &str| {
                env(k)
                    .ok_or_else(|| Error::Config(format!("ALLOY_NOTARY_KEY is set but {k} is not")))
            };
            Some(Notary::ApiKey {
                key,
                key_id: need("ALLOY_NOTARY_KEY_ID")?,
                issuer: need("ALLOY_NOTARY_ISSUER")?,
            })
        } else {
            None
        };
        if notary.is_some() && identity == "-" {
            return Err(Error::Config(
                "notarisation needs a real ALLOY_SIGN_IDENTITY (Developer ID Application); an ad hoc signature cannot be notarised".into(),
            ));
        }
        Ok(Self {
            identity,
            entitlements,
            notary,
        })
    }

    /// True for a Developer ID signature (hardened runtime and timestamp apply).
    pub fn is_developer_id(&self) -> bool {
        self.identity != "-"
    }

    /// `codesign` arguments for `target` (a `.app` or `.dmg`).
    pub fn codesign_args(&self, target: &Path) -> Vec<String> {
        let mut a: Vec<String> = ["--force", "--sign"].map(String::from).into();
        a.push(self.identity.clone());
        if self.is_developer_id() {
            // Notarisation requires the hardened runtime and a secure timestamp.
            a.extend(["--options", "runtime", "--timestamp"].map(String::from));
            if let Some(e) = &self.entitlements {
                a.push("--entitlements".into());
                a.push(e.to_string_lossy().into_owned());
            }
        }
        if target.extension().is_some_and(|e| e == "app") {
            a.push("--deep".into());
        }
        a.push(target.to_string_lossy().into_owned());
        a
    }

    /// `xcrun notarytool submit ... --wait` arguments, if notarising.
    pub fn notarize_args(&self, archive: &Path) -> Option<Vec<String>> {
        let mut a: Vec<String> = ["notarytool", "submit"].map(String::from).into();
        a.push(archive.to_string_lossy().into_owned());
        match self.notary.as_ref()? {
            Notary::Profile(p) => a.extend(["--keychain-profile".into(), p.clone()]),
            Notary::AppleId {
                apple_id,
                team_id,
                password,
            } => a.extend([
                "--apple-id".into(),
                apple_id.clone(),
                "--team-id".into(),
                team_id.clone(),
                "--password".into(),
                password.clone(),
            ]),
            Notary::ApiKey {
                key,
                key_id,
                issuer,
            } => a.extend([
                "--key".into(),
                key.clone(),
                "--key-id".into(),
                key_id.clone(),
                "--issuer".into(),
                issuer.clone(),
            ]),
        }
        a.extend(["--wait".into(), "--timeout".into(), "30m".into()]);
        Some(a)
    }

    /// `xcrun stapler staple <target>` arguments.
    pub fn staple_args(target: &Path) -> Vec<String> {
        vec![
            "stapler".into(),
            "staple".into(),
            target.to_string_lossy().into_owned(),
        ]
    }
}

/// Sign `target`; when notary credentials exist also notarise and staple a
/// `.app` (zipped with `ditto`, which keeps the code signature intact).
pub fn mac_sign_and_notarize(ctx: &Ctx<'_>, target: &Path) -> Result<()> {
    let cfg = MacSign::from_env(&process_env, ctx.project.mac_entitlements.clone())?;
    if !cfg!(target_os = "macos") {
        eprintln!(
            "note: {} not code-signed (not building on macOS)",
            target.display()
        );
        return Ok(());
    }
    if which("codesign").is_none() {
        eprintln!("note: codesign not found; left unsigned");
        return Ok(());
    }
    if !cfg.is_developer_id() {
        eprintln!(
            "note: ad hoc signature (set ALLOY_SIGN_IDENTITY to a Developer ID to distribute)"
        );
    }
    run(Command::new("codesign").args(cfg.codesign_args(target)))?;
    if cfg.notary.is_some() && target.extension().is_some_and(|e| e == "app") {
        let zip = target.with_extension("notarize.zip");
        run(Command::new("ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(target)
            .arg(&zip))?;
        let r = run(Command::new("xcrun").args(cfg.notarize_args(&zip).unwrap_or_default()));
        std::fs::remove_file(&zip).ok();
        r?;
        run(Command::new("xcrun").args(MacSign::staple_args(target)))?;
    }
    Ok(())
}

// ---- Windows --------------------------------------------------------------

/// Which tool signs Windows files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinTool {
    /// Microsoft `signtool.exe`.
    SignTool(PathBuf),
    /// `osslsigncode` (Linux/macOS cross-signing; also handles MSIX).
    OsslSignCode(PathBuf),
}

/// Windows signing settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinSign {
    /// A user command; `{file}` is replaced by the path.
    Command(String),
    /// A certificate handled by `tool`.
    Cert {
        /// The signing program.
        tool: WinTool,
        /// `.pfx` path, or `None` when `sha1` selects a store certificate.
        pfx: Option<String>,
        /// PFX password.
        password: Option<String>,
        /// Certificate thumbprint in the Windows store (signtool only).
        sha1: Option<String>,
        /// RFC 3161 timestamp server.
        timestamp: String,
    },
}

impl WinSign {
    /// Read the settings; `Ok(None)` when no signing is configured.
    /// `find_tool` locates a program (injected so tests need no real tools).
    pub fn from_env(
        env: Env<'_>,
        find_tool: &dyn Fn(&str) -> Option<PathBuf>,
    ) -> Result<Option<Self>> {
        if let Some(c) = env("ALLOY_WIN_SIGN_COMMAND") {
            if !c.contains("{file}") {
                return Err(Error::Config(
                    "ALLOY_WIN_SIGN_COMMAND must contain {file}".into(),
                ));
            }
            return Ok(Some(Self::Command(c)));
        }
        let pfx = env("ALLOY_WIN_CERT");
        let sha1 = env("ALLOY_WIN_CERT_SHA1");
        if pfx.is_none() && sha1.is_none() {
            return Ok(None);
        }
        let tool = find_tool("signtool")
            .map(WinTool::SignTool)
            .or_else(|| find_tool("osslsigncode").map(WinTool::OsslSignCode))
            .ok_or(Error::ToolMissing {
                tool: "signtool",
                hint: "install the Windows SDK (signtool) or osslsigncode, or use ALLOY_WIN_SIGN_COMMAND",
            })?;
        if sha1.is_some() && pfx.is_none() && matches!(tool, WinTool::OsslSignCode(_)) {
            return Err(Error::Config(
                "ALLOY_WIN_CERT_SHA1 needs signtool; osslsigncode needs ALLOY_WIN_CERT (.pfx)"
                    .into(),
            ));
        }
        Ok(Some(Self::Cert {
            tool,
            pfx,
            password: env("ALLOY_WIN_CERT_PASSWORD"),
            sha1,
            timestamp: env("ALLOY_WIN_TIMESTAMP_URL")
                .unwrap_or_else(|| "http://timestamp.digicert.com".into()),
        }))
    }

    /// The program and arguments that sign `file`. For `osslsigncode` the
    /// signed copy is written to `signed_out`, which the caller moves over
    /// `file`.
    pub fn command(&self, file: &Path, signed_out: &Path) -> (String, Vec<String>) {
        let f = file.to_string_lossy().into_owned();
        match self {
            Self::Command(c) => {
                let line = c.replace("{file}", &f);
                if cfg!(windows) {
                    ("cmd".into(), vec!["/C".into(), line])
                } else {
                    ("sh".into(), vec!["-c".into(), line])
                }
            }
            Self::Cert {
                tool: WinTool::SignTool(p),
                pfx,
                password,
                sha1,
                timestamp,
            } => {
                let mut a: Vec<String> = ["sign", "/fd", "SHA256", "/tr"].map(String::from).into();
                a.push(timestamp.clone());
                a.extend(["/td".into(), "SHA256".into()]);
                if let Some(pfx) = pfx {
                    a.extend(["/f".into(), pfx.clone()]);
                    if let Some(pw) = password {
                        a.extend(["/p".into(), pw.clone()]);
                    }
                } else if let Some(h) = sha1 {
                    a.extend(["/sha1".into(), h.clone()]);
                }
                a.push(f);
                (p.to_string_lossy().into_owned(), a)
            }
            Self::Cert {
                tool: WinTool::OsslSignCode(p),
                pfx,
                password,
                timestamp,
                ..
            } => {
                let mut a: Vec<String> = ["sign", "-h", "sha256", "-ts"].map(String::from).into();
                a.push(timestamp.clone());
                if let Some(pfx) = pfx {
                    a.extend(["-pkcs12".into(), pfx.clone()]);
                }
                if let Some(pw) = password {
                    a.extend(["-pass".into(), pw.clone()]);
                }
                a.extend([
                    "-in".into(),
                    f,
                    "-out".into(),
                    signed_out.to_string_lossy().into_owned(),
                ]);
                (p.to_string_lossy().into_owned(), a)
            }
        }
    }
}

/// Sign `file` if Windows signing is configured; otherwise do nothing.
pub fn win_sign_file(file: &Path) -> Result<()> {
    let Some(cfg) = WinSign::from_env(&process_env, &which)? else {
        return Ok(());
    };
    let signed = file.with_extension("signed.tmp");
    let (prog, args) = cfg.command(file, &signed);
    run(Command::new(prog).args(args))?;
    if signed.exists() {
        std::fs::rename(&signed, file).map_err(crate::error::io("replacing the signed file"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    fn tools(_: &str) -> Option<PathBuf> {
        None
    }

    #[test]
    fn mac_ad_hoc_by_default() {
        let m = MacSign::from_env(&env(&[]), None).unwrap();
        assert!(!m.is_developer_id() && m.notary.is_none());
        let a = m.codesign_args(Path::new("/o/Demo.app"));
        assert_eq!(a, ["--force", "--sign", "-", "--deep", "/o/Demo.app"]);
    }

    #[test]
    fn mac_developer_id_uses_hardened_runtime_and_entitlements() {
        let m = MacSign::from_env(
            &env(&[(
                "ALLOY_SIGN_IDENTITY",
                "Developer ID Application: Ann (ABCDE12345)",
            )]),
            Some("/p/mac.entitlements".into()),
        )
        .unwrap();
        let a = m.codesign_args(Path::new("/o/Demo.app"));
        let joined = a.join(" ");
        for needle in [
            "--options runtime",
            "--timestamp",
            "--entitlements /p/mac.entitlements",
            "--deep",
        ] {
            assert!(joined.contains(needle), "{needle} in {joined}");
        }
        assert_eq!(a.last().unwrap(), "/o/Demo.app");
        // A dmg is signed without --deep.
        assert!(!m
            .codesign_args(Path::new("/o/Demo.dmg"))
            .contains(&"--deep".to_string()));
    }

    #[test]
    fn mac_notary_modes_and_validation() {
        let id = ("ALLOY_SIGN_IDENTITY", "Developer ID Application: A (T)");
        let p = MacSign::from_env(&env(&[id, ("ALLOY_NOTARY_PROFILE", "alloy")]), None).unwrap();
        let a = p.notarize_args(Path::new("/o/a.zip")).unwrap();
        assert_eq!(&a[..2], ["notarytool", "submit"]);
        assert!(a.join(" ").contains("--keychain-profile alloy --wait"));
        let ap = MacSign::from_env(
            &env(&[
                id,
                ("ALLOY_NOTARY_APPLE_ID", "a@x.com"),
                ("ALLOY_NOTARY_TEAM_ID", "T"),
                ("ALLOY_NOTARY_PASSWORD", "pw"),
            ]),
            None,
        )
        .unwrap();
        assert!(ap
            .notarize_args(Path::new("z"))
            .unwrap()
            .join(" ")
            .contains("--apple-id a@x.com --team-id T --password pw"));
        let key = MacSign::from_env(
            &env(&[
                id,
                ("ALLOY_NOTARY_KEY", "/k.p8"),
                ("ALLOY_NOTARY_KEY_ID", "K"),
                ("ALLOY_NOTARY_ISSUER", "I"),
            ]),
            None,
        )
        .unwrap();
        assert!(key
            .notarize_args(Path::new("z"))
            .unwrap()
            .join(" ")
            .contains("--key /k.p8 --key-id K --issuer I"));
        // Incomplete credentials and ad hoc + notary are rejected up front.
        assert!(
            MacSign::from_env(&env(&[id, ("ALLOY_NOTARY_APPLE_ID", "a@x.com")]), None).is_err()
        );
        assert!(MacSign::from_env(&env(&[("ALLOY_NOTARY_PROFILE", "alloy")]), None).is_err());
        assert_eq!(
            MacSign::staple_args(Path::new("/o/D.app")),
            ["stapler", "staple", "/o/D.app"]
        );
        assert!(MacSign::from_env(&env(&[]), None)
            .unwrap()
            .notarize_args(Path::new("z"))
            .is_none());
    }

    #[test]
    fn windows_signtool_and_osslsigncode_command_lines() {
        let st = |n: &str| (n == "signtool").then(|| PathBuf::from("signtool.exe"));
        let w = WinSign::from_env(
            &env(&[
                ("ALLOY_WIN_CERT", "c.pfx"),
                ("ALLOY_WIN_CERT_PASSWORD", "pw"),
            ]),
            &st,
        )
        .unwrap()
        .unwrap();
        let (prog, a) = w.command(Path::new("a.exe"), Path::new("a.signed"));
        assert_eq!(prog, "signtool.exe");
        assert_eq!(
            a.join(" "),
            "sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f c.pfx /p pw a.exe"
        );
        let store = WinSign::from_env(
            &env(&[
                ("ALLOY_WIN_CERT_SHA1", "AB12"),
                ("ALLOY_WIN_TIMESTAMP_URL", "http://t"),
            ]),
            &st,
        )
        .unwrap()
        .unwrap();
        assert!(store
            .command(Path::new("a.exe"), Path::new("x"))
            .1
            .join(" ")
            .contains("/tr http://t /td SHA256 /sha1 AB12 a.exe"));

        let os = |n: &str| (n == "osslsigncode").then(|| PathBuf::from("osslsigncode"));
        let o = WinSign::from_env(
            &env(&[
                ("ALLOY_WIN_CERT", "c.pfx"),
                ("ALLOY_WIN_CERT_PASSWORD", "pw"),
            ]),
            &os,
        )
        .unwrap()
        .unwrap();
        let (prog, a) = o.command(Path::new("a.msi"), Path::new("a.signed"));
        assert_eq!(prog, "osslsigncode");
        assert_eq!(
            a.join(" "),
            "sign -h sha256 -ts http://timestamp.digicert.com -pkcs12 c.pfx -pass pw -in a.msi -out a.signed"
        );
        assert!(WinSign::from_env(&env(&[("ALLOY_WIN_CERT_SHA1", "AB")]), &os).is_err());
    }

    #[test]
    fn windows_custom_command_and_errors() {
        let w = WinSign::from_env(
            &env(&[("ALLOY_WIN_SIGN_COMMAND", "azsign --in {file}")]),
            &tools,
        )
        .unwrap()
        .unwrap();
        let (_, a) = w.command(Path::new("x.exe"), Path::new("y"));
        assert_eq!(a.last().unwrap(), "azsign --in x.exe");
        assert!(WinSign::from_env(&env(&[("ALLOY_WIN_SIGN_COMMAND", "azsign")]), &tools).is_err());
        assert!(WinSign::from_env(&env(&[]), &tools).unwrap().is_none());
        assert!(matches!(
            WinSign::from_env(&env(&[("ALLOY_WIN_CERT", "c.pfx")]), &tools),
            Err(Error::ToolMissing { .. })
        ));
    }
}
