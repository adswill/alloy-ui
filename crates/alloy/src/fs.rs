//! File-system helpers that respect the app's capability grants.
//!
//! By default everything is allowed. Call [`grant`] once at start-up with a
//! [`CapabilitySet`] (usually `CapabilitySet::from_file("capabilities.toml")`) and every
//! helper here fails with a readable message for paths outside the granted `fs_read` /
//! `fs_write` scopes, instead of touching the disk.

use crate::cx::LockExt;
use alloy_ui_ipc::CapabilitySet;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

static GRANT: Mutex<Option<CapabilitySet>> = Mutex::new(None);
type Cached = (SystemTime, Arc<Vec<Entry>>);
static LISTINGS: Mutex<Option<HashMap<PathBuf, Cached>>> = Mutex::new(None);

/// Restrict the helpers in this module to `caps`.
pub fn grant(caps: CapabilitySet) {
    *GRANT.lock_ok() = Some(caps);
}

fn allowed(path: &Path, write: bool) -> Result<(), String> {
    match &*GRANT.lock_ok() {
        None => Ok(()),
        Some(c) if write => c.check_fs_write(path).map_err(|e| e.to_string()),
        Some(c) => c.check_fs_read(path).map_err(|e| e.to_string()),
    }
}

/// One item of a directory listing.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// File name.
    pub name: String,
    /// Full path.
    pub path: PathBuf,
    /// Whether it is a directory.
    pub dir: bool,
    /// Size in bytes (0 for directories).
    pub size: u64,
}

impl Entry {
    /// The name, with a trailing `/` for directories.
    pub fn label(&self) -> String {
        if self.dir {
            format!("{}/", self.name)
        } else {
            self.name.clone()
        }
    }
    /// The size as `12.3 KB`, empty for directories.
    pub fn size_text(&self) -> String {
        if self.dir {
            String::new()
        } else {
            human_size(self.size)
        }
    }
    /// Names starting with a dot.
    pub fn hidden(&self) -> bool {
        self.name.starts_with('.')
    }
}

/// The entries of `dir`, folders first then by name, case-insensitively. The listing is cached
/// until the directory changes, so calling this from `view` on every frame is cheap.
pub fn list(dir: &Path) -> Result<Arc<Vec<Entry>>, String> {
    allowed(dir, false)?;
    let stamp = std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?;
    let mut cache = LISTINGS.lock_ok();
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some((_, rows)) = cache.get(dir).filter(|(at, _)| *at == stamp) {
        return Ok(rows.clone());
    }
    let mut rows: Vec<Entry> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path(),
                dir: meta.is_dir(),
                size: if meta.is_dir() { 0 } else { meta.len() },
            })
        })
        .collect();
    rows.sort_by_cached_key(|e| (!e.dir, e.name.to_lowercase()));
    let rows = Arc::new(rows);
    cache.insert(dir.to_path_buf(), (stamp, rows.clone()));
    Ok(rows)
}

/// Read a whole file as text.
pub fn read(path: &Path) -> Result<String, String> {
    allowed(path, false)?;
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Write `text` to a file, creating it or replacing it.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    allowed(path, true)?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// The folder the app was started in.
pub fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

/// A byte count as `512 B`, `1.5 KB` or `3.2 MB`.
pub fn human_size(n: u64) -> String {
    match n {
        0..1024 => format!("{n} B"),
        1024..1_048_576 => format!("{:.1} KB", n as f64 / 1024.0),
        1_048_576..1_073_741_824 => format!("{:.1} MB", n as f64 / 1_048_576.0),
        _ => format!("{:.1} GB", n as f64 / 1_073_741_824.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_folders_first_and_respects_grants() {
        let d = std::env::temp_dir().join(format!("alloy-fs-{}", std::process::id()));
        std::fs::create_dir_all(d.join("b_dir")).unwrap();
        std::fs::write(d.join("a.txt"), "hi").unwrap();
        let rows = list(&d).unwrap();
        assert_eq!(rows[0].label(), "b_dir/");
        assert_eq!(rows[1].size_text(), "2 B");
        assert_eq!(read(&d.join("a.txt")).unwrap(), "hi");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(5), "5 B");
    }
}
