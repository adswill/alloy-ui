//! Crash reports: a panic hook that writes a JSON file, and helpers to find
//! and upload the files on the next launch.
//!
//! ```no_run
//! use alloy_ui_ship::crash::{self, Config};
//! let _guard = crash::install(Config::new("dev.example.app", env!("CARGO_PKG_VERSION")));
//! // Later, after asking the user:
//! // crash::upload_pending(&dir, "https://example.com/crash", &alloy_ui_ship::UreqFetch::default());
//! ```
//!
//! What is covered: Rust panics on any thread, including with
//! `panic = "abort"` (the hook runs before the abort). What is not: native
//! faults (segfaults in a driver) and the OS killing the process; catching
//! those needs a signal or exception handler, which Alloy does not install.
//!
//! Privacy: nothing is sent unless the app calls [`upload_pending`]. A report
//! holds the panic message, source location, a backtrace, the app and
//! OS/architecture, and a timestamp. Panic messages can contain user data;
//! the `scrub` option lets an app redact them before they are written.
//!
//! Release builds with `strip = true` produce backtraces of bare addresses;
//! keep `debug = "line-tables-only"` and `strip = "debuginfo"` (or ship the
//! symbols) if you want readable frames.

use crate::net::Fetch;
use crate::{io, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One crash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// App identifier.
    pub app: String,
    /// App version.
    pub version: String,
    /// `std::env::consts::OS`.
    pub os: String,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// Seconds since the Unix epoch.
    pub time: u64,
    /// Name of the panicking thread.
    pub thread: String,
    /// The panic message.
    pub message: String,
    /// `file:line:column`, when known.
    pub location: String,
    /// `std::backtrace::Backtrace` text.
    pub backtrace: String,
}

type Hook = Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync>;
type Scrub = Arc<dyn Fn(&str) -> String + Send + Sync>;
type OnCrash = Arc<dyn Fn(&Report, &Path) + Send + Sync>;

/// How to record crashes.
#[derive(Clone)]
pub struct Config {
    /// App identifier.
    pub app: String,
    /// App version.
    pub version: String,
    /// Where reports are written. Default: [`default_dir`].
    pub dir: PathBuf,
    /// Keep at most this many reports on disk (oldest are deleted). Default 10.
    pub keep: usize,
    scrub: Option<Scrub>,
    on_crash: Option<OnCrash>,
}

impl Config {
    /// Defaults for `app`.
    pub fn new(app: &str, version: &str) -> Self {
        Self {
            app: app.into(),
            version: version.into(),
            dir: default_dir(app),
            keep: 10,
            scrub: None,
            on_crash: None,
        }
    }

    /// Redact the message and backtrace before they are written.
    pub fn scrub(mut self, f: impl Fn(&str) -> String + Send + Sync + 'static) -> Self {
        self.scrub = Some(Arc::new(f));
        self
    }

    /// Run `f` after the report is on disk (e.g. show a "send report?" dialog
    /// on next launch, or flush app state). Keep it quick and panic-free.
    pub fn on_crash(mut self, f: impl Fn(&Report, &Path) + Send + Sync + 'static) -> Self {
        self.on_crash = Some(Arc::new(f));
        self
    }

    /// Write reports to `dir`.
    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = dir.into();
        self
    }
}

/// `<state dir>/<app>/crashes`: `$XDG_STATE_HOME` or `~/.local/state` on Linux,
/// `~/Library/Logs` on macOS, `%LOCALAPPDATA%` on Windows.
pub fn default_dir(app: &str) -> PathBuf {
    let var = |k: &str| {
        std::env::var_os(k)
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
    };
    let base = match std::env::consts::OS {
        "windows" => var("LOCALAPPDATA"),
        "macos" => var("HOME").map(|h| h.join("Library/Logs")),
        _ => var("XDG_STATE_HOME").or_else(|| var("HOME").map(|h| h.join(".local/state"))),
    }
    .unwrap_or_else(std::env::temp_dir);
    base.join(app).join("crashes")
}

/// Keeps the hook installed; dropping it restores the previous hook.
pub struct Guard {
    previous: Option<Hook>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(p) = self.previous.take() {
            let _ = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |i| p(i)));
        }
    }
}

/// Install the panic hook. The previous hook still runs afterwards, so the
/// default message on stderr is kept.
pub fn install(cfg: Config) -> Guard {
    let previous: Hook = Arc::from(std::panic::take_hook());
    let chained = previous.clone();
    std::panic::set_hook(Box::new(move |info| {
        let report = build_report(&cfg, info);
        // Never panic inside the hook: a failure to write is ignored.
        if let Ok(path) = write_report(&cfg.dir, &report, cfg.keep) {
            if let Some(f) = &cfg.on_crash {
                f(&report, &path);
            }
        }
        chained(info);
    }));
    Guard {
        previous: Some(previous),
    }
}

fn build_report(cfg: &Config, info: &std::panic::PanicHookInfo<'_>) -> Report {
    let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "(non-string panic payload)".into()
    };
    let scrub = |s: String| cfg.scrub.as_ref().map_or(s.clone(), |f| f(&s));
    Report {
        app: cfg.app.clone(),
        version: cfg.version.clone(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        time: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        thread: std::thread::current().name().unwrap_or("<unnamed>").into(),
        message: scrub(message),
        location: info.location().map_or(String::new(), |l| l.to_string()),
        backtrace: scrub(std::backtrace::Backtrace::force_capture().to_string()),
    }
}

/// Write `report` into `dir` (created if needed) and trim old reports.
pub fn write_report(dir: &Path, report: &Report, keep: usize) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).map_err(io(format!("creating {}", dir.display())))?;
    let name = format!("{:012}-{}.json", report.time, std::process::id());
    let path = dir.join(&name);
    let tmp = dir.join(format!("{name}.tmp"));
    let json = serde_json::to_vec_pretty(report).expect("report serialises");
    std::fs::write(&tmp, json).map_err(io("writing crash report"))?;
    std::fs::rename(&tmp, &path).map_err(io("writing crash report"))?;
    let mut all = list(dir);
    while all.len() > keep.max(1) {
        std::fs::remove_file(all.remove(0)).ok();
    }
    Ok(path)
}

/// Report files in `dir`, oldest first.
pub fn list(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    v.sort();
    v
}

/// Reports waiting from earlier runs, oldest first. Unreadable files are skipped.
pub fn pending(dir: &Path) -> Vec<(PathBuf, Report)> {
    list(dir)
        .into_iter()
        .filter_map(|p| {
            let r = serde_json::from_slice(&std::fs::read(&p).ok()?).ok()?;
            Some((p, r))
        })
        .collect()
}

/// POST each pending report (as JSON) to `endpoint` and delete the ones the
/// server accepted. Returns how many were sent. Stops at the first failure so
/// an offline machine does one request, not ten.
pub fn upload_pending(dir: &Path, endpoint: &str, fetch: &dyn Fetch) -> Result<usize> {
    let mut sent = 0;
    for (path, _) in pending(dir) {
        let body = std::fs::read(&path).map_err(io("reading crash report"))?;
        fetch.post(endpoint, "application/json", &body)?;
        std::fs::remove_file(&path).ok();
        sent += 1;
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Mutex;

    // The panic hook is process-global; serialise the tests that touch it.
    static HOOK: Mutex<()> = Mutex::new(());

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("alloy-crash-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        d
    }

    #[test]
    fn panic_writes_a_report_then_chains() {
        let _l = HOOK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tmp("hook");
        let seen = Arc::new(Mutex::new(None));
        let s2 = seen.clone();
        let cfg = Config::new("dev.x.app", "1.2.3")
            .dir(&dir)
            .scrub(|s| s.replace("hunter2", "<redacted>"))
            .on_crash(move |r, p| *s2.lock().unwrap() = Some((r.message.clone(), p.to_path_buf())));
        let guard = install(cfg);
        let r = std::thread::Builder::new()
            .name("worker".into())
            .spawn(|| panic!("bad password hunter2"))
            .unwrap()
            .join();
        assert!(r.is_err());
        drop(guard);
        let got = pending(&dir);
        assert_eq!(got.len(), 1);
        let rep = &got[0].1;
        assert_eq!(
            (rep.app.as_str(), rep.version.as_str()),
            ("dev.x.app", "1.2.3")
        );
        assert_eq!(rep.thread, "worker");
        assert_eq!(rep.message, "bad password <redacted>");
        assert!(rep.location.contains("crash.rs"), "{}", rep.location);
        assert!(!rep.backtrace.is_empty());
        assert_eq!(
            seen.lock().unwrap().as_ref().unwrap().0,
            "bad password <redacted>"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn keeps_only_the_newest_reports() {
        let dir = tmp("keep");
        for t in 0..5 {
            let r = Report {
                app: "a".into(),
                version: "1".into(),
                os: "o".into(),
                arch: "x".into(),
                time: t,
                thread: "t".into(),
                message: format!("m{t}"),
                location: String::new(),
                backtrace: String::new(),
            };
            write_report(&dir, &r, 3).unwrap();
        }
        let left: Vec<_> = pending(&dir).into_iter().map(|(_, r)| r.message).collect();
        assert_eq!(left, ["m2", "m3", "m4"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    struct Sink {
        posts: RefCell<Vec<Vec<u8>>>,
        fail: bool,
    }
    impl Fetch for Sink {
        fn get(&self, _: &str) -> Result<Vec<u8>> {
            unreachable!()
        }
        fn post(&self, _: &str, ct: &str, body: &[u8]) -> Result<()> {
            assert_eq!(ct, "application/json");
            if self.fail {
                return Err(crate::Error::Network("offline".into()));
            }
            self.posts.borrow_mut().push(body.to_vec());
            Ok(())
        }
    }

    #[test]
    fn upload_deletes_only_what_was_accepted() {
        let dir = tmp("up");
        let mk = |t| Report {
            app: "a".into(),
            version: "1".into(),
            os: "o".into(),
            arch: "x".into(),
            time: t,
            thread: "t".into(),
            message: "m".into(),
            location: String::new(),
            backtrace: String::new(),
        };
        write_report(&dir, &mk(1), 10).unwrap();
        write_report(&dir, &mk(2), 10).unwrap();
        let down = Sink {
            posts: RefCell::new(vec![]),
            fail: true,
        };
        assert!(upload_pending(&dir, "https://c.example", &down).is_err());
        assert_eq!(list(&dir).len(), 2, "kept for the next launch");
        let up = Sink {
            posts: RefCell::new(vec![]),
            fail: false,
        };
        assert_eq!(upload_pending(&dir, "https://c.example", &up).unwrap(), 2);
        assert!(list(&dir).is_empty());
        let back: Report = serde_json::from_slice(&up.posts.borrow()[0]).unwrap();
        assert_eq!(back.time, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_dir_is_per_app() {
        assert!(default_dir("dev.x.app").ends_with("dev.x.app/crashes") || cfg!(windows));
    }
}
