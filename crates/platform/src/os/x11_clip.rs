//! Native X11 clipboard over `x11rb` (pure-Rust protocol, no `unsafe`).
//!
//! X11 has no clipboard storage: the owner of the `CLIPBOARD` selection must
//! stay alive and answer requests. `write` therefore parks a small thread that
//! owns the selection until another client takes it (`SelectionClear`).
//! `read` asks the owner to convert the selection to one target (UTF-8 text,
//! `text/html`, `image/png`, `text/uri-list`...), following the `INCR`
//! protocol for large values; `write_formats` serves several targets at once
//! and sends large ones with INCR.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, CreateWindowAux, EventMask, PropMode,
    Property, SelectionNotifyEvent, Window, WindowClass, SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};

/// Values larger than this are left to helper tools (a screenshot can be tens of MB).
const MAX_SERVE: usize = 64 * 1024 * 1024;
/// Values larger than this are sent in INCR chunks of this size.
const INCR_CHUNK: usize = 64 * 1024;
const TIMEOUT: Duration = Duration::from_millis(700);

struct Atoms {
    clipboard: Atom,
    targets: Atom,
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
        targets: intern(&c, "TARGETS")?,
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
    String::from_utf8(read_target("UTF8_STRING")?).ok()
}

/// The targets the current owner offers (`TARGETS`), by name.
pub fn read_targets() -> Option<Vec<String>> {
    let k = connect()?;
    let t = fetch(&k, k.a.targets)?;
    let atoms: Vec<Atom> = t
        .chunks_exact(4)
        .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    Some(
        atoms
            .into_iter()
            .filter_map(|a| {
                let r = k.c.get_atom_name(a).ok()?.reply().ok()?;
                String::from_utf8(r.name).ok()
            })
            .collect(),
    )
}

/// Ask the owner to convert the selection to `target` (a name such as
/// `image/png`) and return the bytes.
pub fn read_target(target: &str) -> Option<Vec<u8>> {
    let k = connect()?;
    let atom = intern(&k.c, target)?;
    fetch(&k, atom)
}

fn fetch(k: &Conn, target: Atom) -> Option<Vec<u8>> {
    let (c, a) = (&k.c, &k.a);
    c.convert_selection(k.win, a.clipboard, target, a.prop, CURRENT_TIME)
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
        return Some(reply.value);
    }
    // INCR: the owner sends chunks as property changes; a zero-length chunk ends it.
    let mut out = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
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
            return Some(out);
        }
        out.extend_from_slice(&chunk.value);
    }
}

/// Become the clipboard owner and serve `text` from a background thread.
/// Returns false if X11 is unavailable or the value is too large to serve.
pub fn write(text: &str) -> bool {
    write_formats(crate::clipboard::to_formats(&crate::clipboard::ClipContent::text(
        text,
    )))
}

/// Become the clipboard owner and serve every `(target, bytes)` pair from a
/// background thread until another client takes the selection.
pub fn write_formats(formats: Vec<(String, Vec<u8>)>) -> bool {
    if formats.is_empty() || formats.iter().any(|(_, d)| d.len() > MAX_SERVE) {
        return false;
    }
    let Some(k) = connect() else {
        return false;
    };
    let mut table: HashMap<Atom, Vec<u8>> = HashMap::new();
    for (name, data) in formats {
        let Some(atom) = intern(&k.c, &name) else {
            return false;
        };
        table.entry(atom).or_insert(data);
    }
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
    std::thread::Builder::new()
        .name("alloy-clipboard".into())
        .spawn(move || serve(k, table))
        .is_ok()
}

/// A large value being sent to one requestor in INCR chunks.
struct Transfer {
    target: Atom,
    data: Vec<u8>,
    sent: usize,
    finished: bool,
}

/// Answer selection requests until another client takes the selection.
fn serve(k: Conn, table: HashMap<Atom, Vec<u8>>) {
    let (c, a) = (&k.c, &k.a);
    let mut transfers: HashMap<(Window, Atom), Transfer> = HashMap::new();
    while let Ok(event) = c.wait_for_event() {
        match event {
            Event::SelectionClear(_) => return,
            Event::PropertyNotify(p) if p.state == Property::DELETE => {
                // The requestor consumed the last chunk: send the next one.
                let key = (p.window, p.atom);
                let Some(t) = transfers.get_mut(&key) else {
                    continue;
                };
                if t.finished {
                    transfers.remove(&key);
                    continue;
                }
                let end = (t.sent + INCR_CHUNK).min(t.data.len());
                let chunk = &t.data[t.sent..end];
                let _ = c.change_property8(PropMode::REPLACE, p.window, p.atom, t.target, chunk);
                let _ = c.flush();
                t.finished = chunk.is_empty();
                t.sent = end;
            }
            Event::SelectionRequest(r) => {
                let property = if r.target == a.targets {
                    let mut t = vec![a.targets];
                    t.extend(table.keys().copied());
                    c.change_property32(
                        PropMode::REPLACE,
                        r.requestor,
                        r.property,
                        AtomEnum::ATOM,
                        &t,
                    )
                    .ok()
                    .map(|_| r.property)
                } else if let Some(data) = table.get(&r.target) {
                    if data.len() > INCR_CHUNK {
                        // Tell the requestor to expect chunks, and watch its window
                        // for the deletions that ask for the next one.
                        let _ = c.change_window_attributes(
                            r.requestor,
                            &ChangeWindowAttributesAux::new()
                                .event_mask(EventMask::PROPERTY_CHANGE),
                        );
                        let _ = c.change_property32(
                            PropMode::REPLACE,
                            r.requestor,
                            r.property,
                            a.incr,
                            &[data.len() as u32],
                        );
                        transfers.insert(
                            (r.requestor, r.property),
                            Transfer {
                                target: r.target,
                                data: data.clone(),
                                sent: 0,
                                finished: false,
                            },
                        );
                        Some(r.property)
                    } else {
                        c.change_property8(
                            PropMode::REPLACE,
                            r.requestor,
                            r.property,
                            r.target,
                            data,
                        )
                        .ok()
                        .map(|_| r.property)
                    }
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
