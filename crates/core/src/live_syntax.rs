//! The `.alloy` live-view markup: grammar tables, parser and diagnostics.
//!
//! Shared by the runtime (`alloy-ui`'s `live!`) and by tooling (`alloy lsp`, the VS Code
//! extension), so the editor and the app can never disagree about what a file means.
//!
//! ```text
//! theme dark accent=#6D4AFF
//! col page gap=14
//!   heading "Hello, {name}"
//!   button "Add" primary @add
//! ```

/// One parsed line and its indented children.
#[derive(Clone, Debug, Default)]
pub struct Item {
    /// Element name: `col`, `button`, ...
    pub kind: String,
    /// The `"quoted text"`, if any.
    pub text: Option<String>,
    /// `name=value` settings in order.
    pub attrs: Vec<(String, String)>,
    /// Bare flags such as `primary`.
    pub flags: Vec<String>,
    /// The `@action` a button is wired to.
    pub action: Option<String>,
    /// Indented lines under this one.
    pub kids: Vec<Item>,
}

impl Item {
    /// A numeric setting such as `gap=8`.
    pub fn attr(&self, k: &str) -> Option<f32> {
        self.attrs
            .iter()
            .find(|(n, _)| n == k)
            .and_then(|(_, v)| v.parse().ok())
    }
    /// Whether the bare flag `f` is present.
    pub fn flag(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }
}

/// What the editor shows about one element.
pub struct ElementDoc {
    /// The name as written in a file.
    pub name: &'static str,
    /// One-line description.
    pub doc: &'static str,
    /// Settings that make sense here (`gap=`, `w=`, ...), without the `=`.
    pub attrs: &'static [&'static str],
    /// Flags that make sense here.
    pub flags: &'static [&'static str],
    /// Whether indented lines may follow.
    pub children: bool,
    /// Whether the element takes `"text"`.
    pub text: bool,
    /// Whether the element can be wired with `@action`.
    pub action: bool,
}

/// Every element, with what it accepts. The order is the order completion offers them.
pub const ELEMENTS: &[ElementDoc] = &[
    ElementDoc {
        name: "col",
        doc: "A vertical stack. `page` makes it the padded page root.",
        attrs: &["gap", "pad", "w", "h"],
        flags: &["page", "fill", "center"],
        children: true,
        text: false,
        action: false,
    },
    ElementDoc {
        name: "row",
        doc: "A horizontal row.",
        attrs: &["gap", "pad", "w", "h"],
        flags: &["fill", "center"],
        children: true,
        text: false,
        action: false,
    },
    ElementDoc {
        name: "heading",
        doc: "Large title text.",
        attrs: &[],
        flags: &["dim", "danger", "success", "accent"],
        children: false,
        text: true,
        action: false,
    },
    ElementDoc {
        name: "text",
        doc: "Body text. `{name}` is filled from `.var(\"name\", ..)` in Rust.",
        attrs: &[],
        flags: &["dim", "danger", "success", "accent"],
        children: false,
        text: true,
        action: false,
    },
    ElementDoc {
        name: "caption",
        doc: "Small secondary text.",
        attrs: &[],
        flags: &["dim", "danger", "success", "accent"],
        children: false,
        text: true,
        action: false,
    },
    ElementDoc {
        name: "button",
        doc: "A button. `@name` wires it to `.on(\"name\", ..)`; without a handler it is disabled.",
        attrs: &[],
        flags: &["primary", "ghost", "fill"],
        children: false,
        text: true,
        action: true,
    },
    ElementDoc {
        name: "rule",
        doc: "A thin dividing line.",
        attrs: &[],
        flags: &[],
        children: false,
        text: false,
        action: false,
    },
    ElementDoc {
        name: "space",
        doc: "Flexible empty space that pushes its siblings apart.",
        attrs: &[],
        flags: &[],
        children: false,
        text: false,
        action: false,
    },
    ElementDoc {
        name: "vspace",
        doc: "Fixed vertical gap (`h=`).",
        attrs: &["h"],
        flags: &[],
        children: false,
        text: false,
        action: false,
    },
    ElementDoc {
        name: "gap",
        doc: "Fixed empty space (`w=` and `h=`).",
        attrs: &["w", "h"],
        flags: &[],
        children: false,
        text: false,
        action: false,
    },
    ElementDoc {
        name: "theme",
        doc: "First line only: `dark` or `light`, `accent=#rrggbb`, `density=compact|spacious`, `contrast=high`.",
        attrs: &["accent", "density", "contrast"],
        flags: &["dark", "light"],
        children: false,
        text: false,
        action: false,
    },
];

/// Look an element up by name.
pub fn element(name: &str) -> Option<&'static ElementDoc> {
    ELEMENTS.iter().find(|e| e.name == name)
}

/// Every element name.
pub fn kinds() -> impl Iterator<Item = &'static str> {
    ELEMENTS.iter().map(|e| e.name)
}

const ATTRS: &[&str] = &["gap", "pad", "w", "h", "accent", "density", "contrast"];
const FLAGS: &[&str] = &[
    "page", "fill", "center", "dim", "danger", "success", "accent", "primary", "ghost", "dark",
    "light",
];

/// How serious a [`Diag`] is. Only errors stop a file from loading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The runtime refuses the file.
    Error,
    /// Loads, but the line probably does not do what was meant.
    Warning,
}

/// A problem in a file, located by line and character columns.
#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    /// Zero-based line.
    pub line: usize,
    /// First character column (zero-based) of the offending text.
    pub start: usize,
    /// One past the last character column.
    pub end: usize,
    /// What is wrong, as the runtime reports it (after `line N: `).
    pub message: String,
    /// Error or warning.
    pub severity: Severity,
}

/// A word on a line: its text, whether it was `"quoted"`, and its character columns.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// The word, without quotes for quoted text.
    pub text: String,
    /// True for `"quoted text"`.
    pub quoted: bool,
    /// First character column in the line.
    pub start: usize,
    /// One past the last character column (the closing quote is included).
    pub end: usize,
}

/// Split one line into words and quoted strings. `Err` carries the column where an unclosed
/// quote began.
pub fn tokens(line: &str) -> Result<Vec<Token>, usize> {
    let mut out = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let start = i;
            i += 1;
            let mut t = String::new();
            loop {
                match chars.get(i) {
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some(&c) => {
                        t.push(c);
                        i += 1;
                    }
                    None => return Err(start),
                }
            }
            out.push(Token {
                text: t,
                quoted: true,
                start,
                end: i,
            });
        } else {
            let start = i;
            let mut t = String::new();
            while i < chars.len() && !chars[i].is_whitespace() {
                t.push(chars[i]);
                i += 1;
            }
            out.push(Token {
                text: t,
                quoted: false,
                start,
                end: i,
            });
        }
    }
    Ok(out)
}

/// Parse a file, collecting every problem instead of stopping at the first. The tree holds the
/// lines that were understood; use it only when no [`Severity::Error`] is present.
pub fn parse_all(src: &str) -> (Item, Vec<Diag>) {
    let mut diags: Vec<Diag> = Vec::new();
    // (indent, item) stack; the root collects top-level items.
    let mut stack: Vec<(usize, Item)> = vec![(
        0,
        Item {
            kind: "col".into(),
            ..Item::default()
        },
    )];
    let mut base: Option<usize> = None;
    for (i, line) in src.lines().enumerate() {
        let mut err = |start: usize, end: usize, m: String| {
            diags.push(Diag {
                line: i,
                start,
                end,
                message: m,
                severity: Severity::Error,
            })
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let width = line.chars().count();
        if line.starts_with('\t') {
            err(0, 1, "use spaces to indent, not tabs".into());
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let base = *base.get_or_insert(indent);
        if indent < base {
            err(0, indent, "indented less than the first line".into());
            continue;
        }
        let toks = match tokens(trimmed) {
            Ok(t) => t,
            Err(col) => {
                err(
                    indent + col,
                    width,
                    "text is missing its closing quote".into(),
                );
                continue;
            }
        };
        let shift = |t: &Token| (indent + t.start, indent + t.end);
        let kind = &toks[0];
        let Some(doc) = element(&kind.text) else {
            let (s, e) = shift(kind);
            err(
                s,
                e,
                format!(
                    "unknown element `{}` (try one of: {})",
                    kind.text,
                    kinds().collect::<Vec<_>>().join(", ")
                ),
            );
            continue;
        };
        let mut item = Item {
            kind: kind.text.clone(),
            ..Item::default()
        };
        let mut bad = false;
        let mut warnings: Vec<(usize, usize, String)> = Vec::new();
        for t in &toks[1..] {
            let (s, e) = shift(t);
            if t.quoted {
                if !doc.text {
                    warnings.push((s, e, format!("`{}` does not show text", doc.name)));
                }
                item.text = Some(t.text.clone());
            } else if let Some(a) = t.text.strip_prefix('@') {
                if !doc.action {
                    warnings.push((s, e, format!("`{}` has no action to wire", doc.name)));
                }
                item.action = Some(a.to_string());
            } else if let Some((k, v)) = t.text.split_once('=') {
                if !ATTRS.contains(&k) {
                    err(s, e, format!("unknown setting `{k}=`"));
                    bad = true;
                    continue;
                }
                if !doc.attrs.contains(&k) {
                    warnings.push((s, e, format!("`{k}=` has no effect on `{}`", doc.name)));
                }
                item.attrs.push((k.into(), v.into()));
            } else if FLAGS.contains(&t.text.as_str()) {
                if !doc.flags.contains(&t.text.as_str()) {
                    warnings.push((
                        s,
                        e,
                        format!("`{}` has no effect on `{}`", t.text, doc.name),
                    ));
                }
                item.flags.push(t.text.clone());
            } else {
                err(
                    s,
                    e,
                    format!(
                        "`{}` is not a flag; put text in \"quotes\" (flags: {})",
                        t.text,
                        FLAGS.join(", ")
                    ),
                );
                bad = true;
            }
        }
        // Warnings only make sense when the line otherwise loads.
        for (s, e, m) in warnings {
            diags.push(Diag {
                line: i,
                start: s,
                end: e,
                message: m,
                severity: Severity::Warning,
            });
        }
        if bad {
            continue;
        }
        // Close finished siblings, then attach under the nearest shallower parent.
        while stack.len() > 1 && stack.last().is_some_and(|(d, _)| *d >= indent) {
            let (_, done) = stack.pop().expect("len > 1");
            stack.last_mut().expect("root").1.kids.push(done);
        }
        let parent = &stack.last().expect("root").1;
        if !matches!(parent.kind.as_str(), "col" | "row") {
            diags.push(Diag {
                line: i,
                start: indent,
                end: width,
                message: format!("`{}` cannot have children", parent.kind),
                severity: Severity::Error,
            });
            continue;
        }
        stack.push((indent, item));
    }
    while stack.len() > 1 {
        let (_, done) = stack.pop().expect("len > 1");
        stack.last_mut().expect("root").1.kids.push(done);
    }
    let root = stack.pop().expect("root").1;
    let tree = if root.kids.len() == 1 && root.kids[0].kind != "theme" {
        root.kids.into_iter().next().expect("one")
    } else {
        root
    };
    (tree, diags)
}

/// Parse a file the way the runtime does: the first error becomes `line N: message`.
pub fn parse(src: &str) -> Result<Item, String> {
    let (tree, diags) = parse_all(src);
    match diags.iter().find(|d| d.severity == Severity::Error) {
        Some(d) => Err(format!("line {}: {}", d.line + 1, d.message)),
        None => Ok(tree),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_file_parses() {
        let t = parse("col page gap=14\n  heading \"Hi {name}\"\n  button \"Add\" primary @add\n")
            .unwrap();
        assert_eq!(t.kind, "col");
        assert_eq!(t.attr("gap"), Some(14.0));
        assert_eq!(t.kids[1].action.as_deref(), Some("add"));
        assert!(t.kids[1].flag("primary"));
    }

    #[test]
    fn runtime_messages_are_unchanged() {
        assert!(parse("bttn \"x\"")
            .unwrap_err()
            .starts_with("line 1: unknown element `bttn` (try one of: col, row"));
        assert_eq!(
            parse("col\n\ttext").unwrap_err(),
            "line 2: use spaces to indent, not tabs"
        );
        assert_eq!(
            parse("text \"oops").unwrap_err(),
            "line 1: text is missing its closing quote"
        );
        assert_eq!(
            parse("col foo=1").unwrap_err(),
            "line 1: unknown setting `foo=`"
        );
        assert!(parse("text hello").unwrap_err().contains("is not a flag"));
        assert_eq!(
            parse("text \"a\"\n  text \"b\"").unwrap_err(),
            "line 2: `text` cannot have children"
        );
    }

    #[test]
    fn collects_every_error_with_columns() {
        let (_, d) = parse_all("col\n  bttn \"a\"\n  text \"ok\"\n  text nope\n");
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].line, d[0].start, d[0].end), (1, 2, 6));
        assert_eq!((d[1].line, d[1].start, d[1].end), (3, 7, 11));
    }

    #[test]
    fn warns_about_flags_that_do_nothing_but_still_loads() {
        let (t, d) = parse_all("col\n  text \"a\" primary\n  rule \"x\"\n");
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|d| d.severity == Severity::Warning));
        assert!(parse("col\n  text \"a\" primary\n").is_ok());
        assert_eq!(t.kids.len(), 2);
    }

    #[test]
    fn tokens_count_characters_not_bytes() {
        let t = tokens("text \"héllo wörld\" dim").unwrap();
        assert_eq!((t[1].start, t[1].end), (5, 18));
        assert_eq!(t[2].start, 19);
        assert_eq!(tokens("x \"open"), Err(2));
    }
}
