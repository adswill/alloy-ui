//! A tiny program that updates itself, used by `ci/e2e-update.sh` to prove the
//! whole release path with real processes on every OS: sign a release, serve
//! it over HTTP, download, verify, replace the running executable, restart, and
//! write a crash report.
//!
//! The version is baked in at build time from `DEMO_VERSION` (default 1.0.0).
//!
//! ```text
//! selfupdate_demo version
//! selfupdate_demo keygen                          # prints "<private> <public>"
//! selfupdate_demo release <dir> <version> <base-url> <binary> <private>
//! selfupdate_demo update <manifest-url> <public>  # exit 0 updated, 3 none, 1 error
//! selfupdate_demo crash <dir>                     # panics; leaves a report in <dir>
//! selfupdate_demo crashes <dir>                   # prints each pending report
//! ```

use alloy_ui_ship::manifest::{Artifact, Manifest, SCHEMA};
use alloy_ui_ship::update::{apply, cleanup_old, Applied, Config, Updater};
use alloy_ui_ship::{crash, keys, UreqFetch};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const APP: &str = "dev.alloyui.selfupdate-demo";
const VERSION: &str = match option_env!("DEMO_VERSION") {
    Some(v) => v,
    None => "1.0.0",
};
/// The demo ships a bare executable; real apps use installers (see the guide).
const FORMAT: &str = "bin";

fn main() {
    cleanup_old();
    let a: Vec<String> = std::env::args().skip(1).collect();
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["version"] => println!("{VERSION}"),
        ["keygen"] => {
            let k = keys::generate().expect("keygen");
            println!("{} {}", k.private_hex, k.public_hex);
        }
        ["release", dir, version, base, binary, private] => {
            let data = std::fs::read(binary).expect("read binary");
            let name = PathBuf::from(binary)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let m = Manifest {
                schema: SCHEMA,
                app: APP.into(),
                version: (*version).into(),
                notes: "demo release".into(),
                artifacts: vec![Artifact {
                    os: std::env::consts::OS.into(),
                    arch: std::env::consts::ARCH.into(),
                    format: FORMAT.into(),
                    url: format!("{base}/{name}"),
                    sha256: keys::hex(&Sha256::digest(&data)),
                    size: data.len() as u64,
                }],
            };
            let json = m.to_json();
            let sig = keys::sign(private, json.as_bytes()).expect("sign");
            std::fs::write(PathBuf::from(dir).join("latest.json"), json).unwrap();
            std::fs::write(PathBuf::from(dir).join("latest.json.sig"), sig + "\n").unwrap();
        }
        ["update", url, public] => std::process::exit(update(url, public)),
        ["crash", dir] => {
            let _guard = crash::install(crash::Config::new(APP, VERSION).dir(*dir));
            panic!("demo crash on purpose");
        }
        ["crashes", dir] => {
            let found = crash::pending(std::path::Path::new(dir));
            println!("{} report(s)", found.len());
            for (_, r) in found {
                println!("{} {} {}: {}", r.app, r.version, r.os, r.message);
            }
        }
        _ => {
            eprintln!("usage: selfupdate_demo version|keygen|release|update|crash|crashes");
            std::process::exit(2);
        }
    }
}

fn update(url: &str, public: &str) -> i32 {
    let mut cfg = Config::new(APP, VERSION, url, public);
    cfg.formats = vec![FORMAT.into()];
    let up = Updater::new(cfg, UreqFetch::default());
    let found = match up.check() {
        Ok(Some(u)) => u,
        Ok(None) => {
            println!("up to date ({VERSION})");
            return 3;
        }
        Err(e) => {
            eprintln!("check failed: {e}");
            return 1;
        }
    };
    println!("update available: {} -> {}", VERSION, found.version);
    let dir = std::env::temp_dir().join("alloy-selfupdate-demo");
    let file = match up.download(&found, &dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("download rejected: {e}");
            return 1;
        }
    };
    match apply(&file, &found.artifact) {
        Ok(Applied::Replaced(p)) => {
            println!("replaced {}", p.display());
            0
        }
        Ok(Applied::HandedOff(_)) => 0,
        Err(e) => {
            eprintln!("apply failed: {e}");
            1
        }
    }
}
