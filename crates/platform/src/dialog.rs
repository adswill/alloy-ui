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

#[cfg(test)]
mod tests {
    use super::*;

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
