//! Web GUI: drive any [`App`] from a browser tab.
//!
//! The app keeps running natively. A tiny HTTP server (std only, no new dependencies) serves a
//! one-file viewer; the viewer polls `GET /frame?v=N` for the latest frame (PNG, `204` when
//! nothing changed) and sends input as JSON to `POST /event`.
//!
//! ```no_run
//! use alloy_ui::prelude::*;
//! let app = sketch(0i64, |n| button(format!("{n}")).act(|n| *n += 1));
//! alloy_ui::web::serve(app, "127.0.0.1:8080", (900.0, 600.0)).unwrap();
//! ```
//!
//! Events (`{"t": ...}`): `move{x,y}`, `down`, `up`, `scroll{dy}`, `text{s}`, `key{k,mods}`
//! (`mods` like `"mod+shift"`, empty for none) and `resize{w,h}`.
//! This is a remote display, not a webview: the page holds no app logic.

use crate::{app::App, testing::Harness};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

const VIEWER: &str = include_str!("web_viewer.html");

enum Req {
    Frame(u64, Sender<Option<(u64, Vec<u8>)>>),
    Event(serde_json::Value, Sender<()>),
}

/// Serve `app` at `addr` (for example `"127.0.0.1:8080"`) with a `size` logical window.
/// Blocks until the listener fails. Bind to loopback unless you mean to expose the app: the
/// page can click and type into it.
pub fn serve<A: App>(app: A, addr: &str, size: (f32, f32)) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("Alloy web GUI: http://{}", listener.local_addr()?);
    let (tx, rx) = mpsc::channel::<Req>();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let _ = connection(stream, &tx);
            });
        }
    });
    let mut ui = Harness::new(app, size.0, size.1);
    // The frame is encoded lazily, when a viewer asks and something changed, so a burst of mouse
    // moves costs one PNG, and an idle app costs nothing: the loop sleeps until a request arrives.
    let (mut version, mut png, mut stale) = (1u64, Vec::new(), true);
    loop {
        let next = if ui.animating() {
            rx.recv_timeout(Duration::from_millis(16))
        } else {
            rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match next {
            Ok(Req::Frame(seen, reply)) => {
                if stale {
                    png = ui.png();
                    stale = false;
                }
                let _ = reply.send((seen != version).then(|| (version, png.clone())));
            }
            Ok(Req::Event(ev, reply)) => {
                apply(&mut ui, &ev);
                ui.settle();
                (version, stale) = (version + 1, true);
                let _ = reply.send(());
            }
            Err(RecvTimeoutError::Timeout) => {
                ui.advance(16);
                (version, stale) = (version + 1, true);
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn apply<A: App>(ui: &mut Harness<A>, ev: &serde_json::Value) {
    let f = |k: &str| ev[k].as_f64().unwrap_or(0.0) as f32;
    let s = |k: &str| ev[k].as_str().unwrap_or("");
    match s("t") {
        "move" => drop(ui.move_to(f("x"), f("y"))),
        "down" => drop(ui.mouse_down()),
        "up" => drop(ui.mouse_up()),
        "scroll" => drop(ui.scroll_here(f("dy"))),
        "text" => drop(ui.type_text(s("s"))),
        "resize" => drop(ui.resize(f("w").clamp(1.0, 8192.0), f("h").clamp(1.0, 8192.0))),
        "key" => {
            let (k, mods) = (s("k"), s("mods"));
            if mods.is_empty() && k != "Backspace" {
                ui.key(k);
            } else {
                let chord = if mods.is_empty() {
                    k.to_lowercase()
                } else {
                    format!("{mods}+{}", k.to_lowercase())
                };
                // An unparsable chord from a browser must not take the app down.
                if alloy_ui_platform::shortcut::Shortcut::parse(&chord).is_some() {
                    ui.chord(&chord);
                }
            }
        }
        _ => {}
    }
    ui.advance(16);
}

fn connection(mut stream: TcpStream, tx: &Sender<Req>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let mut len = 0usize;
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0).min(64 * 1024);
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    let (rtx, rrx) = mpsc::channel();
    match (method.as_str(), path) {
        ("GET", "/") => respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            VIEWER.as_bytes(),
        ),
        ("GET", "/frame") => {
            let seen = query
                .strip_prefix("v=")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            tx.send(Req::Frame(seen, rtx)).ok();
            match rrx.recv().ok().flatten() {
                Some((v, png)) => {
                    let head = format!("200 OK\r\nX-Version: {v}");
                    respond(&mut stream, &head, "image/png", &png)
                }
                None => respond(&mut stream, "204 No Content", "text/plain", b""),
            }
        }
        ("POST", "/event") => {
            let mut body = vec![0; len];
            reader.read_exact(&mut body)?;
            if let Ok(ev) = serde_json::from_slice(&body) {
                tx.send(Req::Event(ev, mpsc::channel().0)).ok();
            }
            respond(&mut stream, "204 No Content", "text/plain", b"")
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn respond(s: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    s.write_all(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    fn get(addr: &str, path: &str) -> (String, Vec<u8>) {
        let mut s = TcpStream::connect(addr).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
        let mut raw = Vec::new();
        s.read_to_end(&mut raw).unwrap();
        let at = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        (
            String::from_utf8_lossy(&raw[..at]).into(),
            raw[at + 4..].to_vec(),
        )
    }

    fn post(addr: &str, body: &str) {
        let mut s = TcpStream::connect(addr).unwrap();
        write!(
            s,
            "POST /event HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut sink = Vec::new();
        let _ = s.read_to_end(&mut sink);
    }

    #[test]
    fn serves_viewer_frames_and_survives_garbage_input() {
        let probe = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap().to_string();
        drop(probe);
        let a = addr.clone();
        std::thread::spawn(move || {
            let app = sketch(0i64, |n| button(format!("n={n}")).act(|n| *n += 1));
            serve(app, &a, (200.0, 100.0)).unwrap();
        });
        let mut viewer = None;
        for _ in 0..100 {
            if let Ok(s) = TcpStream::connect(&addr) {
                drop(s);
                viewer = Some(get(&addr, "/"));
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let (head, body) = viewer.expect("server did not start");
        assert!(
            head.starts_with("HTTP/1.1 200")
                && String::from_utf8_lossy(&body).contains("Alloy web GUI")
        );

        let (head, png) = get(&addr, "/frame?v=0");
        assert!(head.contains("X-Version"), "{head}");
        assert_eq!(&png[..4], b"\x89PNG");
        let v: u64 = head
            .split("X-Version: ")
            .nth(1)
            .unwrap()
            .split("\r\n")
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            get(&addr, &format!("/frame?v={v}"))
                .0
                .starts_with("HTTP/1.1 204"),
            "idle must not resend"
        );

        post(&addr, "not json");
        post(&addr, r#"{"t":"key","k":"Nonsense+++","mods":"zzz"}"#);
        post(&addr, r#"{"t":"resize","w":0,"h":-5}"#);
        post(&addr, r#"{"t":"move","x":30,"y":20}"#);
        let (head, _) = get(&addr, &format!("/frame?v={v}"));
        assert!(
            head.starts_with("HTTP/1.1 200"),
            "an event must produce a new frame: {head}"
        );
    }
}
