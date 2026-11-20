//! A minimal D-Bus client: just enough of the wire protocol to own a bus
//! name, export one object and answer method calls. Written from the D-Bus
//! specification; no D-Bus crate is used.
//!
//! Supported types: byte, bool, int32, uint32, string, object path,
//! signature, variant, arrays and structs. Messages are little-endian only.
//! Used by the Linux tray (`linux_tray.rs`); portable to any Unix.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
};

/// Error from the bus connection.
#[derive(Debug)]
pub struct DbusError(pub String);

impl From<std::io::Error> for DbusError {
    fn from(e: std::io::Error) -> Self {
        DbusError(e.to_string())
    }
}

type Result<T> = std::result::Result<T, DbusError>;

fn err<T>(m: &str) -> Result<T> {
    Err(DbusError(m.into()))
}

/// Growable little-endian writer with D-Bus alignment.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn align(&mut self, n: usize) {
        while self.buf.len() % n != 0 {
            self.buf.push(0);
        }
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u32(&mut self, v: u32) {
        self.align(4);
        self.buf.extend(v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.u32(v as u32);
    }
    pub fn i16(&mut self, v: i16) {
        self.align(2);
        self.buf.extend(v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.align(8);
        self.buf.extend(v.to_le_bytes());
    }
    /// Object reference `(so)`: bus name and path.
    pub fn reference(&mut self, name: &str, path: &str) {
        self.align(8);
        self.string(name);
        self.string(path);
    }
    pub fn bool(&mut self, v: bool) {
        self.u32(v as u32);
    }
    pub fn string(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.buf.extend(s.as_bytes());
        self.buf.push(0);
    }
    pub fn signature(&mut self, s: &str) {
        self.u8(s.len() as u8);
        self.buf.extend(s.as_bytes());
        self.buf.push(0);
    }
    /// Variant holding a value written by `f` with signature `sig`.
    pub fn variant(&mut self, sig: &str, f: impl FnOnce(&mut Writer)) {
        self.signature(sig);
        f(self);
    }
    /// Array: length prefix, padding to the element alignment, then elements.
    pub fn array(&mut self, elem_align: usize, f: impl FnOnce(&mut Writer)) {
        self.align(4);
        let len_at = self.buf.len();
        self.buf.extend([0; 4]);
        self.align(elem_align);
        let start = self.buf.len();
        f(self);
        let n = (self.buf.len() - start) as u32;
        self.buf[len_at..len_at + 4].copy_from_slice(&n.to_le_bytes());
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.array(1, |w| w.buf.extend(b));
    }
}

/// Little-endian reader with D-Bus alignment.
pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    pub fn align(&mut self, n: usize) {
        while self.pos % n != 0 {
            self.pos += 1;
        }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len());
        let Some(end) = end else {
            return err("message truncated");
        };
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32> {
        self.align(4);
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn f64(&mut self) -> Result<f64> {
        self.align(8);
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        let s = String::from_utf8_lossy(self.take(n)?).into_owned();
        self.take(1)?; // NUL
        Ok(s)
    }
    pub fn signature(&mut self) -> Result<String> {
        let n = self.u8()? as usize;
        let s = String::from_utf8_lossy(self.take(n)?).into_owned();
        self.take(1)?;
        Ok(s)
    }
}

/// Message kinds we care about.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Kind {
    Call,
    Return,
    Error,
    Signal,
}

/// A decoded message header plus its raw body.
#[derive(Debug)]
pub struct Message {
    pub kind: Kind,
    pub serial: u32,
    pub reply_serial: Option<u32>,
    pub path: Option<String>,
    pub interface: Option<String>,
    pub member: Option<String>,
    pub sender: Option<String>,
    pub signature: String,
    pub body: Vec<u8>,
}

/// Encode a message. `fields` are (code, signature, writer-for-value).
pub fn encode(
    kind: u8,
    serial: u32,
    fields: &[(u8, &str, &str)],
    u32_fields: &[(u8, u32)],
    sig: &str,
    body: &[u8],
) -> Vec<u8> {
    // Fixed header is 12 bytes; the field array then starts at 16 (8-aligned),
    // so struct alignment inside it is relative to message start as required.
    let mut out = vec![b'l', kind, 0, 1];
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(serial.to_le_bytes());
    let mut w = Writer { buf: out };
    w.array(8, |w| {
        for (code, ty, val) in fields {
            w.align(8);
            w.u8(*code);
            w.variant(ty, |w| {
                if *ty == "g" {
                    w.signature(val)
                } else {
                    w.string(val)
                }
            });
        }
        for (code, v) in u32_fields {
            w.align(8);
            w.u8(*code);
            w.variant("u", |w| w.u32(*v));
        }
        if !sig.is_empty() {
            w.align(8);
            w.u8(8);
            w.variant("g", |w| w.signature(sig));
        }
    });
    w.align(8);
    w.buf.extend(body);
    w.buf
}

/// Parse a complete message from `raw`.
pub fn decode(raw: &[u8]) -> Result<Message> {
    if raw.len() < 16 || raw[0] != b'l' {
        return err("unsupported or short message");
    }
    let kind = match raw[1] {
        1 => Kind::Call,
        2 => Kind::Return,
        3 => Kind::Error,
        4 => Kind::Signal,
        _ => return err("unknown message type"),
    };
    let body_len = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
    let serial = u32::from_le_bytes(raw[8..12].try_into().unwrap());
    let mut r = Reader::new(raw);
    r.pos = 12;
    let fields_len = r.u32()? as usize;
    let end = r.pos + fields_len;
    let mut m = Message {
        kind,
        serial,
        reply_serial: None,
        path: None,
        interface: None,
        member: None,
        sender: None,
        signature: String::new(),
        body: Vec::new(),
    };
    while r.pos < end {
        r.align(8);
        if r.pos >= end {
            break;
        }
        let code = r.u8()?;
        let sig = r.signature()?;
        match (code, sig.as_str()) {
            (1, "o") => m.path = Some(r.string()?),
            (2, "s") => m.interface = Some(r.string()?),
            (3, "s") => m.member = Some(r.string()?),
            (5, "u") => m.reply_serial = Some(r.u32()?),
            (7, "s") => m.sender = Some(r.string()?),
            (8, "g") => m.signature = r.signature()?,
            (_, "s" | "o") => {
                r.string()?;
            }
            (_, "g") => {
                r.signature()?;
            }
            (_, "u") => {
                r.u32()?;
            }
            _ => return err("unsupported header field type"),
        }
    }
    r.pos = end;
    r.align(8);
    m.body = r.take(body_len)?.to_vec();
    Ok(m)
}

/// A connection to the session bus.
pub struct Connection {
    stream: UnixStream,
    /// Write half, shared with [`Emitter`]s so other threads can send signals.
    writer: Arc<Mutex<UnixStream>>,
    serial: Arc<AtomicU32>,
    /// Our unique name, e.g. `:1.42`.
    pub unique_name: String,
}

fn session_address() -> Result<String> {
    std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .ok()
        .or_else(|| {
            std::env::var("XDG_RUNTIME_DIR")
                .ok()
                .map(|d| format!("unix:path={d}/bus"))
        })
        .ok_or_else(|| DbusError("no session bus address".into()))
}

fn connect_stream(addr: &str) -> Result<UnixStream> {
    for part in addr.split(';') {
        let Some(rest) = part.strip_prefix("unix:") else {
            continue;
        };
        for kv in rest.split(',') {
            if let Some(p) = kv.strip_prefix("path=") {
                return Ok(UnixStream::connect(p)?);
            }
            if let Some(name) = kv.strip_prefix("abstract=") {
                use std::os::linux::net::SocketAddrExt;
                let a = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes())?;
                return Ok(UnixStream::connect_addr(&a)?);
            }
        }
    }
    err("unsupported session bus address")
}

impl Connection {
    /// Connect, authenticate (EXTERNAL) and say Hello.
    pub fn session() -> Result<Self> {
        Self::open(&session_address()?)
    }

    /// Connect to the bus at `addr` (a `unix:path=...` or `unix:abstract=...`
    /// address, as used for the accessibility bus), authenticate and say Hello.
    pub fn open(addr: &str) -> Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let mut stream = connect_stream(addr)?;
        let uid = std::fs::metadata("/proc/self")?.uid().to_string();
        let hex: String = uid.bytes().map(|b| format!("{b:02x}")).collect();
        stream.write_all(format!("\0AUTH EXTERNAL {hex}\r\n").as_bytes())?;
        let mut line = Vec::new();
        let mut b = [0u8; 1];
        while !line.ends_with(b"\r\n") {
            stream.read_exact(&mut b)?;
            line.push(b[0]);
        }
        if !line.starts_with(b"OK") {
            return err("bus rejected authentication");
        }
        stream.write_all(b"BEGIN\r\n")?;
        let writer = Arc::new(Mutex::new(stream.try_clone()?));
        let mut c = Connection {
            stream,
            writer,
            serial: Arc::new(AtomicU32::new(0)),
            unique_name: String::new(),
        };
        let reply = c.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "Hello",
            "",
            &[],
        )?;
        c.unique_name = Reader::new(&reply.body).string()?;
        Ok(c)
    }

    /// A second handle on the same socket (e.g. to shut it down from another thread).
    pub fn try_clone_stream(&self) -> Result<UnixStream> {
        Ok(self.stream.try_clone()?)
    }

    fn next_serial(&mut self) -> u32 {
        self.serial.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn send(&self, msg: &[u8]) -> Result<()> {
        let mut w = self
            .writer
            .lock()
            .map_err(|_| DbusError("writer poisoned".into()))?;
        Ok(w.write_all(msg)?)
    }

    /// A cloneable handle for sending signals from other threads.
    pub fn emitter(&self) -> Emitter {
        Emitter {
            writer: self.writer.clone(),
            serial: self.serial.clone(),
        }
    }

    /// Read one message.
    pub fn read(&mut self) -> Result<Message> {
        let mut head = [0u8; 16];
        self.stream.read_exact(&mut head)?;
        let body_len = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
        let fields_len = u32::from_le_bytes(head[12..16].try_into().unwrap()) as usize;
        let pad = (8 - (16 + fields_len) % 8) % 8;
        let total = 16 + fields_len + pad + body_len;
        if total > 1 << 24 {
            return err("message too large");
        }
        let mut raw = head.to_vec();
        raw.resize(total, 0);
        self.stream.read_exact(&mut raw[16..])?;
        decode(&raw)
    }

    /// Send a method call and wait for its reply, ignoring unrelated traffic.
    pub fn call(
        &mut self,
        dest: &str,
        path: &str,
        iface: &str,
        member: &str,
        sig: &str,
        body: &[u8],
    ) -> Result<Message> {
        let serial = self.next_serial();
        let msg = encode(
            1,
            serial,
            &[
                (1, "o", path),
                (2, "s", iface),
                (3, "s", member),
                (6, "s", dest),
            ],
            &[],
            sig,
            body,
        );
        self.send(&msg)?;
        loop {
            let m = self.read()?;
            if m.reply_serial == Some(serial) {
                return match m.kind {
                    Kind::Error => err(&format!("{dest}.{member} failed")),
                    _ => Ok(m),
                };
            }
        }
    }

    /// Reply to a method call.
    pub fn reply(&mut self, to: &Message, sig: &str, body: &[u8]) -> Result<()> {
        let serial = self.next_serial();
        let mut fields = Vec::new();
        if let Some(s) = &to.sender {
            fields.push((6u8, "s", s.as_str()));
        }
        let msg = encode(2, serial, &fields, &[(5, to.serial)], sig, body);
        self.send(&msg)
    }

    /// Reply with an error.
    pub fn reply_error(&mut self, to: &Message, name: &str, text: &str) -> Result<()> {
        let serial = self.next_serial();
        let mut w = Writer::default();
        w.string(text);
        let mut fields = vec![(4u8, "s", name)];
        if let Some(s) = &to.sender {
            fields.push((6u8, "s", s.as_str()));
        }
        let msg = encode(3, serial, &fields, &[(5, to.serial)], "s", &w.buf);
        self.send(&msg)
    }
}

/// Sends signals on a connection from any thread. Messages are written whole
/// under a lock, so they never interleave with the connection's own replies.
#[derive(Clone)]
pub struct Emitter {
    writer: Arc<Mutex<UnixStream>>,
    serial: Arc<AtomicU32>,
}

impl Emitter {
    /// Broadcast a signal (no destination) with the given body.
    pub fn signal(
        &self,
        path: &str,
        iface: &str,
        member: &str,
        sig: &str,
        body: &[u8],
    ) -> Result<()> {
        let serial = self.serial.fetch_add(1, Ordering::Relaxed) + 1;
        let msg = encode(
            4,
            serial,
            &[(1, "o", path), (2, "s", iface), (3, "s", member)],
            &[],
            sig,
            body,
        );
        let mut w = self
            .writer
            .lock()
            .map_err(|_| DbusError("writer poisoned".into()))?;
        Ok(w.write_all(&msg)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let mut body = Writer::default();
        body.string("hello");
        let raw = encode(
            1,
            7,
            &[
                (1, "o", "/a/b"),
                (2, "s", "x.Y"),
                (3, "s", "Do"),
                (6, "s", "dest.name"),
            ],
            &[],
            "s",
            &body.buf,
        );
        assert_eq!((raw.len() - body.buf.len()) % 8, 0); // body starts 8-aligned
        let m = decode(&raw).unwrap();
        assert_eq!(m.kind, Kind::Call);
        assert_eq!(m.serial, 7);
        assert_eq!(m.path.as_deref(), Some("/a/b"));
        assert_eq!(m.interface.as_deref(), Some("x.Y"));
        assert_eq!(m.member.as_deref(), Some("Do"));
        assert_eq!(m.signature, "s");
        assert_eq!(Reader::new(&m.body).string().unwrap(), "hello");
    }

    #[test]
    fn reply_serial_field() {
        let raw = encode(2, 3, &[], &[(5, 99)], "", &[]);
        let m = decode(&raw).unwrap();
        assert_eq!(m.kind, Kind::Return);
        assert_eq!(m.reply_serial, Some(99));
    }

    #[test]
    fn array_alignment() {
        // Struct arrays pad to 8 even when empty.
        let mut w = Writer::default();
        w.u8(1);
        w.array(8, |_| {});
        assert_eq!(w.buf.len(), 8);
        assert_eq!(&w.buf[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn truncated_rejected() {
        assert!(decode(b"l\x01\x00\x01").is_err());
    }
}
