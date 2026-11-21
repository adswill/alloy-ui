//! A small, dependency-free syntax highlighter for [`CodeEditor`](crate::CodeEditor).
//!
//! It is a line-at-a-time lexer driven by a per-language table: keywords, comment
//! markers, string quotes. The only thing carried from one line to the next is a
//! [`State`] (inside a block comment, or a multi-line string), so the editor can
//! highlight any visible line after one cheap pass over the lines above it. It
//! colours; it does not parse. Good enough to read code by, and fast enough that a
//! 100 000-line file costs no more than a screenful.

/// A language the highlighter knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// No highlighting.
    #[default]
    Plain,
    /// Rust.
    Rust,
    /// Python.
    Python,
    /// JavaScript and TypeScript.
    JavaScript,
    /// JSON.
    Json,
    /// TOML.
    Toml,
    /// Shell scripts.
    Shell,
    /// C and C++.
    C,
    /// SQL.
    Sql,
    /// Go.
    Go,
    /// Java.
    Java,
    /// Kotlin.
    Kotlin,
    /// Swift.
    Swift,
    /// Ruby.
    Ruby,
    /// YAML.
    Yaml,
    /// CSS and SCSS.
    Css,
}

impl Lang {
    /// The language for a name or file extension (`"rs"`, `"py"`, `"main.rs"`, `"JSON"`).
    /// Unknown names give [`Lang::Plain`].
    pub fn from_name(name: &str) -> Lang {
        let ext = name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
        match ext.as_str() {
            "rs" | "rust" => Lang::Rust,
            "py" | "python" => Lang::Python,
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "javascript" | "typescript" => Lang::JavaScript,
            "json" => Lang::Json,
            "toml" => Lang::Toml,
            "sh" | "bash" | "zsh" | "shell" => Lang::Shell,
            "c" | "h" | "cc" | "cpp" | "hpp" | "cxx" => Lang::C,
            "sql" => Lang::Sql,
            "go" | "golang" => Lang::Go,
            "java" => Lang::Java,
            "kt" | "kts" | "kotlin" => Lang::Kotlin,
            "swift" => Lang::Swift,
            "rb" | "ruby" | "rake" | "gemspec" => Lang::Ruby,
            "yml" | "yaml" => Lang::Yaml,
            "css" | "scss" => Lang::Css,
            _ => Lang::Plain,
        }
    }
    /// The line-comment marker, used by the editor's comment toggle.
    pub fn line_comment(self) -> Option<&'static str> {
        self.spec().line_comment.first().copied()
    }
    fn spec(self) -> &'static Spec {
        match self {
            Lang::Plain => &PLAIN,
            Lang::Rust => &RUST,
            Lang::Python => &PYTHON,
            Lang::JavaScript => &JS,
            Lang::Json => &JSON,
            Lang::Toml => &TOML,
            Lang::Shell => &SHELL,
            Lang::C => &C,
            Lang::Sql => &SQL,
            Lang::Go => &GO,
            Lang::Java => &JAVA,
            Lang::Kotlin => &KOTLIN,
            Lang::Swift => &SWIFT,
            Lang::Ruby => &RUBY,
            Lang::Yaml => &YAML,
            Lang::Css => &CSS,
        }
    }
}

/// What a span of source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    /// Anything else.
    Plain,
    /// `fn`, `if`, `return`, `true`, `null`.
    Keyword,
    /// A capitalised name or a built-in type.
    Type,
    /// A name followed by `(`.
    Function,
    /// A string or character literal.
    String,
    /// A number.
    Number,
    /// A comment.
    Comment,
    /// Operators and brackets.
    Punct,
    /// An attribute, macro, decorator or lifetime.
    Attr,
}

/// A coloured byte range of one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
    /// What it is.
    pub kind: Token,
}

/// What a line leaves open for the next one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    /// Nothing open.
    #[default]
    Normal,
    /// Inside a `/* ... */` comment.
    Block,
    /// Inside a string that continues on the next line: its quote, and whether it is a
    /// triple quote.
    Str(char, bool),
}

struct Spec {
    keywords: &'static [&'static str],
    types: &'static [&'static str],
    line_comment: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    quotes: &'static [char],
    /// `'` starts a char literal or a lifetime (Rust).
    lifetimes: bool,
    /// `"""` and `'''` strings (Python).
    triple: bool,
    /// Backtick strings span lines (JavaScript).
    template: bool,
    /// `#[..]`, `@name`, `name!` are attributes and macros.
    attrs: &'static [char],
    macro_bang: bool,
    /// Keywords match case-insensitively (SQL).
    nocase: bool,
    /// Capitalised identifiers are types.
    caps_are_types: bool,
}

const NONE: Spec = Spec {
    keywords: &[],
    types: &[],
    line_comment: &[],
    block: None,
    quotes: &[],
    lifetimes: false,
    triple: false,
    template: false,
    attrs: &[],
    macro_bang: false,
    nocase: false,
    caps_are_types: false,
};
static PLAIN: Spec = NONE;
static RUST: Spec = Spec {
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while",
    ],
    types: &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64", "bool", "char", "str",
    ],
    line_comment: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"', '\''],
    lifetimes: true,
    attrs: &['#'],
    macro_bang: true,
    caps_are_types: true,
    ..NONE
};
static PYTHON: Spec = Spec {
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
        "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True",
        "try", "while", "with", "yield",
    ],
    types: &[
        "int", "str", "float", "bool", "list", "dict", "set", "tuple", "bytes",
    ],
    line_comment: &["#"],
    quotes: &['"', '\''],
    triple: true,
    attrs: &['@'],
    caps_are_types: true,
    ..NONE
};
static JS: Spec = Spec {
    keywords: &[
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "delete",
        "do",
        "else",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "from",
        "function",
        "if",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "null",
        "of",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "type",
        "typeof",
        "undefined",
        "var",
        "void",
        "while",
        "yield",
    ],
    types: &[
        "string", "number", "boolean", "any", "unknown", "never", "object",
    ],
    line_comment: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"', '\'', '`'],
    template: true,
    attrs: &['@'],
    caps_are_types: true,
    ..NONE
};
static JSON: Spec = Spec {
    keywords: &["true", "false", "null"],
    quotes: &['"'],
    ..NONE
};
static TOML: Spec = Spec {
    keywords: &["true", "false"],
    line_comment: &["#"],
    quotes: &['"', '\''],
    triple: true,
    ..NONE
};
static SHELL: Spec = Spec {
    keywords: &[
        "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in",
        "function", "return", "export", "local", "echo", "cd", "set", "unset", "source",
    ],
    line_comment: &["#"],
    quotes: &['"', '\''],
    ..NONE
};
static C: Spec = Spec {
    keywords: &[
        "auto",
        "break",
        "case",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "extern",
        "false",
        "for",
        "goto",
        "if",
        "inline",
        "namespace",
        "new",
        "nullptr",
        "private",
        "public",
        "return",
        "sizeof",
        "static",
        "struct",
        "switch",
        "template",
        "this",
        "true",
        "typedef",
        "union",
        "using",
        "virtual",
        "while",
    ],
    types: &[
        "int", "long", "short", "char", "float", "double", "void", "unsigned", "signed", "bool",
        "size_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int32_t", "int64_t",
    ],
    line_comment: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"', '\''],
    attrs: &['#'],
    caps_are_types: true,
    ..NONE
};
static SQL: Spec = Spec {
    keywords: &[
        "select", "from", "where", "and", "or", "not", "null", "insert", "into", "values",
        "update", "set", "delete", "create", "table", "drop", "alter", "index", "join", "left",
        "right", "inner", "outer", "on", "as", "group", "by", "order", "having", "limit", "offset",
        "distinct", "union", "all", "in", "is", "like", "between", "case", "when", "then", "else",
        "end", "primary", "key", "default", "unique", "asc", "desc", "with",
    ],
    types: &[
        "int",
        "integer",
        "text",
        "varchar",
        "real",
        "boolean",
        "date",
        "timestamp",
        "numeric",
    ],
    line_comment: &["--"],
    block: Some(("/*", "*/")),
    quotes: &['\''],
    nocase: true,
    ..NONE
};

static GO: Spec = Spec {
    keywords: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "false",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "nil",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "true",
        "type",
        "var",
    ],
    types: &[
        "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32", "uint64",
        "uintptr", "float32", "float64", "string", "bool", "byte", "rune", "error", "any",
    ],
    line_comment: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"', '\'', '`'],
    template: true,
    caps_are_types: true,
    ..NONE
};
static JAVA: Spec = Spec {
    keywords: &[
        "abstract",
        "assert",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "extends",
        "false",
        "final",
        "finally",
        "for",
        "if",
        "implements",
        "import",
        "instanceof",
        "interface",
        "native",
        "new",
        "null",
        "package",
        "private",
        "protected",
        "public",
        "record",
        "return",
        "static",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "true",
        "try",
        "var",
        "void",
        "volatile",
        "while",
    ],
    types: &[
        "int", "long", "short", "byte", "char", "float", "double", "boolean", "String", "Object",
    ],
    line_comment: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"', '\''],
    triple: true,
    attrs: &['@'],
    caps_are_types: true,
    ..NONE
};
static KOTLIN: Spec = Spec {
    keywords: &[
        "abstract",
        "as",
        "break",
        "by",
        "catch",
        "class",
        "companion",
        "const",
        "constructor",
        "continue",
        "data",
        "do",
        "else",
        "enum",
        "false",
        "finally",
        "for",
        "fun",
        "if",
        "import",
        "in",
        "init",
        "interface",
        "internal",
        "is",
        "lateinit",
        "null",
        "object",
        "open",
        "override",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "sealed",
        "super",
        "suspend",
        "this",
        "throw",
        "true",
        "try",
        "typealias",
        "val",
        "var",
        "when",
        "while",
    ],
    types: &[
        "Int", "Long", "Short", "Byte", "Char", "Float", "Double", "Boolean", "String", "Unit",
        "Any", "Nothing",
    ],
    line_comment: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"', '\''],
    triple: true,
    attrs: &['@'],
    caps_are_types: true,
    ..NONE
};
static SWIFT: Spec = Spec {
    keywords: &[
        "actor",
        "as",
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "continue",
        "default",
        "defer",
        "do",
        "else",
        "enum",
        "extension",
        "false",
        "fileprivate",
        "for",
        "func",
        "guard",
        "if",
        "import",
        "in",
        "init",
        "inout",
        "internal",
        "is",
        "let",
        "nil",
        "open",
        "operator",
        "override",
        "private",
        "protocol",
        "public",
        "repeat",
        "return",
        "self",
        "Self",
        "static",
        "struct",
        "subscript",
        "super",
        "switch",
        "throw",
        "throws",
        "true",
        "try",
        "typealias",
        "var",
        "where",
        "while",
    ],
    types: &[
        "Int",
        "UInt",
        "Double",
        "Float",
        "Bool",
        "String",
        "Character",
        "Array",
        "Dictionary",
        "Set",
        "Optional",
        "Any",
        "Void",
    ],
    line_comment: &["//"],
    block: Some(("/*", "*/")),
    quotes: &['"'],
    triple: true,
    attrs: &['@', '#'],
    caps_are_types: true,
    ..NONE
};
static RUBY: Spec = Spec {
    keywords: &[
        "and", "begin", "break", "case", "class", "def", "do", "else", "elsif", "end", "ensure",
        "false", "for", "if", "in", "include", "lambda", "module", "next", "nil", "not", "or",
        "proc", "puts", "raise", "require", "rescue", "retry", "return", "self", "super", "then",
        "true", "unless", "until", "when", "while", "yield",
    ],
    line_comment: &["#"],
    quotes: &['"', '\''],
    caps_are_types: true,
    ..NONE
};
static YAML: Spec = Spec {
    keywords: &["true", "false", "null", "yes", "no"],
    line_comment: &["#"],
    quotes: &['"', '\''],
    ..NONE
};
static CSS: Spec = Spec {
    keywords: &[
        "absolute",
        "auto",
        "block",
        "calc",
        "fixed",
        "flex",
        "grid",
        "important",
        "inherit",
        "initial",
        "inline",
        "none",
        "relative",
        "solid",
        "var",
    ],
    block: Some(("/*", "*/")),
    quotes: &['"', '\''],
    attrs: &['@'],
    ..NONE
};

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}
fn is_ident(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// Lex one line, given the state the previous line left. Returns the coloured spans (only
/// non-[`Token::Plain`] ones, in order) and the state for the next line.
pub fn tokenize(lang: Lang, line: &str, state: State) -> (Vec<Span>, State) {
    if lang == Lang::Plain {
        return (Vec::new(), State::Normal);
    }
    let sp = lang.spec();
    let mut out: Vec<Span> = Vec::new();
    let mut push = |start: usize, end: usize, kind: Token| {
        if end > start && kind != Token::Plain {
            match out.last_mut() {
                Some(l) if l.end == start && l.kind == kind => l.end = end,
                _ => out.push(Span { start, end, kind }),
            }
        }
    };
    let b = line.as_bytes();
    let n = line.len();
    let mut i = 0;
    let at = |i: usize, pat: &str| line[i..].starts_with(pat);

    // Resume whatever the previous line left open.
    match state {
        State::Normal => {}
        State::Block => {
            let end = sp.block.map_or("*/", |b| b.1);
            match line.find(end) {
                Some(p) => {
                    i = p + end.len();
                    push(0, i, Token::Comment);
                }
                None => {
                    push(0, n, Token::Comment);
                    return (out, State::Block);
                }
            }
        }
        State::Str(q, triple) => match string_end(line, 0, q, triple) {
            Some(e) => {
                push(0, e, Token::String);
                i = e;
            }
            None => {
                push(0, n, Token::String);
                // A `"` string ends at the line unless it is a triple or template string.
                let keeps = triple || q == '`';
                return (
                    out,
                    if keeps {
                        State::Str(q, triple)
                    } else {
                        State::Normal
                    },
                );
            }
        },
    }

    while i < n {
        let c = line[i..].chars().next().unwrap_or(' ');
        let cl = c.len_utf8();
        // Comments.
        if sp.line_comment.iter().any(|m| at(i, m)) {
            push(i, n, Token::Comment);
            return (out, State::Normal);
        }
        if let Some((open, close)) = sp.block {
            if at(i, open) {
                match line[i + open.len()..].find(close) {
                    Some(p) => {
                        let e = i + open.len() + p + close.len();
                        push(i, e, Token::Comment);
                        i = e;
                        continue;
                    }
                    None => {
                        push(i, n, Token::Comment);
                        return (out, State::Block);
                    }
                }
            }
        }
        // Attributes: `#[derive]`, `@decorator`, `#include`.
        if sp.attrs.contains(&c) && (i == 0 || line[..i].trim().is_empty() || c == '@') {
            let rest = &line[i + cl..];
            let ok = match c {
                '#' if sp.lifetimes => rest.starts_with('[') || rest.starts_with("!["),
                '#' => rest.starts_with(|x: char| x.is_alphabetic()),
                _ => rest.starts_with(is_ident_start),
            };
            if ok {
                let mut e = i + cl;
                if c == '#' && sp.lifetimes {
                    e = line[i..].find(']').map_or(n, |p| i + p + 1);
                } else {
                    while e < n {
                        let ch = line[e..].chars().next().unwrap_or(' ');
                        if !(is_ident(ch) || ch == '.') {
                            break;
                        }
                        e += ch.len_utf8();
                    }
                }
                push(i, e, Token::Attr);
                i = e;
                continue;
            }
        }
        // Strings.
        if sp.quotes.contains(&c) {
            if c == '\'' && sp.lifetimes {
                // 'a' or '\n' is a char; 'a is a lifetime.
                let rest = &line[i + 1..];
                let mut it = rest.chars();
                let first = it.next();
                let is_char = match first {
                    Some('\\') => true,
                    Some(_) => it.next() == Some('\''),
                    None => false,
                };
                if !is_char {
                    let mut e = i + 1;
                    while e < n && is_ident(line[e..].chars().next().unwrap_or(' ')) {
                        e += line[e..].chars().next().map_or(1, char::len_utf8);
                    }
                    push(i, e.max(i + 1), Token::Attr);
                    i = e.max(i + 1);
                    continue;
                }
            }
            let triple = sp.triple && line[i..].starts_with(&c.to_string().repeat(3));
            let open = if triple { 3 } else { 1 };
            match string_end(line, i + open, c, triple) {
                Some(e) => {
                    push(i, e, Token::String);
                    i = e;
                }
                None => {
                    push(i, n, Token::String);
                    let keeps = triple || (c == '`' && sp.template);
                    return (
                        out,
                        if keeps {
                            State::Str(c, triple)
                        } else {
                            State::Normal
                        },
                    );
                }
            }
            continue;
        }
        // Numbers (not the tail of an identifier).
        let prev_ident = i > 0 && line[..i].chars().next_back().is_some_and(is_ident);
        if c.is_ascii_digit() && !prev_ident {
            let mut e = i;
            while e < n && (b[e].is_ascii_alphanumeric() || b[e] == b'_' || b[e] == b'.') {
                // `1..2` is a range, not a number with two dots.
                if b[e] == b'.' && (e + 1 >= n || !b[e + 1].is_ascii_digit()) {
                    break;
                }
                e += 1;
            }
            push(i, e, Token::Number);
            i = e;
            continue;
        }
        // Words.
        if is_ident_start(c) {
            let mut e = i;
            while e < n {
                let ch = line[e..].chars().next().unwrap_or(' ');
                if !is_ident(ch) {
                    break;
                }
                e += ch.len_utf8();
            }
            let word = &line[i..e];
            let eq = |w: &&str| {
                if sp.nocase {
                    w.eq_ignore_ascii_case(word)
                } else {
                    *w == word
                }
            };
            let next = line[e..].chars().next();
            let kind = if sp.keywords.iter().any(eq) {
                Token::Keyword
            } else if sp.types.iter().any(eq)
                || (sp.caps_are_types
                    && word.chars().next().is_some_and(char::is_uppercase)
                    && word.chars().any(char::is_lowercase))
            {
                Token::Type
            } else if sp.macro_bang && next == Some('!') && !line[e..].starts_with("!=") {
                e += 1;
                Token::Attr
            } else if next == Some('(') && !sp.keywords.is_empty() {
                Token::Function
            } else {
                Token::Plain
            };
            push(i, e, kind);
            i = e;
            continue;
        }
        // Punctuation.
        if !sp.keywords.is_empty() || !sp.quotes.is_empty() {
            let kind = if "{}()[]<>=+-*/%&|^!~?:;,.@".contains(c) {
                Token::Punct
            } else {
                Token::Plain
            };
            push(i, i + cl, kind);
        }
        i += cl;
    }
    (out, State::Normal)
}

/// Byte index just past the closing quote of a string whose body starts at `from`.
fn string_end(line: &str, from: usize, q: char, triple: bool) -> Option<usize> {
    let mut esc = false;
    let close = q.to_string().repeat(if triple { 3 } else { 1 });
    let mut i = from;
    while i < line.len() {
        let c = line[i..].chars().next()?;
        if esc {
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if line[i..].starts_with(&close) {
            return Some(i + close.len());
        }
        i += c.len_utf8();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(lang: Lang, line: &str) -> Vec<(&str, Token)> {
        tokenize(lang, line, State::Normal)
            .0
            .iter()
            .map(|s| (&line[s.start..s.end], s.kind))
            .collect()
    }

    #[test]
    fn rust_basics() {
        let k = kinds(Lang::Rust, "pub fn add(a: u32) -> Vec<T> { 42 } // hi");
        assert!(k.contains(&("pub", Token::Keyword)));
        assert!(k.contains(&("add", Token::Function)));
        assert!(k.contains(&("u32", Token::Type)));
        assert!(k.contains(&("Vec", Token::Type)));
        assert!(k.contains(&("42", Token::Number)));
        assert_eq!(*k.last().unwrap(), ("// hi", Token::Comment));
    }
    #[test]
    fn rust_lifetime_vs_char_and_macros() {
        let k = kinds(
            Lang::Rust,
            "fn f<'a>(c: &'a str) { let x = 'z'; println!(\"hi\"); }",
        );
        assert!(k.contains(&("'a", Token::Attr)));
        assert!(k.contains(&("'z'", Token::String)));
        assert!(k.contains(&("println!", Token::Attr)));
        assert!(k.contains(&("\"hi\"", Token::String)));
    }
    #[test]
    fn block_comments_carry_over() {
        let (s1, st) = tokenize(Lang::Rust, "let a = 1; /* start", State::Normal);
        assert_eq!(st, State::Block);
        assert_eq!(s1.last().unwrap().kind, Token::Comment);
        let (s2, st) = tokenize(Lang::Rust, "still */ let b", st);
        assert_eq!(st, State::Normal);
        assert_eq!(
            s2[0],
            Span {
                start: 0,
                end: 8,
                kind: Token::Comment
            }
        );
    }
    #[test]
    fn python_triple_strings_carry_over() {
        let (_, st) = tokenize(Lang::Python, "x = \"\"\"doc", State::Normal);
        assert_eq!(st, State::Str('"', true));
        let (s, st) = tokenize(Lang::Python, "end\"\"\" + y", st);
        assert_eq!(st, State::Normal);
        assert_eq!(s[0].end, 6);
    }
    #[test]
    fn newer_languages_colour_their_keywords_and_comments() {
        assert!(
            kinds(Lang::Go, "func main() { go run() } // go").contains(&("func", Token::Keyword))
        );
        assert!(kinds(Lang::Java, "public class A { }").contains(&("class", Token::Keyword)));
        assert!(kinds(Lang::Kotlin, "fun f(): Int = 1").contains(&("Int", Token::Type)));
        assert!(kinds(Lang::Swift, "guard let x = y else { return }")
            .contains(&("guard", Token::Keyword)));
        assert!(kinds(Lang::Ruby, "def hi # note\nend").contains(&("def", Token::Keyword)));
        assert!(kinds(Lang::Yaml, "on: true # yes")
            .iter()
            .any(|k| k.1 == Token::Comment));
        assert!(
            kinds(Lang::Css, "a { display: flex; } /* x */").contains(&("flex", Token::Keyword))
        );
        for (name, lang) in [
            ("main.go", Lang::Go),
            ("A.java", Lang::Java),
            ("a.kt", Lang::Kotlin),
            ("a.swift", Lang::Swift),
            ("a.rb", Lang::Ruby),
            ("ci.yml", Lang::Yaml),
            ("app.scss", Lang::Css),
        ] {
            assert_eq!(Lang::from_name(name), lang, "{name}");
        }
    }

    #[test]
    fn plain_text_has_no_spans_and_unterminated_quote_resets() {
        assert!(tokenize(Lang::Plain, "fn main() { 1 }", State::Normal)
            .0
            .is_empty());
        assert_eq!(
            tokenize(Lang::Rust, "let s = \"oops", State::Normal).1,
            State::Normal
        );
    }
    #[test]
    fn numbers_ranges_and_identifier_tails() {
        let k = kinds(Lang::Rust, "for i in 0..10 { x1 }");
        assert!(k.contains(&("0", Token::Number)) && k.contains(&("10", Token::Number)));
        assert!(
            !k.iter().any(|(t, _)| *t == "1"),
            "digit inside x1 is not a number"
        );
    }
    #[test]
    fn sql_is_case_insensitive_and_names_map() {
        assert!(kinds(Lang::Sql, "SELECT id FROM t -- c").contains(&("SELECT", Token::Keyword)));
        assert_eq!(Lang::from_name("main.RS"), Lang::Rust);
        assert_eq!(Lang::from_name("whatever"), Lang::Plain);
    }
}
