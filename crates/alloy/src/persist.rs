//! Where persisted state lives and how it is written.

use std::path::PathBuf;

/// `<config dir>/<name>/state.json` in the platform's usual place.
pub(crate) fn path(name: &str) -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let home = env("HOME")
        .or_else(|| env("USERPROFILE"))
        .unwrap_or_default();
    let base = if let Some(dir) = alloy_ui_platform::app_data_dir() {
        dir
    } else if cfg!(windows) {
        env("APPDATA").unwrap_or(home)
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        home.join("Library/Application Support")
    } else {
        env("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))
    };
    base.join(name).join("state.json")
}

/// Write `state` as JSON, via a temp file so a crash never leaves half a file.
pub(crate) fn write<S: serde::Serialize>(path: &std::path::Path, state: &S) {
    let Ok(json) = serde_json::to_vec_pretty(state) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}
