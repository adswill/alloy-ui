//! The command palette: `mod+k` opens a searchable list of every command and shortcut the app
//! declared. State and matching live here; [`Chrome`](crate::chrome) routes events and draws it.

use std::cell::RefCell;

#[derive(Default)]
pub(crate) struct State {
    pub open: bool,
    pub query: String,
    pub sel: usize,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(crate) fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub(crate) fn is_open() -> bool {
    with(|s| s.open)
}

/// Open or close; opening starts with an empty query and the first row selected.
pub(crate) fn toggle() {
    with(|s| {
        s.open = !s.open;
        s.query.clear();
        s.sel = 0;
    });
}

pub(crate) fn close() {
    with(|s| *s = State::default());
}

/// How well `query` matches `title`: `None` if its letters are not found in order, otherwise a
/// score where higher is better (word starts and consecutive letters count most).
pub(crate) fn score(query: &str, title: &str) -> Option<i32> {
    let q: Vec<char> = query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = title.to_lowercase().chars().collect();
    let (mut qi, mut total, mut run) = (0, 0, 0);
    for (i, &c) in t.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            let word_start = i == 0 || !t[i - 1].is_alphanumeric();
            total += 10 + run * 6 + if word_start { 14 } else { 0 };
            if qi == 0 && i == 0 {
                total += 10;
            }
            run += 1;
            qi += 1;
        } else {
            run = 0;
            total -= 1;
        }
    }
    (qi == q.len()).then_some(total)
}

/// Indices into `titles` that match, best first (ties keep the declared order).
pub(crate) fn rank(query: &str, titles: &[String]) -> Vec<usize> {
    let mut hits: Vec<(i32, usize)> = titles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| score(query, t).map(|s| (s, i)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, i)| i).collect()
}

/// `"mod+shift+z"` as the user would read it: `⌘⇧Z` on macOS, `Ctrl+Shift+Z` elsewhere.
pub(crate) fn pretty(keys: &str) -> String {
    let mac = cfg!(target_os = "macos");
    let parts: Vec<String> = keys
        .split('+')
        .map(|p| match (p.to_lowercase().as_str(), mac) {
            ("mod", true) | ("cmd", _) | ("super", true) => "⌘".into(),
            ("mod", false) | ("ctrl", false) | ("control", false) => "Ctrl".into(),
            ("ctrl", true) | ("control", true) => "⌃".into(),
            ("shift", true) => "⇧".into(),
            ("shift", false) => "Shift".into(),
            ("alt", true) | ("option", true) => "⌥".into(),
            ("alt", false) | ("option", false) => "Alt".into(),
            (k, _) if k.chars().count() == 1 => k.to_uppercase(),
            (k, _) => {
                let mut c = k.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            }
        })
        .collect();
    if mac {
        parts.concat()
    } else {
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy() {
        assert!(score("ecsv", "Export CSV").is_some());
        assert!(score("xyz", "Export CSV").is_none());
        let t = vec![
            "Toggle theme".to_string(),
            "Export CSV".into(),
            "New note".into(),
        ];
        assert_eq!(rank("", &t), vec![0, 1, 2]);
        assert_eq!(rank("exp", &t), vec![1]);
        // A word start beats a letter buried in a word.
        assert_eq!(rank("n", &t)[0], 2);
    }

    #[test]
    fn keys() {
        if !cfg!(target_os = "macos") {
            assert_eq!(pretty("mod+shift+z"), "Ctrl+Shift+Z");
            assert_eq!(pretty("f5"), "F5");
        }
    }
}
