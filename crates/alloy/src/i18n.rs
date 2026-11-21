//! Lightweight internationalisation: translated text, plurals, locale switching and
//! right-to-left layout, with no message enums and no build step.
//!
//! Write catalogs as plain `key = value` text (one per language), register them once, and
//! look text up with [`t!`](crate::t):
//!
//! ```
//! use alloy_ui::i18n;
//! use alloy_ui::t;
//!
//! i18n::add("en", "hello = Hello, {name}!\nitems.one = {n} item\nitems.other = {n} items");
//! i18n::add("fr", "hello = Bonjour, {name} !\nitems.one = {n} élément\nitems.other = {n} éléments");
//!
//! i18n::set_locale("fr");
//! assert_eq!(t!("hello", name = "Léa"), "Bonjour, Léa !");
//! assert_eq!(t!("items", n = 3), "3 éléments");
//!
//! i18n::set_locale("de"); // no German catalog: falls back to English
//! assert_eq!(t!("items", n = 1), "1 item");
//! # i18n::set_locale("en");
//! ```
//!
//! * **Catalog format**: `key = value` per line, `#` starts a comment, `\n` is a line break,
//!   a trailing `\` continues the value on the next line. `{name}` is replaced by the
//!   argument of that name; `{{` and `}}` are literal braces.
//! * **Plurals**: give `key.one`, `key.other` (and `zero`, `two`, `few`, `many` where the
//!   language needs them) and pass the count as `n`. The category comes from the language's
//!   CLDR rule (English, French, Russian, Polish, Czech, Arabic, Hebrew, East Asian
//!   languages and more); an unknown language uses the English rule.
//! * **Fallback**: `pt-BR` → `pt` → the fallback locale (`en` unless changed) → the key itself,
//!   so a missing translation shows its key and logs one warning, never panics.
//! * **Switching**: [`set_locale`] takes effect on the next frame, with no restart, because the
//!   view is rebuilt after every message. [`language_picker`] is a ready-made switcher, and
//!   [`detect`] reads the system language from the environment.
//! * **Right-to-left**: for `ar`, `he`, `fa`, `ur` and friends, [`is_rtl`] is true and every
//!   `row!` mirrors (first child on the right) while columns align to the right edge, so a
//!   sidebar-and-content app flips with no code changes. Text itself is shaped bidirectionally
//!   by the text engine. Icons that point somewhere, table column order and the scrollbar side
//!   are not mirrored yet.
//!
//! Keep catalogs honest in tests with [`missing`], which lists the keys a language lacks.

use crate::{dsl::Act, view::View};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

#[derive(Default)]
struct Registry {
    catalogs: HashMap<String, HashMap<String, String>>,
    locale: String,
    fallback: String,
    warned: HashSet<String>,
}

static REG: LazyLock<Mutex<Registry>> = LazyLock::new(|| {
    Mutex::new(Registry {
        locale: "en".into(),
        fallback: "en".into(),
        ..Registry::default()
    })
});
static RTL: AtomicBool = AtomicBool::new(false);

fn reg() -> MutexGuard<'static, Registry> {
    REG.lock().unwrap_or_else(|e| e.into_inner())
}

/// `pt_BR.UTF-8` → `pt-br`: lower case, `-` separated, without encoding or modifier.
fn normalize(tag: &str) -> String {
    let tag = tag.split(['.', '@']).next().unwrap_or("");
    tag.replace('_', "-").to_ascii_lowercase()
}

/// `pt-br` → `["pt-br", "pt"]`.
fn chain(tag: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = normalize(tag);
    while !cur.is_empty() {
        out.push(cur.clone());
        match cur.rfind('-') {
            Some(i) => cur.truncate(i),
            None => break,
        }
    }
    out
}

fn language(tag: &str) -> String {
    normalize(tag).split('-').next().unwrap_or("").to_string()
}

/// Parse a catalog: `key = value` lines, `#` comments, `\n` and `\\` escapes, `\` at the end
/// of a line continues the value.
pub fn parse_catalog(source: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut lines = source.lines();
    while let Some(line) = lines.next() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            log::warn!("i18n: ignoring catalog line without `=`: {line:?}");
            continue;
        };
        let mut value = value.trim_start().trim_end().to_string();
        while value.ends_with('\\') && !value.ends_with("\\\\") {
            value.pop();
            match lines.next() {
                Some(next) => value.push_str(next.trim()),
                None => break,
            }
        }
        out.insert(key.trim().to_string(), unescape(&value));
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Register (or extend) the catalog for `locale`. Later calls overwrite keys of earlier ones.
pub fn add(locale: &str, catalog: &str) {
    let parsed = parse_catalog(catalog);
    reg()
        .catalogs
        .entry(normalize(locale))
        .or_default()
        .extend(parsed);
}

/// The language shown when the current locale lacks a key (default `en`).
pub fn set_fallback(locale: &str) {
    reg().fallback = normalize(locale);
}

/// Switch the active locale (`"fr"`, `"pt-BR"`, `"ar"`). Takes effect on the next frame.
pub fn set_locale(locale: &str) {
    let l = normalize(locale);
    RTL.store(is_rtl_tag(&l), Ordering::Relaxed);
    reg().locale = l;
}

/// The active locale, lower case (`"pt-br"`).
pub fn locale() -> String {
    reg().locale.clone()
}

/// The system's preferred language from `LC_ALL`, `LC_MESSAGES` or `LANG` (`"en"` if unset or
/// `C`). Pair with [`set_locale`] at startup: `i18n::set_locale(&i18n::detect())`.
pub fn detect() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            let n = normalize(&v);
            if !n.is_empty() && n != "c" && n != "posix" {
                return n;
            }
        }
    }
    "en".into()
}

/// Whether a locale tag is written right to left.
pub fn is_rtl_tag(tag: &str) -> bool {
    matches!(
        language(tag).as_str(),
        "ar" | "he" | "iw" | "fa" | "ur" | "ps" | "sd" | "ug" | "yi" | "dv" | "ku" | "ckb"
    )
}

/// Whether the active locale is right to left; `row!` mirrors while this is true.
pub fn is_rtl() -> bool {
    RTL.load(Ordering::Relaxed)
}

/// The CLDR plural category of `n` in `locale`: `zero`, `one`, `two`, `few`, `many` or `other`.
pub fn plural_category(locale: &str, n: f64) -> &'static str {
    let whole = n.fract() == 0.0;
    let i = n.abs().trunc() as u64;
    let (m10, m100) = (i % 10, i % 100);
    match language(locale).as_str() {
        "ja" | "zh" | "ko" | "vi" | "th" | "id" | "ms" | "lo" | "my" | "km" => "other",
        "fr" | "pt" | "hy" => {
            if n.abs() < 2.0 {
                "one"
            } else {
                "other"
            }
        }
        "ru" | "uk" | "be" if whole => match () {
            _ if m10 == 1 && m100 != 11 => "one",
            _ if (2..=4).contains(&m10) && !(12..=14).contains(&m100) => "few",
            _ => "many",
        },
        "ru" | "uk" | "be" => "other",
        "pl" if whole => match () {
            _ if i == 1 => "one",
            _ if (2..=4).contains(&m10) && !(12..=14).contains(&m100) => "few",
            _ => "many",
        },
        "pl" => "other",
        "cs" | "sk" if whole => match i {
            1 => "one",
            2..=4 => "few",
            _ => "other",
        },
        "ar" if whole => match () {
            _ if i == 0 => "zero",
            _ if i == 1 => "one",
            _ if i == 2 => "two",
            _ if (3..=10).contains(&m100) => "few",
            _ if (11..=99).contains(&m100) => "many",
            _ => "other",
        },
        "he" | "iw" if whole => match i {
            1 => "one",
            2 => "two",
            _ => "other",
        },
        _ => {
            if n == 1.0 {
                "one"
            } else {
                "other"
            }
        }
    }
}

fn find(r: &Registry, chain: &[String], key: &str) -> Option<String> {
    chain
        .iter()
        .find_map(|l| r.catalogs.get(l).and_then(|c| c.get(key)).cloned())
}

/// Look up `key` and fill in `args`. Prefer the [`t!`](crate::t) macro; call this directly for
/// keys computed at run time.
pub fn translate(key: &str, args: &[(&str, String)]) -> String {
    let mut r = reg();
    let mut chain = chain(&r.locale);
    chain.extend(self::chain(&r.fallback));
    let count = args
        .iter()
        .find(|(k, _)| *k == "n" || *k == "count")
        .and_then(|(_, v)| v.parse::<f64>().ok());
    let found = count
        .and_then(|n| {
            let cat = plural_category(&r.locale, n);
            find(&r, &chain, &format!("{key}.{cat}"))
                .or_else(|| find(&r, &chain, &format!("{key}.other")))
        })
        .or_else(|| find(&r, &chain, key));
    match found {
        Some(s) => fill(&s, args),
        None => {
            let tag = format!("{}:{key}", r.locale);
            if r.warned.insert(tag) {
                log::warn!("i18n: no translation for `{key}` in `{}`", r.locale);
            }
            key.to_string()
        }
    }
}

fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut it = template.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        match c {
            '{' if it.peek().is_some_and(|(_, n)| *n == '{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek().is_some_and(|(_, n)| *n == '}') => {
                it.next();
                out.push('}');
            }
            '{' => match template[i + 1..].find('}') {
                Some(end) => {
                    let name = template[i + 1..i + 1 + end].trim();
                    match args.iter().find(|(k, _)| *k == name) {
                        Some((_, v)) => out.push_str(v),
                        None => out.push_str(&template[i..i + end + 2]),
                    }
                    while it.peek().is_some_and(|(j, _)| *j <= i + end + 1) {
                        it.next();
                    }
                }
                None => out.push('{'),
            },
            c => out.push(c),
        }
    }
    out
}

/// Keys that `reference` has and `locale` lacks (plural keys compare by their base name,
/// since languages need different categories). Empty means the translation is complete.
/// Use it in a test: `assert!(i18n::missing("fr", "en").is_empty())`.
pub fn missing(locale: &str, reference: &str) -> Vec<String> {
    let r = reg();
    let base = |k: &str| match k.rsplit_once('.') {
        Some((b, "zero" | "one" | "two" | "few" | "many" | "other")) => b.to_string(),
        _ => k.to_string(),
    };
    let have: HashSet<String> = chain(locale)
        .iter()
        .filter_map(|l| r.catalogs.get(l))
        .flat_map(|c| c.keys().map(|k| base(k)))
        .collect();
    let mut out: Vec<String> = chain(reference)
        .iter()
        .filter_map(|l| r.catalogs.get(l))
        .flat_map(|c| c.keys().map(|k| base(k)))
        .filter(|k| !have.contains(k))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// A language switcher (a segmented control) listing `(tag, name)` pairs, which sets the
/// locale when picked: `language_picker(&[("en", "English"), ("ar", "العربية")])`.
pub fn language_picker<S: 'static>(langs: &[(&str, &str)]) -> impl View<Act<S>> {
    let cur = locale();
    let cur = langs
        .iter()
        .find(|(tag, _)| chain(&cur).contains(&normalize(tag)))
        .map_or(cur, |(tag, _)| tag.to_string());
    crate::choose(
        langs.iter().map(|(tag, name)| (*name, tag.to_string())),
        cur,
    )
    .act(|_: &mut S, tag: String| set_locale(&tag))
}

/// Translated text: `t!("save")`, `t!("hello", name = user)`, `t!("items", n = list.len())`.
///
/// The key is a string literal; arguments are `name = value` and fill `{name}` in the
/// catalog entry (see [`i18n`](crate::i18n)). Returns a `String`.
#[macro_export]
macro_rules! t {
    ($key:literal $(, $name:ident = $val:expr)* $(,)?) => {
        $crate::i18n::translate(
            $key,
            &[$((::std::stringify!($name), ::std::string::ToString::to_string(&$val))),*],
        )
    };
    ($key:literal $($rest:tt)+) => {
        ::std::compile_error!(
            "t! arguments are written `name = value`, for example t!(\"items\", n = 3); \
             they fill `{name}` in the catalog entry"
        )
    };
    ($($other:tt)*) => {
        ::std::compile_error!(
            "t! takes a string literal key, for example t!(\"save\"); for a key computed at \
             run time call `alloy_ui::i18n::translate(&key, &[])`"
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> MutexGuard<'static, ()> {
        static L: Mutex<()> = Mutex::new(());
        L.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn catalog_parsing() {
        let c = parse_catalog(
            "# c\n\na = 1\nb=two words\nc = line\\nbreak\nd = long \\\n   tail\ne = \\\\x\n",
        );
        assert_eq!(c["a"], "1");
        assert_eq!(c["b"], "two words");
        assert_eq!(c["c"], "line\nbreak");
        assert_eq!(c["d"], "long tail");
        assert_eq!(c["e"], "\\x");
    }

    #[test]
    fn interpolation() {
        let a = [("name", "Ann".to_string()), ("n", "3".to_string())];
        assert_eq!(
            fill("Hi {name}, {n} {{x}} {gone}", &a),
            "Hi Ann, 3 {x} {gone}"
        );
        assert_eq!(fill("{ name }!", &a), "Ann!");
        assert_eq!(fill("open { brace", &a), "open { brace");
        assert_eq!(fill("héllo {name} ✓", &a), "héllo Ann ✓");
    }

    #[test]
    fn plural_rules() {
        let p = plural_category;
        assert_eq!(
            (p("en", 1.0), p("en", 2.0), p("en", 0.0)),
            ("one", "other", "other")
        );
        assert_eq!(
            (p("fr", 0.0), p("fr", 1.0), p("fr", 2.0)),
            ("one", "one", "other")
        );
        assert_eq!(
            (
                p("ru", 1.0),
                p("ru", 3.0),
                p("ru", 5.0),
                p("ru", 11.0),
                p("ru", 21.0)
            ),
            ("one", "few", "many", "many", "one")
        );
        assert_eq!(
            (p("pl", 1.0), p("pl", 22.0), p("pl", 12.0)),
            ("one", "few", "many")
        );
        assert_eq!(
            (
                p("ar", 0.0),
                p("ar", 2.0),
                p("ar", 7.0),
                p("ar", 15.0),
                p("ar", 100.0)
            ),
            ("zero", "two", "few", "many", "other")
        );
        assert_eq!((p("ja", 1.0), p("zh-CN", 5.0)), ("other", "other"));
        assert_eq!(p("en", 1.5), "other");
    }

    #[test]
    fn lookup_fallback_and_plurals() {
        let _g = guard();
        add("en", "tst.hello = Hello {name}\ntst.only_en = English only\ntst.items.one = {n} item\ntst.items.other = {n} items");
        add(
            "pt",
            "tst.hello = Olá {name}\ntst.items.one = {n} item\ntst.items.other = {n} itens",
        );
        add("pt-br", "tst.hello = E aí {name}");
        add("ru", "tst.items.one = {n} штука\ntst.items.few = {n} штуки\ntst.items.many = {n} штук\ntst.items.other = {n} штуки");
        set_locale("pt_BR.UTF-8");
        assert_eq!(locale(), "pt-br");
        assert_eq!(t!("tst.hello", name = "Zé"), "E aí Zé");
        assert_eq!(t!("tst.items", n = 2), "2 itens");
        assert_eq!(t!("tst.only_en"), "English only");
        assert_eq!(t!("tst.nowhere"), "tst.nowhere");
        set_locale("ru");
        assert_eq!(t!("tst.items", n = 3), "3 штуки");
        assert_eq!(t!("tst.items", n = 5), "5 штук");
        set_locale("en");
        assert_eq!(t!("tst.items", n = 1), "1 item");
        assert!(missing("pt", "en").contains(&"tst.only_en".to_string()));
        assert!(!missing("pt", "en").contains(&"tst.items".to_string()));
    }

    #[test]
    fn direction_of_tags() {
        assert!(is_rtl_tag("ar") && is_rtl_tag("he-IL") && is_rtl_tag("fa_IR"));
        assert!(!is_rtl_tag("fr") && !is_rtl_tag("en-US"));
    }
}
