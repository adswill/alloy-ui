//! Native X11 clipboard over `x11rb` (pure-Rust protocol, no `unsafe`).
//!
//! X11 has no clipboard storage: the owner of the `CLIPBOARD` selection must
//! stay alive and answer requests. `write` therefore parks a small thread that
//! owns the selection until another client takes it (`SelectionClear`).
//! `read` asks the owner to convert the selection to UTF-8 text, following the
//! `INCR` protocol for large values.

use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt, CreateWindowAux, EventMask, PropMode, Property,
    SelectionNotifyEvent, Window, WindowClass, SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};

/// Values larger than this are left to helper tools: serving them would need
/// the sender side of INCR.
const MAX_SERVE: usize = 128 * 1024;
const TIMEOUT: Duration = Duration::from_millis(700);

struct Atoms {
    clipboard: Atom,
    utf8: Atom,
    targets: Atom,
    text: Atom,
    plain_utf8: Atom,
    incr: Atom,
    prop: Atom,
}

struct Conn {
    c: RustConnection,
    win: Window,
    a: Atoms,
}

fn intern(c: &RustConnection, name: &str) -> Option<Atom> {
    Some(
        c.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom,
    )
}

fn connect() -> Option<Conn> {
    let (c, screen) = RustConnection::connect(None).ok()?;
    let root = c.setup().roots.get(screen)?.root;
    let win = c.generate_id().ok()?;
    c.create_window(
        COPY_DEPTH_FROM_PARENT,
        win,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        // Property changes are needed to receive INCR chunks.
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )
    .ok()?;
    let a = Atoms {
        clipboard: intern(&c, "CLIPBOARD")?,
        utf8: intern(&c, "UTF8_STRING")?,
        targets: intern(&c, "TARGETS")?,
        text: intern(&c, "TEXT")?,
        plain_utf8: intern(&c, "text/plain;charset=utf-8")?,
        incr: intern(&c, "INCR")?,
        prop: intern(&c, "ALLOY_CLIPBOARD")?,
    };
    Some(Conn { c, win, a })
}

/// Wait for the next event or give up at `deadline`.
fn next_event(c: &RustConnection, deadline: Instant) -> Option<Event> {
    loop {
        if let Ok(Some(e)) = c.poll_for_event() {
            return Some(e);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Read the clipboard as UTF-8 text. `None` if X11 is unavailable, nobody owns
/// the selection, or the owner does not answer in time.
pub fn read() -> Option<String> {
    let k = connect()?;
    let (c, a) = (&k.c, &k.a);
    c.convert_selection(k.win, a.clipboard, a.utf8, a.prop, CURRENT_TIME)
        .ok()?;
    c.flush().ok()?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Event::SelectionNotify(n) = next_event(c, deadline)? {
            if n.property == x11rb::NONE {
                return None; // owner refused the conversion
            }
            break;
        }
    }
    let reply = c
        .get_property(true, k.win, a.prop, AtomEnum::ANY, 0, u32::MAX / 4)
        .ok()?
        .reply()
        .ok()?;
    if reply.type_ != a.incr {
        return String::from_utf8(reply.value).ok();
    }
    // INCR: the owner sends chunks as property changes; a zero-length chunk ends it.
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let Event::PropertyNotify(p) = next_event(c, deadline)? else {
            continue;
        };
        if p.window != k.win || p.atom != a.prop || p.state != Property::NEW_VALUE {
            continue;
        }
        let chunk = c
            .get_property(true, k.win, a.prop, AtomEnum::ANY, 0, u32::MAX / 4)
            .ok()?
            .reply()
            .ok()?;
        if chunk.value.is_empty() {
            return String::from_utf8(out).ok();
        }
        out.extend_from_slice(&chunk.value);
    }
}

/// Become the clipboard owner and serve `text` from a background thread.
/// Returns false if X11 is unavailable or the value is too large to serve.
pub fn write(text: &str) -> bool {
    if text.len() > MAX_SERVE {
        return false;
    }
    let Some(k) = connect() else {
        return false;
    };
    if k.c
        .set_selection_owner(k.win, k.a.clipboard, CURRENT_TIME)
        .is_err()
        || k.c.flush().is_err()
    {
        return false;
    }
    // Confirm we really own it before reporting success.
    let owned =
        k.c.get_selection_owner(k.a.clipboard)
            .ok()
            .and_then(|r| r.reply().ok())
            .is_some_and(|r| r.owner == k.win);
    if !owned {
        return false;
    }
    let data = text.as_bytes().to_vec();
    std::thread::Builder::new()
        .name("alloy-clipboard".into())
        .spawn(move || serve(k, data))
        .is_ok()
}

/// Answer selection requests until another client takes the selection.
fn serve(k: Conn, data: Vec<u8>) {
    let (c, a) = (&k.c, &k.a);
    while let Ok(event) = c.wait_for_event() {
        match event {
            Event::SelectionClear(_) => return,
            Event::SelectionRequest(r) => {
                let property = if r.target == a.targets {
                    let t = [
                        a.targets,
                        a.utf8,
                        AtomEnum::STRING.into(),
                        a.text,
                        a.plain_utf8,
                    ];
                    c.change_property32(
                        PropMode::REPLACE,
                        r.requestor,
                        r.property,
                        AtomEnum::ATOM,
                        &t,
                    )
                    .ok()
                    .map(|_| r.property)
                } else if [a.utf8, a.text, a.plain_utf8, AtomEnum::STRING.into()]
                    .contains(&r.target)
                {
                    c.change_property8(PropMode::REPLACE, r.requestor, r.property, r.target, &data)
                        .ok()
                        .map(|_| r.property)
                } else {
                    None
                };
                let notify = SelectionNotifyEvent {
                    response_type: SELECTION_NOTIFY_EVENT,
                    sequence: 0,
                    time: r.time,
                    requestor: r.requestor,
                    selection: r.selection,
                    target: r.target,
                    property: property.unwrap_or(x11rb::NONE),
                };
                let _ = c.send_event(false, r.requestor, EventMask::NO_EVENT, notify);
                let _ = c.flush();
            }
            _ => {}
        }
    }
}
