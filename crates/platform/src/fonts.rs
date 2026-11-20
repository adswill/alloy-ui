//! Font discovery: where each OS keeps fonts, and a per-OS default fallback
//! chain. The renderer loads the files; this module only finds them.

use std::path::{Path, PathBuf};

/// Directories that hold system fonts on the current OS.
pub fn system_font_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut d: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        d.extend(["/System/Library/Fonts", "/Library/Fonts"].map(PathBuf::from));
        d.extend(home.iter().map(|h| h.join("Library/Fonts")));
    } else if cfg!(target_os = "windows") {
        let root = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
        d.push(PathBuf::from(root).join("Fonts"));
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            d.push(PathBuf::from(l).join("Microsoft/Windows/Fonts"));
        }
    } else {
        d.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
        d.extend(
            home.iter()
                .flat_map(|h| [h.join(".fonts"), h.join(".local/share/fonts")]),
        );
    }
    d
}

/// Preferred family names for UI text, in fallback order, for the current OS.
pub fn default_families() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &[
            "SF Pro Text",
            "Helvetica Neue",
            "Arial Unicode MS",
            "Apple Color Emoji",
        ]
    } else if cfg!(target_os = "windows") {
        &[
            "Segoe UI",
            "Arial",
            "Segoe UI Emoji",
            "Segoe UI Symbol",
            "MS Gothic",
        ]
    } else {
        &[
            "Noto Sans",
            "DejaVu Sans",
            "Liberation Sans",
            "Noto Color Emoji",
            "Noto Sans CJK SC",
        ]
    }
}

fn is_font(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("ttf" | "otf" | "ttc" | "otc")
    )
}

/// Recursively list font files under `dir` (depth-limited, symlink-safe).
pub fn scan_dir(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth == 0 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            scan_dir(&p, out, depth - 1);
        } else if is_font(&p) {
            out.push(p);
        }
    }
}

/// All font files installed on the system.
pub fn discover() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for d in system_font_dirs() {
        scan_dir(&d, &mut v, 6);
    }
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_fonts_recursively() {
        let root = std::env::temp_dir().join(format!("alloy-fonts-{}", std::process::id()));
        let sub = root.join("a/b");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("x.TTF"), b"").unwrap();
        std::fs::write(root.join("y.otf"), b"").unwrap();
        std::fs::write(root.join("note.txt"), b"").unwrap();
        let mut v = Vec::new();
        scan_dir(&root, &mut v, 6);
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn has_fallbacks() {
        assert!(default_families().len() >= 3);
        assert!(!system_font_dirs().is_empty());
    }
}
