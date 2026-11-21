//! `alloy lsp`: a language server for `.alloy` live views, over stdio.
//!
//! It speaks just enough of the Language Server Protocol for editors (the VS Code extension in
//! `editors/vscode` starts it): full-text sync, diagnostics, completion and hover. The grammar
//! comes from `alloy_ui_core::live_syntax`, the very parser the running app uses, so a squiggle
//! in the editor is exactly the error the app would show.

use alloy_ui_core::live_syntax::{self, Diag, Severity};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::io::{self, BufRead, Write};

/// The state of one editor connection.
#[derive(Default)]
pub struct Server {
    docs: HashMap<String, String>,
    shutdown: bool,
}

/// What [`Server::handle`] wants done after a message.
pub struct Reply {
    /// Messages to send back (responses and notifications), in order.
    pub out: Vec<Value>,
    /// True after `exit`: stop reading.
    pub exit: bool,
}

impl Server {
    /// A server with no open documents.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process one incoming JSON-RPC message.
    pub fn handle(&mut self, msg: &Value) -> Reply {
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let mut out = Vec::new();
        let mut exit = false;
        let respond = |id: Option<Value>, result: Value| {
            id.map(|id| json!({"jsonrpc":"2.0","id":id,"result":result}))
        };
        match method {
            "initialize" => out.extend(respond(
                id,
                json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "completionProvider": {"triggerCharacters": [" ", "@", "{", "="]},
                        "hoverProvider": true
                    },
                    "serverInfo": {"name": "alloy-lsp", "version": env!("CARGO_PKG_VERSION")}
                }),
            )),
            "shutdown" => {
                self.shutdown = true;
                out.extend(respond(id, Value::Null));
            }
            "exit" => exit = true,
            "textDocument/didOpen" => {
                let d = &params["textDocument"];
                if let (Some(uri), Some(text)) = (d["uri"].as_str(), d["text"].as_str()) {
                    self.docs.insert(uri.into(), text.into());
                    out.push(self.diagnostics(uri));
                }
            }
            "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"].as_str();
                let text = params["contentChanges"]
                    .as_array()
                    .and_then(|c| c.last())
                    .and_then(|c| c["text"].as_str());
                if let (Some(uri), Some(text)) = (uri, text) {
                    self.docs.insert(uri.into(), text.into());
                    out.push(self.diagnostics(uri));
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = params["textDocument"]["uri"].as_str() {
                    self.docs.remove(uri);
                    out.push(publish(uri, vec![]));
                }
            }
            "textDocument/completion" => {
                let items = self
                    .position(&params)
                    .map_or(vec![], |(src, l, c)| complete(src, l, c));
                out.extend(respond(id, json!(items)));
            }
            "textDocument/hover" => {
                let hover = self
                    .position(&params)
                    .and_then(|(src, l, c)| hover(src, l, c))
                    .map_or(
                        Value::Null,
                        |md| json!({"contents": {"kind": "markdown", "value": md}}),
                    );
                out.extend(respond(id, hover));
            }
            _ => {
                // An unknown request needs an answer; an unknown notification is ignored.
                if let Some(id) = id {
                    out.push(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":format!("method not found: {method}")}}));
                }
            }
        }
        Reply { out, exit }
    }

    /// Whether the client asked to shut down before `exit` (the exit code is 0 then).
    pub fn was_shut_down(&self) -> bool {
        self.shutdown
    }

    fn position(&self, params: &Value) -> Option<(&str, usize, usize)> {
        let src = self.docs.get(params["textDocument"]["uri"].as_str()?)?;
        let line = params["position"]["line"].as_u64()? as usize;
        let utf16 = params["position"]["character"].as_u64()? as usize;
        let text = src.lines().nth(line).unwrap_or("");
        Some((src, line, char_of_utf16(text, utf16)))
    }

    fn diagnostics(&self, uri: &str) -> Value {
        let src = &self.docs[uri];
        let lines: Vec<&str> = src.lines().collect();
        let (_, diags) = live_syntax::parse_all(src);
        publish(uri, diags.iter().map(|d| lsp_diag(d, &lines)).collect())
    }
}

fn publish(uri: &str, diagnostics: Vec<Value>) -> Value {
    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":diagnostics}})
}

fn lsp_diag(d: &Diag, lines: &[&str]) -> Value {
    let line = lines.get(d.line).copied().unwrap_or("");
    json!({
        "range": {
            "start": {"line": d.line, "character": utf16_of_char(line, d.start)},
            "end": {"line": d.line, "character": utf16_of_char(line, d.end)}
        },
        "severity": if d.severity == Severity::Error { 1 } else { 2 },
        "source": "alloy",
        "message": d.message
    })
}

/// LSP columns count UTF-16 code units; the parser counts characters.
fn utf16_of_char(line: &str, col: usize) -> usize {
    line.chars().take(col).map(char::len_utf16).sum()
}

fn char_of_utf16(line: &str, units: usize) -> usize {
    let mut seen = 0;
    for (i, c) in line.chars().enumerate() {
        if seen >= units {
            return i;
        }
        seen += c.len_utf16();
    }
    line.chars().count()
}

/// Names written as `{name}` anywhere in the file.
fn variables(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = src;
    while let Some(i) = rest.find('{') {
        rest = &rest[i + 1..];
        if let Some(j) = rest.find('}') {
            let name = &rest[..j];
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                out.insert(name.to_string());
            }
        }
    }
    out
}

/// Names written as `@name` anywhere in the file.
fn actions(src: &str) -> BTreeSet<String> {
    src.lines()
        .flat_map(|l| live_syntax::tokens(l).unwrap_or_default())
        .filter(|t| !t.quoted)
        .filter_map(|t| t.text.strip_prefix('@').map(str::to_string))
        .filter(|a| !a.is_empty())
        .collect()
}

fn item(label: &str, kind: u8, detail: &str, insert: Option<&str>) -> Value {
    let mut v = json!({"label": label, "kind": kind, "detail": detail});
    if let Some(i) = insert {
        v["insertText"] = json!(i);
    }
    v
}

// LSP CompletionItemKind numbers.
const KIND_PROPERTY: u8 = 10;
const KIND_VALUE: u8 = 12;
const KIND_KEYWORD: u8 = 14;
const KIND_VARIABLE: u8 = 6;
const KIND_CLASS: u8 = 7;

/// Completion items for the cursor at character `col` of `line`.
pub fn complete(src: &str, line: usize, col: usize) -> Vec<Value> {
    let text = src.lines().nth(line).unwrap_or("");
    let before: String = text.chars().take(col).collect();
    // Inside an open "quoted text": offer `{variables}` after a `{`.
    if before.matches('"').count() % 2 == 1 {
        if before.ends_with('{') {
            return variables(src)
                .into_iter()
                .map(|v| {
                    item(
                        &v,
                        KIND_VARIABLE,
                        "variable in this file",
                        Some(&format!("{v}}}")),
                    )
                })
                .collect();
        }
        return vec![];
    }
    let toks = live_syntax::tokens(&before).unwrap_or_default();
    let typing_word = !before.is_empty() && !before.ends_with(char::is_whitespace);
    // The element name itself.
    if toks.len() <= 1 && (toks.is_empty() || typing_word) {
        let first_content_line = !src
            .lines()
            .take(line)
            .any(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
        return live_syntax::ELEMENTS
            .iter()
            .filter(|e| e.name != "theme" || first_content_line)
            .map(|e| item(e.name, KIND_CLASS, e.doc, None))
            .collect();
    }
    let Some(doc) = live_syntax::element(&toks[0].text) else {
        return vec![];
    };
    let current = if typing_word {
        toks.last().map(|t| t.text.as_str())
    } else {
        None
    };
    let used: Vec<&str> = toks[1..toks.len() - usize::from(typing_word)]
        .iter()
        .map(|t| t.text.as_str())
        .collect();
    // `@action`: reuse the names already in the file.
    if let Some(partial) = current.filter(|c| c.starts_with('@')) {
        let _ = partial;
        return actions(src)
            .into_iter()
            .map(|a| {
                item(
                    &format!("@{a}"),
                    KIND_VALUE,
                    "action used elsewhere in this file",
                    None,
                )
            })
            .collect();
    }
    // Values after `density=` / `contrast=`.
    if let Some(c) = current {
        let values: &[&str] = match c.split_once('=').map(|(k, _)| k) {
            Some("density") => &["compact", "spacious"],
            Some("contrast") => &["high"],
            _ => &[],
        };
        if !values.is_empty() {
            return values
                .iter()
                .map(|v| item(v, KIND_VALUE, "value", None))
                .collect();
        }
    }
    let has = |name: &str| {
        used.iter()
            .any(|u| *u == name || u.split_once('=').is_some_and(|(k, _)| k == name))
    };
    let mut out = Vec::new();
    for a in doc.attrs.iter().filter(|a| !has(a)) {
        out.push(item(
            &format!("{a}="),
            KIND_PROPERTY,
            &format!("setting of `{}`", doc.name),
            Some(&format!("{a}=")),
        ));
    }
    for f in doc.flags.iter().filter(|f| !has(f)) {
        out.push(item(
            f,
            KIND_KEYWORD,
            &format!("flag of `{}`", doc.name),
            None,
        ));
    }
    if doc.action && !used.iter().any(|u| u.starts_with('@')) {
        out.push(item(
            "@",
            KIND_VALUE,
            "wire to a `.on(\"name\", ..)` handler",
            Some("@"),
        ));
    }
    out
}

/// Markdown for the word under the cursor, if it is an element name.
pub fn hover(src: &str, line: usize, col: usize) -> Option<String> {
    let text = src.lines().nth(line)?;
    let tok = live_syntax::tokens(text)
        .ok()?
        .into_iter()
        .find(|t| t.start <= col && col <= t.end)?;
    let first = live_syntax::tokens(text).ok()?.into_iter().next()?;
    if tok == first {
        let e = live_syntax::element(&tok.text)?;
        let mut md = format!("**{}**: {}", e.name, e.doc);
        if !e.attrs.is_empty() {
            let a: Vec<String> = e.attrs.iter().map(|a| format!("`{a}=`")).collect();
            md.push_str(&format!("\n\nSettings: {}", a.join(", ")));
        }
        if !e.flags.is_empty() {
            let f: Vec<String> = e.flags.iter().map(|f| format!("`{f}`")).collect();
            md.push_str(&format!("\n\nFlags: {}", f.join(", ")));
        }
        return Some(md);
    }
    if let Some(a) = tok.text.strip_prefix('@').filter(|_| !tok.quoted) {
        return Some(format!(
            "Runs the handler registered with `.on(\"{a}\", ..)`."
        ));
    }
    None
}

/// Read one `Content-Length`-framed message. `Ok(None)` at end of input.
pub fn read_message(input: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if input.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(v) = header.to_ascii_lowercase().strip_prefix("content-length:") {
            length = v.trim().parse::<usize>().ok();
        }
    }
    let Some(n) = length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing Content-Length",
        ));
    };
    let mut body = vec![0; n];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write one framed message.
pub fn write_message(out: &mut impl Write, msg: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(msg).map_err(io::Error::other)?;
    write!(out, "Content-Length: {}\r\n\r\n", body.len())?;
    out.write_all(&body)?;
    out.flush()
}

/// Serve until `exit` or end of input. Returns the process exit code (0 after `shutdown`).
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<i32> {
    let mut server = Server::new();
    while let Some(msg) = read_message(&mut input)? {
        let reply = server.handle(&msg);
        for m in &reply.out {
            write_message(&mut output, m)?;
        }
        if reply.exit {
            return Ok(if server.was_shut_down() { 0 } else { 1 });
        }
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "file:///ui/home.alloy";

    fn open(s: &mut Server, text: &str) -> Value {
        let r = s.handle(&json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"text":text}}}));
        r.out.into_iter().next().unwrap()
    }

    fn complete_at(s: &mut Server, line: usize, ch: usize) -> Vec<String> {
        let r = s.handle(&json!({"jsonrpc":"2.0","id":7,"method":"textDocument/completion","params":{"textDocument":{"uri":URI},"position":{"line":line,"character":ch}}}));
        r.out[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["label"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn initialize_advertises_features_and_shutdown_exits_zero() {
        let mut s = Server::new();
        let r = s.handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        let caps = &r.out[0]["result"]["capabilities"];
        assert_eq!(caps["textDocumentSync"], 1);
        assert_eq!(caps["hoverProvider"], true);
        s.handle(&json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}));
        assert!(s.handle(&json!({"jsonrpc":"2.0","method":"exit"})).exit);
        assert!(s.was_shut_down());
        let r = s.handle(&json!({"jsonrpc":"2.0","id":3,"method":"nope"}));
        assert_eq!(r.out[0]["error"]["code"], -32601);
    }

    #[test]
    fn diagnostics_match_the_runtime_and_follow_edits() {
        let mut s = Server::new();
        let p = open(&mut s, "col\n  bttn \"a\"\n");
        let d = &p["params"]["diagnostics"][0];
        assert_eq!(p["method"], "textDocument/publishDiagnostics");
        assert!(d["message"]
            .as_str()
            .unwrap()
            .starts_with("unknown element `bttn`"));
        assert_eq!(
            (
                d["range"]["start"]["line"].clone(),
                d["range"]["start"]["character"].clone()
            ),
            (json!(1), json!(2))
        );
        assert_eq!(d["severity"], 1);
        // Fix it: the next publish is empty.
        let r = s.handle(&json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":URI},"contentChanges":[{"text":"col\n  button \"a\"\n"}]}}));
        assert_eq!(r.out[0]["params"]["diagnostics"], json!([]));
        let r = s.handle(&json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":URI}}}));
        assert_eq!(r.out[0]["params"]["diagnostics"], json!([]));
    }

    #[test]
    fn columns_are_utf16() {
        let mut s = Server::new();
        // The emoji is two UTF-16 units, one character: the bad word starts at character 9.
        let p = open(&mut s, "col\n  text \"😀\" nope\n");
        let d = &p["params"]["diagnostics"][0];
        assert_eq!(d["range"]["start"]["character"], 12);
        assert_eq!(d["range"]["end"]["character"], 16);
    }

    #[test]
    fn completes_elements_settings_flags_and_actions() {
        let mut s = Server::new();
        open(&mut s, "theme dark\ncol\n  \n  button \"x\" @add\n  button \"y\" \n  col \n  text \"{n} x {\"\n");
        let els = complete_at(&mut s, 2, 2);
        assert!(els.contains(&"button".into()) && !els.contains(&"theme".into()));
        // `theme` is offered on the first content line only.
        open(&mut s, "\n");
        assert!(complete_at(&mut s, 0, 0).contains(&"theme".into()));
        open(&mut s, "col\n  button \"x\" @add\n  button \"y\" \n  col \n  text \"{n} x {\"\n  button \"z\" @\n  theme \n");
        let btn = complete_at(&mut s, 2, 13);
        assert!(
            btn.contains(&"primary".into())
                && btn.contains(&"ghost".into())
                && btn.contains(&"@".into())
        );
        assert!(!btn.contains(&"dim".into()));
        let col = complete_at(&mut s, 3, 6);
        assert!(col.contains(&"gap=".into()) && col.contains(&"page".into()));
        assert_eq!(complete_at(&mut s, 4, 15), ["n"]); // after `{` inside the quotes
        assert_eq!(complete_at(&mut s, 5, 14), ["@add"]);
    }

    #[test]
    fn already_used_settings_are_not_offered_again() {
        let mut s = Server::new();
        open(&mut s, "col gap=4 page \n");
        let got = complete_at(&mut s, 0, 15);
        assert!(!got.contains(&"gap=".into()) && !got.contains(&"page".into()));
        assert!(got.contains(&"pad=".into()));
    }

    #[test]
    fn hover_describes_elements() {
        let md = hover("col\n  button \"x\" primary\n", 1, 4).unwrap();
        assert!(md.starts_with("**button**") && md.contains("`primary`"));
        assert!(hover("col\n  button \"x\" @go\n", 1, 16)
            .unwrap()
            .contains("\"go\""));
        assert_eq!(hover("col\n", 0, 99), None);
    }

    #[test]
    fn framing_round_trips_through_serve() {
        let msgs = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":URI,"text":"x \"a\""}}}),
            json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}),
            json!({"jsonrpc":"2.0","method":"exit"}),
        ];
        let mut input = Vec::new();
        for m in &msgs {
            write_message(&mut input, m).unwrap();
        }
        let mut out = Vec::new();
        let code = serve(io::Cursor::new(input), &mut out).unwrap();
        assert_eq!(code, 0);
        let mut cur = io::Cursor::new(out);
        let first = read_message(&mut cur).unwrap().unwrap();
        assert_eq!(first["id"], 1);
        let second = read_message(&mut cur).unwrap().unwrap();
        assert_eq!(second["method"], "textDocument/publishDiagnostics");
        assert_eq!(read_message(&mut cur).unwrap().unwrap()["id"], 2);
        assert!(read_message(&mut cur).unwrap().is_none());
        // Ending without `exit` is an abnormal close.
        assert_eq!(serve(io::Cursor::new(Vec::new()), Vec::new()).unwrap(), 1);
    }
}
