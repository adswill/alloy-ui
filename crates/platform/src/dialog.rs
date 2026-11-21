//! Pure helpers for parsing native file-dialog output. Kept free of OS calls so
//! every platform's tests can exercise them.

use std::path::PathBuf;

/// Split newline-separated paths printed by `zenity`/`kdialog`/`osascript`.
pub fn parse_lines(out: &str) -> Vec<PathBuf> {
    out.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Parse the buffer `GetOpenFileNameW` fills with `OFN_EXPLORER |
/// OFN_ALLOWMULTISELECT`: a single full path, or `dir\0name1\0name2\0\0`.
pub fn parse_multiselect(buf: &[u16]) -> Vec<PathBuf> {
    let mut parts = buf
        .split(|&c| c == 0)
        .take_while(|s| !s.is_empty())
        .map(String::from_utf16_lossy);
    let Some(first) = parts.next() else {
        return Vec::new();
    };
    let rest: Vec<String> = parts.collect();
    if rest.is_empty() {
        return vec![PathBuf::from(first)];
    }
    let dir = PathBuf::from(first);
    rest.into_iter().map(|n| dir.join(n)).collect()
}

/// What a file dialog asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogKind {
    /// One existing file.
    #[default]
    OpenFile,
    /// Several existing files.
    OpenFiles,
    /// One existing folder.
    Folder,
    /// A new or existing file to write.
    Save,
}

/// A named group of file extensions, such as `Images` with `png` and `jpg`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFilter {
    /// Shown in the dialog's type menu.
    pub name: String,
    /// Extensions without the dot; empty means every file.
    pub extensions: Vec<String>,
}

/// Everything a native file dialog can be told. See [`show`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDialogOptions {
    /// What to pick.
    pub kind: DialogKind,
    /// Window title (the OS default when `None`).
    pub title: Option<String>,
    /// Folder to start in.
    pub dir: Option<PathBuf>,
    /// Suggested file name for [`DialogKind::Save`].
    pub name: String,
    /// File types to offer; none means every file.
    pub filters: Vec<FileFilter>,
}

/// Show a native file dialog and wait for it. An empty list means the user cancelled.
/// Errors where the OS has no dialog (Linux without `zenity` or `kdialog`).
pub fn show(opts: &FileDialogOptions) -> Result<Vec<PathBuf>, crate::PlatformError> {
    crate::os::file_dialog(opts)
}

/// Arguments for `zenity --file-selection`, in the order the program wants them.
pub fn zenity_args(o: &FileDialogOptions) -> Vec<String> {
    let mut a = vec!["--file-selection".to_string()];
    match o.kind {
        DialogKind::OpenFile => {}
        DialogKind::OpenFiles => {
            a.push("--multiple".into());
            a.push("--separator=\n".into());
        }
        DialogKind::Folder => a.push("--directory".into()),
        DialogKind::Save => {
            a.push("--save".into());
            a.push("--confirm-overwrite".into());
        }
    }
    if let Some(t) = &o.title {
        a.push(format!("--title={t}"));
    }
    // zenity takes the start folder and the suggested name in one `--filename`.
    let mut start = o
        .dir
        .clone()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    if !o.name.is_empty() {
        if !start.is_empty() && !start.ends_with('/') {
            start.push('/');
        }
        start.push_str(&o.name);
    } else if !start.is_empty() && !start.ends_with('/') {
        start.push('/');
    }
    if !start.is_empty() {
        a.push(format!("--filename={start}"));
    }
    if o.kind != DialogKind::Folder {
        for f in &o.filters {
            let globs: Vec<String> = f.extensions.iter().map(|e| format!("*.{e}")).collect();
            a.push(format!("--file-filter={} | {}", f.name, globs.join(" ")));
        }
    }
    a
}

/// Arguments for `kdialog`, which names the dialog kind first.
pub fn kdialog_args(o: &FileDialogOptions) -> Vec<String> {
    let start = {
        let dir = o
            .dir
            .clone()
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| ".".into());
        if o.name.is_empty() {
            dir
        } else {
            format!("{}/{}", dir.trim_end_matches('/'), o.name)
        }
    };
    let mut a = match o.kind {
        DialogKind::OpenFile => vec!["--getopenfilename".to_string(), start],
        DialogKind::OpenFiles => vec![
            "--getopenfilename".to_string(),
            start,
            "--multiple".into(),
            "--separate-output".into(),
        ],
        DialogKind::Folder => vec!["--getexistingdirectory".to_string(), start],
        DialogKind::Save => vec!["--getsavefilename".to_string(), start],
    };
    if o.kind != DialogKind::Folder && !o.filters.is_empty() {
        // One filter per line: "*.png *.jpg|Images".
        let lines: Vec<String> = o
            .filters
            .iter()
            .map(|f| {
                let globs: Vec<String> = f.extensions.iter().map(|e| format!("*.{e}")).collect();
                let globs = if globs.is_empty() {
                    "*".to_string()
                } else {
                    globs.join(" ")
                };
                format!("{globs}|{}", f.name)
            })
            .collect();
        a.push(lines.join("\n"));
    }
    if let Some(t) = &o.title {
        a.push("--title".into());
        a.push(t.clone());
    }
    a
}

/// The AppleScript that shows the dialog, for use when AppKit is not reachable
/// (called from a background thread). It prints one POSIX path per line.
pub fn applescript(o: &FileDialogOptions) -> String {
    let q = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut opts = String::new();
    if let Some(t) = &o.title {
        opts.push_str(&format!(" with prompt \"{}\"", q(t)));
    }
    if let Some(d) = &o.dir {
        opts.push_str(&format!(
            " default location (POSIX file \"{}\")",
            q(&d.display().to_string())
        ));
    }
    let types: Vec<String> = o
        .filters
        .iter()
        .flat_map(|f| f.extensions.iter())
        .map(|e| format!("\"{}\"", q(e)))
        .collect();
    let of_type = if types.is_empty() || o.kind == DialogKind::Folder || o.kind == DialogKind::Save
    {
        String::new()
    } else {
        format!(" of type {{{}}}", types.join(", "))
    };
    match o.kind {
        DialogKind::OpenFile => format!("POSIX path of (choose file{of_type}{opts})"),
        DialogKind::OpenFiles => format!(
            "set fs to choose file{of_type}{opts} with multiple selections allowed\nset out to \"\"\nrepeat with f in fs\nset out to out & POSIX path of f & linefeed\nend repeat\nreturn out"
        ),
        DialogKind::Folder => format!("POSIX path of (choose folder{opts})"),
        DialogKind::Save => {
            let name = if o.name.is_empty() {
                String::new()
            } else {
                format!(" default name \"{}\"", q(&o.name))
            };
            format!("POSIX path of (choose file name{name}{opts})")
        }
    }
}

/// Windows `COMDLG_FILTERSPEC` pairs: display name and `*.a;*.b` pattern.
pub fn windows_filters(filters: &[FileFilter]) -> Vec<(String, String)> {
    filters
        .iter()
        .map(|f| {
            let pat = if f.extensions.is_empty() {
                "*.*".to_string()
            } else {
                f.extensions
                    .iter()
                    .map(|e| format!("*.{e}"))
                    .collect::<Vec<_>>()
                    .join(";")
            };
            (f.name.clone(), pat)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(kind: DialogKind) -> FileDialogOptions {
        FileDialogOptions {
            kind,
            title: Some("Pick".into()),
            dir: Some(PathBuf::from("/home/me")),
            name: "a.txt".into(),
            filters: vec![FileFilter {
                name: "Images".into(),
                extensions: vec!["png".into(), "jpg".into()],
            }],
        }
    }

    #[test]
    fn zenity_open_and_save() {
        let a = zenity_args(&opts(DialogKind::OpenFiles));
        assert!(a.contains(&"--multiple".to_string()));
        assert!(a.contains(&"--title=Pick".to_string()));
        assert!(a.contains(&"--filename=/home/me/a.txt".to_string()));
        assert!(a.contains(&"--file-filter=Images | *.png *.jpg".to_string()));
        let s = zenity_args(&opts(DialogKind::Save));
        assert!(
            s.contains(&"--save".to_string()) && s.contains(&"--confirm-overwrite".to_string())
        );
        let f = zenity_args(&opts(DialogKind::Folder));
        assert!(f.contains(&"--directory".to_string()));
        assert!(!f.iter().any(|x| x.starts_with("--file-filter")));
    }

    #[test]
    fn kdialog_kinds() {
        let a = kdialog_args(&opts(DialogKind::OpenFile));
        assert_eq!(a[0], "--getopenfilename");
        assert_eq!(a[1], "/home/me/a.txt");
        assert!(a.contains(&"*.png *.jpg|Images".to_string()));
        assert_eq!(
            kdialog_args(&opts(DialogKind::Folder))[0],
            "--getexistingdirectory"
        );
        assert_eq!(
            kdialog_args(&opts(DialogKind::Save))[0],
            "--getsavefilename"
        );
    }

    #[test]
    fn applescript_forms() {
        let s = applescript(&opts(DialogKind::OpenFile));
        assert!(s.starts_with("POSIX path of (choose file of type {\"png\", \"jpg\"}"));
        assert!(s.contains("with prompt \"Pick\""));
        assert!(applescript(&opts(DialogKind::Folder)).contains("choose folder"));
        let save = applescript(&opts(DialogKind::Save));
        assert!(save.contains("choose file name default name \"a.txt\""));
        assert!(!save.contains("of type"));
        let mut quoted = opts(DialogKind::Save);
        quoted.name = "a\"b".into();
        assert!(applescript(&quoted).contains("a\\\"b"));
    }

    #[test]
    fn windows_patterns() {
        let f = windows_filters(&opts(DialogKind::OpenFile).filters);
        assert_eq!(f, vec![("Images".to_string(), "*.png;*.jpg".to_string())]);
        assert_eq!(
            windows_filters(&[FileFilter {
                name: "All".into(),
                extensions: vec![]
            }])[0]
                .1,
            "*.*"
        );
    }

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn lines() {
        assert_eq!(
            parse_lines("/a/b\n/c d/e\n\n"),
            vec![PathBuf::from("/a/b"), PathBuf::from("/c d/e")]
        );
        assert!(parse_lines("\n").is_empty());
    }

    #[test]
    fn multiselect_single_and_many() {
        let mut one = w(r"C:\x\a.txt");
        one.extend([0, 0, 0]);
        assert_eq!(parse_multiselect(&one), vec![PathBuf::from(r"C:\x\a.txt")]);

        let mut many = w(r"C:\x");
        many.push(0);
        many.extend(w("a.txt"));
        many.push(0);
        many.extend(w("b.txt"));
        many.extend([0, 0, 0]);
        assert_eq!(
            parse_multiselect(&many),
            vec![
                PathBuf::from(r"C:\x").join("a.txt"),
                PathBuf::from(r"C:\x").join("b.txt")
            ]
        );
        assert!(parse_multiselect(&[0, 0]).is_empty());
    }
}
