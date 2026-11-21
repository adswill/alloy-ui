//! Shared helpers for friendly compile errors: "did you mean" suggestions.

/// Levenshtein distance, small inputs only.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The candidate closest to `word`, when it is close enough to be a likely typo.
pub fn closest<'a>(word: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .map(|c| (distance(word, c), *c))
        .filter(|(d, c)| *d <= 2.max(c.len() / 3))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// `a`, `b` and `c` as "`a`, `b` and `c`".
pub fn list(items: &[&str]) -> String {
    let q: Vec<String> = items.iter().map(|i| format!("`{i}`")).collect();
    match q.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {}", rest.join(", "), last),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_close_words_only() {
        assert_eq!(closest("lable", &["label", "email"]), Some("label"));
        assert_eq!(closest("rnage", &["range", "step"]), Some("range"));
        assert_eq!(closest("banana", &["label", "email"]), None);
    }

    #[test]
    fn lists() {
        assert_eq!(list(&["a"]), "`a`");
        assert_eq!(list(&["a", "b", "c"]), "`a`, `b` and `c`");
    }
}
