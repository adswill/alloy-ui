//! Global hotkeys on Wayland through the XDG desktop portal
//! (`org.freedesktop.portal.GlobalShortcuts`), over our own D-Bus client.
//!
//! Wayland lets no client grab keys, so the compositor's portal does it: we ask
//! for a session, then for a list of shortcuts, and the portal sends an
//! `Activated` signal when one fires. The portal may show the user a dialog to
//! approve or rebind the shortcuts, and the user's choice wins over the key we
//! asked for. Supported by GNOME 48+ and KDE Plasma 5.27+; compositors without
//! the portal (Sway, Hyprland without a backend) answer `ServiceUnknown` and
//! registration fails, so apps should keep in-window shortcuts as the fallback.
//!
//! One session carries every hotkey: `BindShortcuts` may be called only once per
//! session, so each new `register` closes the session and opens another with the
//! whole list.

use super::dbus::{Connection, Kind, Message, Reader, Writer};
use super::push_pending;
use crate::{shortcut::Shortcut, PlatformError, PlatformEvent};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;

const DEST: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
const REQUEST: &str = "org.freedesktop.portal.Request";

struct Session {
    stop: std::sync::Arc<AtomicBool>,
    stream: std::os::unix::net::UnixStream,
}

static HOTKEYS: Mutex<Vec<(String, Shortcut)>> = Mutex::new(Vec::new());
static OPEN: Mutex<Option<Session>> = Mutex::new(None);

/// The shortcut in the portal's `CTRL+ALT+space` notation (xkb key names).
pub(crate) fn trigger(sc: &Shortcut) -> Option<String> {
    let key = match sc.key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "Return".to_string(),
        "escape" | "esc" => "Escape".into(),
        "tab" => "Tab".into(),
        "backspace" => "BackSpace".into(),
        "delete" => "Delete".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        "left" | "arrowleft" => "Left".into(),
        "right" | "arrowright" => "Right".into(),
        "up" | "arrowup" => "Up".into(),
        "down" | "arrowdown" => "Down".into(),
        "pageup" => "Page_Up".into(),
        "pagedown" => "Page_Down".into(),
        "space" => "space".into(),
        k if k.len() == 1 && k.is_ascii() => k.to_string(),
        k if k.starts_with('f') && k[1..].parse::<u32>().is_ok_and(|n| (1..=12).contains(&n)) => {
            k.to_ascii_uppercase()
        }
        _ => return None,
    };
    let mut parts = Vec::new();
    if sc.mods.ctrl {
        parts.push("CTRL");
    }
    if sc.mods.alt {
        parts.push("ALT");
    }
    if sc.mods.shift {
        parts.push("SHIFT");
    }
    if sc.mods.logo {
        parts.push("LOGO");
    }
    parts.push(&key);
    Some(parts.join("+"))
}

fn sv_string(w: &mut Writer, key: &str, val: &str) {
    w.align(8);
    w.string(key);
    w.variant("s", |w| w.string(val));
}

/// Where the portal will answer a request made with `token` from `conn`.
fn request_path(conn: &Connection, token: &str) -> String {
    let sender = conn.unique_name.trim_start_matches(':').replace('.', "_");
    format!("{PATH}/request/{sender}/{token}")
}

/// Read until the `Response` for `path`; returns the result code and, if the
/// results dictionary has one, the `session_handle`.
fn await_response(
    conn: &mut Connection,
    path: &str,
) -> Result<(u32, Option<String>), PlatformError> {
    loop {
        let m = conn.read().map_err(|e| PlatformError(e.0))?;
        if m.kind == Kind::Signal
            && m.interface.as_deref() == Some(REQUEST)
            && m.member.as_deref() == Some("Response")
            && m.path.as_deref() == Some(path)
        {
            return parse_response(&m)
                .ok_or_else(|| PlatformError("malformed portal response".into()));
        }
    }
}

fn parse_response(m: &Message) -> Option<(u32, Option<String>)> {
    let mut r = Reader::new(&m.body);
    let code = r.u32().ok()?;
    let len = r.u32().ok()? as usize;
    r.align(8);
    let end = r.pos + len;
    let mut session = None;
    while r.pos < end {
        r.align(8);
        let key = r.string().ok()?;
        let sig = r.signature().ok()?;
        if sig == "s" || sig == "o" {
            let v = r.string().ok()?;
            if key == "session_handle" {
                session = Some(v);
            }
        } else {
            break; // other value types are not needed; stop rather than mis-parse
        }
    }
    Some((code, session))
}

fn add_match(conn: &mut Connection, rule: &str) -> Result<(), PlatformError> {
    let mut w = Writer::default();
    w.string(rule);
    conn.call(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "AddMatch",
        "s",
        &w.buf,
    )
    .map(|_| ())
    .map_err(|e| PlatformError(e.0))
}

pub(crate) fn register(keys: &str, sc: &Shortcut) -> Result<(), PlatformError> {
    let fail = |m: String| PlatformError(format!("global hotkey {keys}: {m}"));
    trigger(sc).ok_or_else(|| fail(format!("unknown key {:?}", sc.key)))?;
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
        && std::env::var_os("XDG_RUNTIME_DIR").is_none()
    {
        return Err(fail("no session bus".into()));
    }
    {
        let mut all = HOTKEYS.lock().map_err(|_| fail("lock poisoned".into()))?;
        all.retain(|(k, _)| k != keys);
        all.push((keys.to_string(), sc.clone()));
    }
    let result = open_session().map_err(|e| fail(e.0));
    if result.is_err() {
        if let Ok(mut all) = HOTKEYS.lock() {
            all.retain(|(k, _)| k != keys);
        }
    }
    result
}

/// Close the current session and open one that binds every registered hotkey.
fn open_session() -> Result<(), PlatformError> {
    close_session();
    let list = HOTKEYS.lock().map(|h| h.clone()).unwrap_or_default();
    let mut conn = Connection::session().map_err(|e| PlatformError(e.0))?;
    let stream = conn.try_clone_stream().map_err(|e| PlatformError(e.0))?;
    // A portal that is slow to answer must not hang the UI thread forever.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    add_match(&mut conn, &format!("type='signal',interface='{REQUEST}'"))?;
    add_match(&mut conn, &format!("type='signal',interface='{IFACE}'"))?;

    let token = format!("alloy{}", std::process::id());
    let session_token = format!("{token}s");
    let create = request_path(&conn, &format!("{token}c"));
    let mut w = Writer::default();
    w.array(8, |w| {
        sv_string(w, "handle_token", &format!("{token}c"));
        sv_string(w, "session_handle_token", &session_token);
    });
    conn.call(DEST, PATH, IFACE, "CreateSession", "a{sv}", &w.buf)
        .map_err(|e| {
            PlatformError(format!(
                "{} (the desktop has no GlobalShortcuts portal; Wayland offers no other way)",
                e.0
            ))
        })?;
    let (code, session) = await_response(&mut conn, &create)?;
    let session = match (code, session) {
        (0, Some(s)) => s,
        (c, _) => {
            return Err(PlatformError(format!(
                "the portal refused a session (code {c})"
            )))
        }
    };

    let bind = request_path(&conn, &format!("{token}b"));
    let mut w = Writer::default();
    w.string(&session);
    w.array(8, |w| {
        for (name, sc) in &list {
            w.align(8);
            w.string(name);
            w.array(8, |w| {
                sv_string(w, "description", name);
                if let Some(t) = trigger(sc) {
                    sv_string(w, "preferred_trigger", &t);
                }
            });
        }
    });
    w.string(""); // parent_window: none
    w.array(8, |w| sv_string(w, "handle_token", &format!("{token}b")));
    conn.call(
        DEST,
        PATH,
        IFACE,
        "BindShortcuts",
        "oa(sa{sv})sa{sv}",
        &w.buf,
    )
    .map_err(|e| PlatformError(e.0))?;

    // The bind answer can wait on a dialog, so the worker reads it, not the caller.
    let _ = stream.set_read_timeout(None);
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::Builder::new()
        .name("alloy-hotkey".into())
        .spawn(move || {
            while let Ok(m) = conn.read() {
                if flag.load(Ordering::Acquire) {
                    break;
                }
                if m.kind != Kind::Signal {
                    continue;
                }
                match (m.interface.as_deref(), m.member.as_deref()) {
                    (Some(REQUEST), Some("Response")) if m.path.as_deref() == Some(&bind) => {
                        if let Some((code, _)) = parse_response(&m) {
                            if code != 0 {
                                log::warn!(
                                    "global shortcuts: the portal did not bind them (code {code})"
                                );
                            }
                        }
                    }
                    (Some(IFACE), Some("Activated")) => {
                        let mut r = Reader::new(&m.body);
                        if r.string().is_ok_and(|s| s == session) {
                            if let Ok(id) = r.string() {
                                push_pending(PlatformEvent::GlobalHotkey(id));
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| PlatformError(e.to_string()))?;
    if let Ok(mut open) = OPEN.lock() {
        *open = Some(Session { stop, stream });
    }
    Ok(())
}

fn close_session() {
    if let Some(s) = OPEN.lock().ok().and_then(|mut o| o.take()) {
        s.stop.store(true, Ordering::Release);
        let _ = s.stream.shutdown(std::net::Shutdown::Both);
    }
}

pub(crate) fn release_all() {
    close_session();
    if let Ok(mut h) = HOTKEYS.lock() {
        h.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_notation() {
        let t = |s: &str| trigger(&Shortcut::parse(s).unwrap());
        assert_eq!(t("ctrl+alt+space").as_deref(), Some("CTRL+ALT+space"));
        assert_eq!(t("shift+f5").as_deref(), Some("SHIFT+F5"));
        assert_eq!(t("ctrl+pagedown").as_deref(), Some("CTRL+Page_Down"));
        assert_eq!(t("ctrl+K").as_deref(), Some("CTRL+k"));
    }

    /// A fake portal on a private session bus: serves CreateSession and
    /// BindShortcuts, then fires `Activated`. Skipped without a session bus.
    #[test]
    fn portal_session_delivers_activations() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            eprintln!("skipped: no session bus");
            return;
        }
        let _guard = super::super::BUS_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut server = Connection::session().expect("server");
        let mut w = Writer::default();
        w.string(DEST);
        w.u32(4);
        server
            .call(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "RequestName",
                "su",
                &w.buf,
            )
            .expect("own the portal name");
        let emit = server.emitter();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(m) = server.read() {
                if m.kind != Kind::Call {
                    continue;
                }
                let sender = m.sender.clone().unwrap_or_default();
                let sender = sender.trim_start_matches(':').replace('.', "_");
                let body = String::from_utf8_lossy(&m.body).into_owned();
                let (token_suffix, is_create) = match m.member.as_deref() {
                    Some("CreateSession") => ('c', true),
                    Some("BindShortcuts") => ('b', false),
                    _ => continue,
                };
                let token = format!("alloy{}{token_suffix}", std::process::id());
                assert!(body.contains(&token), "request carries its handle token");
                let req = format!("{PATH}/request/{sender}/{token}");
                let mut w = Writer::default();
                w.string(&req);
                server.reply(&m, "o", &w.buf).unwrap();
                let mut r = Writer::default();
                r.u32(0);
                r.array(8, |w| {
                    if is_create {
                        w.align(8);
                        w.string("session_handle");
                        w.variant("s", |w| {
                            w.string("/org/freedesktop/portal/desktop/session/x/s")
                        });
                    }
                });
                emit.signal(&req, REQUEST, "Response", "ua{sv}", &r.buf)
                    .unwrap();
                if !is_create {
                    assert!(body.contains("CTRL+ALT+space"), "preferred trigger sent");
                    tx.send(()).unwrap();
                    std::thread::sleep(Duration::from_millis(100));
                    let mut a = Writer::default();
                    a.string("/org/freedesktop/portal/desktop/session/x/s");
                    a.string("ctrl+alt+space");
                    a.align(8);
                    a.buf.extend([0u8; 8]); // timestamp (t)
                    a.array(8, |_| {}); // empty a{sv}
                    emit.signal(PATH, IFACE, "Activated", "osta{sv}", &a.buf)
                        .unwrap();
                }
            }
        });
        register(
            "ctrl+alt+space",
            &Shortcut::parse("ctrl+alt+space").unwrap(),
        )
        .expect("session opens against the fake portal");
        rx.recv_timeout(Duration::from_secs(5))
            .expect("portal got BindShortcuts");
        let start = std::time::Instant::now();
        loop {
            let got = crate::os::take_pending();
            if got.contains(&PlatformEvent::GlobalHotkey("ctrl+alt+space".into())) {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "Activated never arrived"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        release_all();
    }
}
