//! Live views: a screen written in a tiny indented markup that `alloy dev` reloads the moment
//! you save it, with no rebuild and no restart, keeping all state.
//!
//! ```text
//! # ui/home.alloy
//! theme dark accent=#6D4AFF
//! col page gap=14
//!   heading "Hello, {name}"
//!   text "{count} so far" dim
//!   row gap=8
//!     button "Add" primary @add
//!     button "Reset" ghost @reset
//! ```
//!
//! ```ignore
//! sketch(State::default(), |s| {
//!     live!("ui/home.alloy")
//!         .var("name", &s.name)
//!         .var("count", s.count)
//!         .on("add", |s: &mut State| s.count += 1)
//!         .on("reset", |s| s.count = 0)
//! })
//! ```
//!
//! Layout, copy, spacing and theme come from the file; behaviour stays in Rust, wired by
//! `@name`. In a release build the file is baked into the binary and never read from disk.
//! In `alloy dev` (`ALLOY_DEV=1`) it is re-read on every change, and a mistake shows as a red
//! message over the last good screen instead of closing the window.
//!
//! Elements: `col`, `row` (`gap=`, `pad=`, `w=`, `h=`, flags `page`, `fill`, `center`),
//! `text`, `heading`, `caption` (flags `dim`, `danger`, `success`, `accent`), `button`
//! (flags `primary`, `ghost`, `fill`), `rule`, `space`, `vspace`, `gap` (`w=`, `h=`).
//! `"quoted text"` may contain `{var}`. Comment lines start with `#`. Children are indented.
//! A first `theme` line sets `dark`/`light`, `accent=#rrggbb`, `density=compact|spacious`
//! and `contrast=high`.

use crate::{
    cx::LockExt,
    dsl::{button, caption, heading, text, Act, Txt},
    view::{col, row, Node},
    Look,
};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, SystemTime},
};

use alloy_ui_core::live_syntax::{parse, Item};

// ------------------------------------------------------------------ file registry

#[derive(Default)]
struct Entry {
    stamp: Option<SystemTime>,
    tree: Option<Item>,
    error: Option<String>,
    /// The `theme` line last applied, so a stable file never rebuilds the theme.
    theme: Option<String>,
}

static FILES: Mutex<Vec<(String, Entry)>> = Mutex::new(Vec::new());

fn dev() -> bool {
    std::env::var_os("ALLOY_DEV").is_some()
}

fn apply_source(e: &mut Entry, src: &str) {
    match parse(src) {
        Ok(t) => (e.tree, e.error, e.theme) = (Some(t), None, None),
        Err(m) => e.error = Some(m),
    }
}

/// Re-read `path` if it changed (dev only). True if something new was loaded.
fn refresh(path: &str, e: &mut Entry) -> bool {
    let Ok(stamp) = std::fs::metadata(path).and_then(|m| m.modified()) else {
        return false;
    };
    if e.stamp == Some(stamp) {
        return false;
    }
    e.stamp = Some(stamp);
    match std::fs::read_to_string(path) {
        Ok(src) => apply_source(e, &src),
        Err(err) => e.error = Some(format!("cannot read file: {err}")),
    }
    true
}

fn with_entry<R>(path: &str, embedded: &str, f: impl FnOnce(&mut Entry) -> R) -> R {
    let mut files = FILES.lock_ok();
    let at = match files.iter().position(|(p, _)| p == path) {
        Some(i) => i,
        None => {
            let mut e = Entry::default();
            apply_source(&mut e, embedded);
            files.push((path.to_string(), e));
            files.len() - 1
        }
    };
    if dev() {
        refresh(path, &mut files[at].1);
    }
    f(&mut files[at].1)
}

/// Re-check every live file; the UI rebuilds after each poll so a save shows up at once.
pub(crate) fn poll() {
    for (p, e) in FILES.lock_ok().iter_mut() {
        refresh(p, e);
    }
}

/// The dev-only timer that notices saves. Absent in release builds and when no live view exists.
pub(crate) fn poll_sub<S: 'static>() -> Option<crate::Sub<Act<S>>> {
    (dev() && !FILES.lock_ok().is_empty())
        .then(|| crate::every(Duration::from_millis(250), || Act::new(|_: &mut S| poll())))
}

// ------------------------------------------------------------------ the view

/// A screen described by a live file. Made by [`live!`](crate::live!) or [`live_file`].
pub struct Live<S: 'static> {
    path: String,
    embedded: &'static str,
    vars: HashMap<String, String>,
    acts: HashMap<String, Act<S>>,
}

/// Like [`live!`](crate::live!) with an explicit absolute `path` (read in dev) and the `embedded`
/// source used otherwise.
pub fn live_file<S: 'static>(path: &str, embedded: &'static str) -> Live<S> {
    Live {
        path: path.into(),
        embedded,
        vars: HashMap::new(),
        acts: HashMap::new(),
    }
}

/// Load `ui/home.alloy` (relative to the crate root): read from disk and hot reloaded under
/// `alloy dev`, compiled into the binary otherwise.
#[macro_export]
macro_rules! live {
    ($file:literal) => {
        $crate::live_file(
            concat!(env!("CARGO_MANIFEST_DIR"), "/", $file),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $file)),
        )
    };
}

impl<S: 'static> Live<S> {
    /// Fill `{name}` in the file's text.
    pub fn var(mut self, name: &str, value: impl ToString) -> Self {
        self.vars.insert(name.into(), value.to_string());
        self
    }
    /// Run `f` on the state when a button marked `@name` is pressed.
    pub fn on(mut self, name: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.acts.insert(name.into(), Act::new(f));
        self
    }

    fn fill(&self, s: &str) -> String {
        let mut out = s.to_string();
        for (k, v) in &self.vars {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        out
    }

    fn styled(&self, mut t: Txt, it: &Item) -> Txt {
        for f in &it.flags {
            t = match f.as_str() {
                "dim" => t.dim(),
                "danger" => t.danger(),
                "success" => t.success(),
                "accent" => t.accent(),
                _ => t,
            };
        }
        t
    }

    fn node(&self, it: &Item) -> Option<Node<Act<S>>> {
        let label = self.fill(it.text.as_deref().unwrap_or(""));
        Some(match it.kind.as_str() {
            "col" | "row" => {
                let kids: Vec<Node<Act<S>>> = it.kids.iter().filter_map(|k| self.node(k)).collect();
                let mut f = if it.kind == "col" {
                    col(kids)
                } else {
                    row(kids)
                };
                if it.flag("page") {
                    f = f.page();
                }
                if let Some(g) = it.attr("gap") {
                    f = f.gap(g);
                }
                if let Some(p) = it.attr("pad") {
                    f = f.pad(p);
                }
                if let Some(w) = it.attr("w") {
                    f = f.w(w);
                }
                if let Some(h) = it.attr("h") {
                    f = f.h(h);
                }
                if it.flag("center") {
                    f = f.align_center();
                }
                if it.flag("fill") {
                    f = f.fill();
                }
                f.into()
            }
            "text" => self.styled(text(label), it).into(),
            "heading" => self.styled(heading(label), it).into(),
            "caption" => self.styled(caption(label), it).into(),
            "button" => {
                let mut b = button::<Act<S>>(label);
                if it.flag("primary") {
                    b = b.primary();
                }
                if it.flag("ghost") {
                    b = b.ghost();
                }
                if it.flag("fill") {
                    b = b.fill_w();
                }
                match it.action.as_ref().and_then(|a| self.acts.get(a)) {
                    Some(a) => b.on(a.clone()).into(),
                    None => b.enabled(false).into(),
                }
            }
            "rule" => crate::rule(),
            "space" => crate::space(),
            "vspace" => crate::vspace(),
            "gap" => crate::gap(it.attr("w").unwrap_or(0.0), it.attr("h").unwrap_or(0.0)),
            _ => return None, // `theme` draws nothing
        })
    }

    fn build(self) -> Node<Act<S>> {
        let (tree, error, theme) = with_entry(&self.path, self.embedded, |e| {
            (e.tree.clone(), e.error.clone(), e.theme.clone())
        });
        if let Some(t) = &tree {
            self.apply_theme(t, theme);
        }
        let body = tree.and_then(|t| self.node(&t));
        let name = self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path);
        match (body, error) {
            (Some(b), None) => b,
            (b, Some(e)) => {
                let banner: Node<Act<S>> = text(format!("{name}: {e}")).danger().into();
                col(b.into_iter().fold(vec![banner], |mut v, b| {
                    v.push(b);
                    v
                }))
                .gap(8.0)
                .into()
            }
            (None, None) => text(format!("{name} is empty")).dim().into(),
        }
    }

    fn apply_theme(&self, root: &Item, applied: Option<String>) {
        let Some(t) = root
            .kids
            .iter()
            .chain(Some(root))
            .find(|k| k.kind == "theme")
        else {
            return;
        };
        let key = format!("{:?}{:?}", t.flags, t.attrs);
        if applied.as_deref() == Some(&key) {
            return;
        }
        let mut look = if t.flag("dark") {
            Look::dark()
        } else {
            Look::light()
        };
        for (k, v) in &t.attrs {
            look = match k.as_str() {
                "accent" => match u32::from_str_radix(v.trim_start_matches('#'), 16) {
                    Ok(c) => look.accent(alloy_ui_core::Color::hex(c)),
                    Err(_) => look,
                },
                "density" => look.density(match v.as_str() {
                    "compact" => alloy_ui_theme::Density::Compact,
                    "spacious" => alloy_ui_theme::Density::Spacious,
                    _ => alloy_ui_theme::Density::Comfortable,
                }),
                "contrast" => look.high_contrast(v == "high"),
                _ => look,
            };
        }
        crate::set_look(look);
        if let Some((_, e)) = FILES.lock_ok().iter_mut().find(|(p, _)| *p == self.path) {
            e.theme = Some(key);
        }
    }
}

impl<S: 'static> From<Live<S>> for Node<Act<S>> {
    fn from(l: Live<S>) -> Self {
        l.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nesting_flags_and_actions() {
        let t =
            parse("col gap=4\n  text \"a b\" dim\n  row\n    button \"Go\" primary @go\n").unwrap();
        assert_eq!(t.kind, "col");
        assert_eq!(t.attr("gap"), Some(4.0));
        assert_eq!(t.kids.len(), 2);
        assert!(t.kids[0].flag("dim") && t.kids[0].text.as_deref() == Some("a b"));
        assert_eq!(t.kids[1].kids[0].action.as_deref(), Some("go"));
    }

    #[test]
    fn errors_name_the_line() {
        assert!(parse("col\n  bttn \"x\"")
            .unwrap_err()
            .starts_with("line 2: unknown element `bttn`"));
        assert!(parse("col\n  text hello").unwrap_err().contains("quotes"));
        assert!(parse("text \"x\"\n  text \"y\"")
            .unwrap_err()
            .contains("cannot have children"));
        assert!(parse("col\n  text \"x")
            .unwrap_err()
            .contains("closing quote"));
    }
}
