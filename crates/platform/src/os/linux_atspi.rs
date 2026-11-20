//! Linux accessibility: an AT-SPI2 server over D-Bus, on the crate's own
//! D-Bus client (`dbus.rs`). Written from the AT-SPI2 interface definitions
//! (`org.a11y.atspi.*`); no AT-SPI or D-Bus crate is used.
//!
//! One process-wide service owns one connection to the accessibility bus
//! (found through `org.a11y.Bus.GetAddress` on the session bus). It registers
//! the application with the registry (`Socket.Embed`) and then answers the
//! calls screen readers make: the `Accessible`, `Component`, `Action`,
//! `Value`, `Text` and `EditableText` interfaces over a root object (the
//! application), one object per window and one per node. The UI thread
//! publishes a new [`AccessTree`] per frame; the service keeps the latest
//! one and emits `ChildrenChanged`, `StateChanged`, `PropertyChange` and
//! `TextChanged` signals for what differs, so a screen reader hears only
//! changes. Method calls become [`AccessRequest`]s on the window's sink.
//!
//! The connection is made on a background thread, so a missing or slow
//! accessibility bus never delays the first frame; trees published before it
//! is up are simply kept for when clients ask.

use super::dbus::{Connection, Emitter, Kind, Message, Reader, Writer};
use crate::access::{
    AccessAction, AccessAdapter, AccessId, AccessNode, AccessRequest, AccessRole, AccessTree,
    ActionSink,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

const PREFIX: &str = "/org/a11y/atspi/accessible";
const NULL_PATH: &str = "/org/a11y/atspi/null";
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
const IFACE: &str = "org.a11y.atspi.";

// AT-SPI role numbers (AtspiRole).
const ROLE_CHECK_BOX: u32 = 7;
const ROLE_COMBO_BOX: u32 = 11;
const ROLE_DATE_EDITOR: u32 = 12;
const ROLE_FRAME: u32 = 23;
const ROLE_LABEL: u32 = 29;
const ROLE_PAGE_TAB: u32 = 37;
const ROLE_PANEL: u32 = 39;
const ROLE_PROGRESS_BAR: u32 = 42;
const ROLE_PUSH_BUTTON: u32 = 43;
const ROLE_RADIO_BUTTON: u32 = 44;
const ROLE_SLIDER: u32 = 51;
const ROLE_SPIN_BUTTON: u32 = 52;
const ROLE_TEXT: u32 = 61;
const ROLE_TOGGLE_BUTTON: u32 = 62;
const ROLE_APPLICATION: u32 = 75;
const ROLE_ENTRY: u32 = 79;

// AT-SPI state bit numbers (AtspiStateType).
const ST_ACTIVE: u32 = 1;
const ST_CHECKED: u32 = 4;
const ST_EDITABLE: u32 = 7;
const ST_ENABLED: u32 = 8;
const ST_FOCUSABLE: u32 = 11;
const ST_FOCUSED: u32 = 12;
const ST_MULTI_LINE: u32 = 17;
const ST_SENSITIVE: u32 = 24;
const ST_SHOWING: u32 = 25;
const ST_SINGLE_LINE: u32 = 26;
const ST_VISIBLE: u32 = 30;

/// What the service needs to know about the window's place on screen.
pub type Origin = Arc<dyn Fn() -> ScreenOrigin + Send + Sync>;

/// Window content origin on screen (physical pixels) and scale factor.
#[derive(Debug, Clone, Copy)]
pub struct ScreenOrigin {
    /// Left edge of the window content.
    pub x: i32,
    /// Top edge of the window content.
    pub y: i32,
    /// Physical pixels per logical pixel.
    pub scale: f64,
}

struct WindowState {
    tree: AccessTree,
    origin: Origin,
    sink: ActionSink,
}

#[derive(Default)]
struct Shared {
    /// Our unique name on the accessibility bus; empty until connected.
    bus_name: String,
    /// The registry's root, from `Embed`; the parent of our root.
    registry: (String, String),
    app_name: String,
    windows: BTreeMap<u64, WindowState>,
    emitter: Option<Emitter>,
}

static SERVICE: OnceLock<Arc<Mutex<Shared>>> = OnceLock::new();

fn service() -> Arc<Mutex<Shared>> {
    SERVICE
        .get_or_init(|| {
            let shared = Arc::new(Mutex::new(Shared {
                app_name: std::env::current_exe()
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "alloy".into()),
                ..Shared::default()
            }));
            let s = shared.clone();
            let spawned = std::thread::Builder::new()
                .name("alloy-atspi".into())
                .spawn(move || {
                    if let Err(e) = serve(s) {
                        log::info!("accessibility bus unavailable: {}", e.0);
                    }
                });
            if let Err(e) = spawned {
                log::warn!("atspi thread: {e}");
            }
            shared
        })
        .clone()
}

/// The adapter for one window. Cheap: all state lives in the shared service.
pub struct AtspiAdapter {
    key: u64,
    shared: Arc<Mutex<Shared>>,
}

impl AtspiAdapter {
    /// Register window `key` with the accessibility service, starting the
    /// service on first use.
    pub fn new(key: u64, origin: Origin, sink: ActionSink) -> Self {
        let shared = service();
        if let Ok(mut s) = shared.lock() {
            s.windows.insert(
                key,
                WindowState {
                    tree: AccessTree::default(),
                    origin,
                    sink,
                },
            );
        }
        Self { key, shared }
    }
}

impl Drop for AtspiAdapter {
    fn drop(&mut self) {
        let Ok(mut s) = self.shared.lock() else {
            return;
        };
        if s.windows.remove(&self.key).is_some() {
            let idx = s.windows.range(..self.key).count() as i32;
            let (name, em) = (s.bus_name.clone(), s.emitter.clone());
            if let Some(em) = em {
                emit_children(&em, ROOT_PATH, "remove", idx, &name, &window_path(self.key));
            }
        }
    }
}

impl AccessAdapter for AtspiAdapter {
    fn update(&mut self, tree: &AccessTree) {
        let Ok(mut s) = self.shared.lock() else {
            return;
        };
        let name = s.bus_name.clone();
        let em = s.emitter.clone();
        let idx = s.windows.range(..self.key).count() as i32;
        let Some(w) = s.windows.get_mut(&self.key) else {
            return;
        };
        let old = std::mem::replace(&mut w.tree, tree.clone());
        let Some(em) = em else {
            return; // not connected yet: clients will read the stored tree
        };
        let first = old.nodes.is_empty() && old.title.is_empty();
        let wpath = window_path(self.key);
        if first {
            emit_children(&em, ROOT_PATH, "add", idx, &name, &wpath);
            emit_window(&em, &wpath, "Create");
            emit_window(&em, &wpath, "Activate");
            return;
        }
        if old.title != tree.title {
            emit_prop_str(&em, &wpath, "accessible-name", &tree.title);
        }
        let diff = old.diff(tree);
        for id in &diff.removed {
            let i = old.nodes.iter().position(|n| n.id == *id).unwrap_or(0) as i32;
            emit_children(&em, &wpath, "remove", i, &name, &node_path(self.key, *id));
        }
        for id in &diff.added {
            let i = tree.nodes.iter().position(|n| n.id == *id).unwrap_or(0) as i32;
            emit_children(&em, &wpath, "add", i, &name, &node_path(self.key, *id));
        }
        for id in &diff.changed {
            let (Some(o), Some(n)) = (old.get(*id), tree.get(*id)) else {
                continue;
            };
            let path = node_path(self.key, *id);
            if o.label != n.label {
                emit_prop_str(&em, &path, "accessible-name", &n.label);
            }
            if o.value != n.value {
                match n.role {
                    AccessRole::TextInput | AccessRole::TextEditor => {
                        let old_len = o.value.as_deref().unwrap_or("").chars().count() as i32;
                        let new = n.value.clone().unwrap_or_default();
                        if old_len > 0 {
                            emit_text(
                                &em,
                                &path,
                                "delete",
                                0,
                                old_len,
                                &o.value.clone().unwrap_or_default(),
                            );
                        }
                        if !new.is_empty() {
                            emit_text(&em, &path, "insert", 0, new.chars().count() as i32, &new);
                        }
                    }
                    _ => emit_prop_f64(&em, &path, "accessible-value", number(n)),
                }
            }
            if o.checked != n.checked {
                emit_state(&em, &path, "checked", n.checked == Some(true));
            }
            if o.enabled != n.enabled {
                emit_state(&em, &path, "enabled", n.enabled);
                emit_state(&em, &path, "sensitive", n.enabled);
            }
            if o.focused != n.focused {
                emit_state(&em, &path, "focused", n.focused);
            }
        }
    }
}

// ----- signal emission ------------------------------------------------------

fn event_body(kind: &str, d1: i32, d2: i32, any: impl FnOnce(&mut Writer)) -> Writer {
    let mut w = Writer::default();
    w.string(kind);
    w.i32(d1);
    w.i32(d2);
    any(&mut w);
    w.array(8, |_| {}); // properties: empty a{sv}
    w
}

const EV_OBJECT: &str = "org.a11y.atspi.Event.Object";

fn emit_children(em: &Emitter, path: &str, kind: &str, index: i32, name: &str, child: &str) {
    let b = event_body(kind, index, 0, |w| {
        w.variant("(so)", |w| w.reference(name, child))
    });
    let _ = em.signal(path, EV_OBJECT, "ChildrenChanged", "siiva{sv}", &b.buf);
}

fn emit_state(em: &Emitter, path: &str, kind: &str, on: bool) {
    let b = event_body(kind, on as i32, 0, |w| w.variant("i", |w| w.i32(0)));
    let _ = em.signal(path, EV_OBJECT, "StateChanged", "siiva{sv}", &b.buf);
}

fn emit_prop_str(em: &Emitter, path: &str, kind: &str, v: &str) {
    let b = event_body(kind, 0, 0, |w| w.variant("s", |w| w.string(v)));
    let _ = em.signal(path, EV_OBJECT, "PropertyChange", "siiva{sv}", &b.buf);
}

fn emit_prop_f64(em: &Emitter, path: &str, kind: &str, v: f64) {
    let b = event_body(kind, 0, 0, |w| w.variant("d", |w| w.f64(v)));
    let _ = em.signal(path, EV_OBJECT, "PropertyChange", "siiva{sv}", &b.buf);
}

fn emit_text(em: &Emitter, path: &str, kind: &str, at: i32, len: i32, text: &str) {
    let b = event_body(kind, at, len, |w| w.variant("s", |w| w.string(text)));
    let _ = em.signal(path, EV_OBJECT, "TextChanged", "siiva{sv}", &b.buf);
}

fn emit_window(em: &Emitter, path: &str, kind: &str) {
    let b = event_body("", 0, 0, |w| w.variant("i", |w| w.i32(0)));
    let _ = em.signal(
        path,
        "org.a11y.atspi.Event.Window",
        kind,
        "siiva{sv}",
        &b.buf,
    );
}

// ----- object paths ---------------------------------------------------------

fn window_path(key: u64) -> String {
    format!("{PREFIX}/w{key}")
}

fn node_path(key: u64, id: AccessId) -> String {
    format!("{PREFIX}/w{key}n{}", id.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Obj {
    Root,
    Window(u64),
    Node(u64, AccessId),
}

fn parse_path(p: &str) -> Option<Obj> {
    let rest = p.strip_prefix(PREFIX)?.strip_prefix('/')?;
    if rest == "root" {
        return Some(Obj::Root);
    }
    let rest = rest.strip_prefix('w')?;
    match rest.split_once('n') {
        Some((k, n)) => Some(Obj::Node(k.parse().ok()?, AccessId(n.parse().ok()?))),
        None => Some(Obj::Window(rest.parse().ok()?)),
    }
}

// ----- role and state mapping ----------------------------------------------

fn atspi_role(r: AccessRole) -> u32 {
    match r {
        AccessRole::Label => ROLE_LABEL,
        AccessRole::Button => ROLE_PUSH_BUTTON,
        AccessRole::Checkbox => ROLE_CHECK_BOX,
        AccessRole::Switch => ROLE_TOGGLE_BUTTON,
        AccessRole::Radio => ROLE_RADIO_BUTTON,
        AccessRole::Slider => ROLE_SLIDER,
        AccessRole::SpinButton => ROLE_SPIN_BUTTON,
        AccessRole::TextInput => ROLE_ENTRY,
        AccessRole::TextEditor => ROLE_TEXT,
        AccessRole::ComboBox => ROLE_COMBO_BOX,
        AccessRole::DatePicker => ROLE_DATE_EDITOR,
        AccessRole::Tab => ROLE_PAGE_TAB,
        AccessRole::ProgressBar => ROLE_PROGRESS_BAR,
        AccessRole::Group => ROLE_PANEL,
    }
}

fn role_name(r: AccessRole) -> &'static str {
    match r {
        AccessRole::Label => "label",
        AccessRole::Button => "push button",
        AccessRole::Checkbox => "check box",
        AccessRole::Switch => "toggle button",
        AccessRole::Radio => "radio button",
        AccessRole::Slider => "slider",
        AccessRole::SpinButton => "spin button",
        AccessRole::TextInput => "entry",
        AccessRole::TextEditor => "text",
        AccessRole::ComboBox => "combo box",
        AccessRole::DatePicker => "date editor",
        AccessRole::Tab => "page tab",
        AccessRole::ProgressBar => "progress bar",
        AccessRole::Group => "panel",
    }
}

fn is_text(r: AccessRole) -> bool {
    matches!(r, AccessRole::TextInput | AccessRole::TextEditor)
}

fn is_range(r: AccessRole) -> bool {
    matches!(
        r,
        AccessRole::Slider | AccessRole::SpinButton | AccessRole::ProgressBar
    )
}

fn has_action(r: AccessRole) -> bool {
    matches!(
        r,
        AccessRole::Button
            | AccessRole::Checkbox
            | AccessRole::Switch
            | AccessRole::Radio
            | AccessRole::Tab
            | AccessRole::ComboBox
            | AccessRole::DatePicker
    )
}

fn focusable(r: AccessRole) -> bool {
    !matches!(
        r,
        AccessRole::Label | AccessRole::ProgressBar | AccessRole::Group
    )
}

fn action_name(r: AccessRole) -> &'static str {
    match r {
        AccessRole::Checkbox | AccessRole::Switch | AccessRole::Radio => "toggle",
        AccessRole::Tab => "select",
        AccessRole::ComboBox | AccessRole::DatePicker => "open",
        _ => "click",
    }
}

fn state_words(bits: &[u32]) -> [u32; 2] {
    let mut s = [0u32; 2];
    for b in bits {
        s[(*b / 32) as usize] |= 1 << (b % 32);
    }
    s
}

fn node_states(n: &AccessNode) -> [u32; 2] {
    let mut b = vec![ST_VISIBLE, ST_SHOWING];
    if n.enabled {
        b.extend([ST_ENABLED, ST_SENSITIVE]);
        if focusable(n.role) {
            b.push(ST_FOCUSABLE);
        }
    }
    if n.focused {
        b.push(ST_FOCUSED);
    }
    if n.checked == Some(true) {
        b.push(ST_CHECKED);
    }
    if is_text(n.role) {
        b.push(ST_EDITABLE);
        b.push(if n.role == AccessRole::TextInput {
            ST_SINGLE_LINE
        } else {
            ST_MULTI_LINE
        });
    }
    state_words(&b)
}

fn number(n: &AccessNode) -> f64 {
    n.value
        .as_deref()
        .and_then(|v| v.trim().trim_end_matches('%').parse().ok())
        .unwrap_or(0.0)
}

fn range_of(n: &AccessNode) -> (f64, f64, f64) {
    n.range
        .map_or((0.0, 100.0, 1.0), |r| (r.min, r.max, r.step))
}

// ----- the service ---------------------------------------------------------

type Res<T> = Result<T, super::dbus::DbusError>;

fn serve(shared: Arc<Mutex<Shared>>) -> Res<()> {
    // Ask the session bus where the accessibility bus is.
    let mut session = Connection::session()?;
    let reply = session.call(
        "org.a11y.Bus",
        "/org/a11y/bus",
        "org.a11y.Bus",
        "GetAddress",
        "",
        &[],
    )?;
    let addr = Reader::new(&reply.body).string()?;
    drop(session);
    let mut conn = Connection::open(&addr)?;
    let me = conn.unique_name.clone();
    {
        let mut s = lock(&shared);
        s.bus_name = me.clone();
        s.emitter = Some(conn.emitter());
    }
    // Tell the registry we exist; it answers with its own root reference.
    let mut w = Writer::default();
    w.reference(&me, ROOT_PATH);
    let r = conn.call(
        "org.a11y.atspi.Registry",
        ROOT_PATH,
        "org.a11y.atspi.Socket",
        "Embed",
        "(so)",
        &w.buf,
    )?;
    {
        let mut rd = Reader::new(&r.body);
        rd.align(8);
        if let (Ok(n), Ok(p)) = (rd.string(), rd.string()) {
            lock(&shared).registry = (n, p);
        }
    }
    // Windows published before we were up still need announcing.
    {
        let s = lock(&shared);
        if let Some(em) = &s.emitter {
            for (i, (k, w)) in s.windows.iter().enumerate() {
                if !w.tree.nodes.is_empty() || !w.tree.title.is_empty() {
                    let p = window_path(*k);
                    emit_children(em, ROOT_PATH, "add", i as i32, &me, &p);
                    emit_window(em, &p, "Activate");
                }
            }
        }
    }
    loop {
        let m = conn.read()?;
        if m.kind != Kind::Call {
            continue;
        }
        let out = {
            let s = lock(&shared);
            dispatch(&s, &m)
        };
        match out {
            Ok((sig, body)) => conn.reply(&m, &sig, &body)?,
            Err((name, text)) => conn.reply_error(&m, name, &text)?,
        }
    }
}

fn lock(s: &Arc<Mutex<Shared>>) -> std::sync::MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|p| p.into_inner())
}

type Out = std::result::Result<(String, Vec<u8>), (&'static str, String)>;

fn ok(sig: &str, f: impl FnOnce(&mut Writer)) -> Out {
    let mut w = Writer::default();
    f(&mut w);
    Ok((sig.to_string(), w.buf))
}

fn unknown(what: &str) -> Out {
    Err(("org.freedesktop.DBus.Error.UnknownMethod", what.to_string()))
}

fn no_object(p: &str) -> Out {
    Err(("org.freedesktop.DBus.Error.UnknownObject", p.to_string()))
}

/// A property value, enough types for the interfaces we serve.
enum V {
    S(String),
    O(String, String),
    I(i32),
    D(f64),
}

impl V {
    fn sig(&self) -> &'static str {
        match self {
            V::S(_) => "s",
            V::O(..) => "(so)",
            V::I(_) => "i",
            V::D(_) => "d",
        }
    }
    fn write(&self, w: &mut Writer) {
        match self {
            V::S(s) => w.string(s),
            V::O(n, p) => w.reference(n, p),
            V::I(i) => w.i32(*i),
            V::D(d) => w.f64(*d),
        }
    }
}

struct Ctx<'a> {
    s: &'a Shared,
    obj: Obj,
}

impl Ctx<'_> {
    fn me(&self) -> &str {
        &self.s.bus_name
    }
    fn win(&self) -> Option<(&u64, &WindowState)> {
        match self.obj {
            Obj::Window(k) | Obj::Node(k, _) => self.s.windows.get_key_value(&k),
            Obj::Root => None,
        }
    }
    fn node(&self) -> Option<&AccessNode> {
        match self.obj {
            Obj::Node(k, id) => self.s.windows.get(&k)?.tree.get(id),
            _ => None,
        }
    }
    fn reference(&self, o: Obj) -> V {
        let p = match o {
            Obj::Root => ROOT_PATH.to_string(),
            Obj::Window(k) => window_path(k),
            Obj::Node(k, id) => node_path(k, id),
        };
        V::O(self.me().to_string(), p)
    }
    fn parent(&self) -> V {
        match self.obj {
            Obj::Root => V::O(self.s.registry.0.clone(), self.s.registry.1.clone()),
            Obj::Window(_) => self.reference(Obj::Root),
            Obj::Node(k, _) => self.reference(Obj::Window(k)),
        }
    }
    fn children(&self) -> Vec<Obj> {
        match self.obj {
            Obj::Root => self.s.windows.keys().map(|k| Obj::Window(*k)).collect(),
            Obj::Window(k) => self
                .s
                .windows
                .get(&k)
                .map(|w| w.tree.nodes.iter().map(|n| Obj::Node(k, n.id)).collect())
                .unwrap_or_default(),
            Obj::Node(..) => Vec::new(),
        }
    }
    fn name(&self) -> String {
        match self.obj {
            Obj::Root => self.s.app_name.clone(),
            Obj::Window(k) => self
                .s
                .windows
                .get(&k)
                .map(|w| w.tree.title.clone())
                .unwrap_or_default(),
            Obj::Node(..) => self.node().map(|n| n.label.clone()).unwrap_or_default(),
        }
    }
    fn role(&self) -> u32 {
        match self.obj {
            Obj::Root => ROLE_APPLICATION,
            Obj::Window(_) => ROLE_FRAME,
            Obj::Node(..) => self.node().map_or(ROLE_PANEL, |n| atspi_role(n.role)),
        }
    }
    fn role_name(&self) -> &'static str {
        match self.obj {
            Obj::Root => "application",
            Obj::Window(_) => "frame",
            Obj::Node(..) => self.node().map_or("panel", |n| role_name(n.role)),
        }
    }
    fn interfaces(&self) -> Vec<&'static str> {
        let mut v = vec!["Accessible"];
        match self.obj {
            Obj::Root => v.push("Application"),
            Obj::Window(_) => v.push("Component"),
            Obj::Node(..) => {
                v.push("Component");
                if let Some(n) = self.node() {
                    if has_action(n.role) {
                        v.push("Action");
                    }
                    if is_range(n.role) {
                        v.push("Value");
                    }
                    if is_text(n.role) {
                        v.push("Text");
                        v.push("EditableText");
                    }
                }
            }
        }
        v
    }
    fn states(&self) -> [u32; 2] {
        match self.obj {
            Obj::Root => state_words(&[ST_VISIBLE, ST_SHOWING]),
            Obj::Window(_) => {
                state_words(&[ST_ENABLED, ST_SENSITIVE, ST_VISIBLE, ST_SHOWING, ST_ACTIVE])
            }
            Obj::Node(..) => self.node().map(node_states).unwrap_or_default(),
        }
    }
    /// Extents in the requested coordinate space (0 screen, 1 window, 2 parent).
    fn extents(&self, coord: u32) -> (i32, i32, i32, i32) {
        let Some((_, w)) = self.win() else {
            return (0, 0, 0, 0);
        };
        let o = (w.origin)();
        let scale = if o.scale > 0.0 { o.scale } else { 1.0 };
        let r = match self.node() {
            Some(n) => n.bounds,
            None => {
                // The window itself: the union of its nodes.
                let (mut x1, mut y1) = (0f32, 0f32);
                for n in &w.tree.nodes {
                    x1 = x1.max(n.bounds.origin.x + n.bounds.size.width);
                    y1 = y1.max(n.bounds.origin.y + n.bounds.size.height);
                }
                alloy_ui_core::Rectangle {
                    origin: alloy_ui_core::Point { x: 0.0, y: 0.0 },
                    size: alloy_ui_core::Size {
                        width: x1,
                        height: y1,
                    },
                }
            }
        };
        let (ox, oy) = if coord == 0 { (o.x, o.y) } else { (0, 0) };
        // Parent coordinates equal window coordinates: every node is a direct child.
        (
            ox + (r.origin.x as f64 * scale).round() as i32,
            oy + (r.origin.y as f64 * scale).round() as i32,
            (r.size.width as f64 * scale).round() as i32,
            (r.size.height as f64 * scale).round() as i32,
        )
    }
    fn request(&self, action: AccessAction) -> bool {
        let (Obj::Node(k, id), Some(w)) = (self.obj, self.win().map(|x| x.1)) else {
            return false;
        };
        let _ = k;
        (w.sink)(AccessRequest { target: id, action });
        true
    }
    fn get_prop(&self, iface: &str, name: &str) -> Option<V> {
        let n = self.node();
        Some(match (iface.strip_prefix(IFACE)?, name) {
            ("Accessible", "Name") => V::S(self.name()),
            ("Accessible", "Description") => V::S(String::new()),
            ("Accessible", "Parent") => self.parent(),
            ("Accessible", "ChildCount") => V::I(self.children().len() as i32),
            ("Accessible", "Locale") => V::S("en_US".into()),
            ("Accessible", "AccessibleId") => V::S(String::new()),
            ("Application", "ToolkitName") => V::S("alloy-ui".into()),
            ("Application", "Version") => V::S(env!("CARGO_PKG_VERSION").into()),
            ("Application", "AtspiVersion") => V::S("2.1".into()),
            ("Application", "Id") => V::I(0),
            ("Value", "MinimumValue") => V::D(range_of(n?).0),
            ("Value", "MaximumValue") => V::D(range_of(n?).1),
            ("Value", "MinimumIncrement") => V::D(range_of(n?).2),
            ("Value", "CurrentValue") => V::D(number(n?)),
            ("Text", "CharacterCount") => {
                V::I(n?.value.as_deref().unwrap_or("").chars().count() as i32)
            }
            ("Text", "CaretOffset") => {
                V::I(n?.value.as_deref().unwrap_or("").chars().count() as i32)
            }
            ("Action", "NActions") => V::I(1),
            _ => return None,
        })
    }
    fn props_of(&self, iface: &str) -> Vec<(&'static str, V)> {
        let names: &[&'static str] = match iface.strip_prefix(IFACE) {
            Some("Accessible") => &[
                "Name",
                "Description",
                "Parent",
                "ChildCount",
                "Locale",
                "AccessibleId",
            ],
            Some("Application") => &["ToolkitName", "Version", "AtspiVersion", "Id"],
            Some("Value") => &[
                "MinimumValue",
                "MaximumValue",
                "MinimumIncrement",
                "CurrentValue",
            ],
            Some("Text") => &["CharacterCount", "CaretOffset"],
            Some("Action") => &["NActions"],
            _ => &[],
        };
        names
            .iter()
            .filter_map(|n| self.get_prop(iface, n).map(|v| (*n, v)))
            .collect()
    }
}

fn dispatch(s: &Shared, m: &Message) -> Out {
    let path = m.path.as_deref().unwrap_or("");
    // We keep no cache; clients fall back to asking for each object.
    if path == "/org/a11y/atspi/cache" && m.member.as_deref() == Some("GetItems") {
        return ok("a((so)(so)(so)iiassusau)", |w| w.array(8, |_| {}));
    }
    let Some(obj) = parse_path(path) else {
        return no_object(path);
    };
    let c = Ctx { s, obj };
    // The object must exist (nodes and windows come and go).
    match obj {
        Obj::Window(k) if !s.windows.contains_key(&k) => return no_object(path),
        Obj::Node(..) if c.node().is_none() => return no_object(path),
        _ => {}
    }
    let iface = m.interface.as_deref().unwrap_or("");
    let member = m.member.as_deref().unwrap_or("");
    let mut r = Reader::new(&m.body);
    let bad = |_| {
        (
            "org.freedesktop.DBus.Error.InvalidArgs",
            "bad arguments".to_string(),
        )
    };

    match (iface, member) {
        ("org.freedesktop.DBus.Introspectable", "Introspect") => {
            return ok("s", |w| w.string("<node/>"))
        }
        ("org.freedesktop.DBus.Properties", "Get") => {
            let (i, n) = (r.string().map_err(bad)?, r.string().map_err(bad)?);
            return match c.get_prop(&i, &n) {
                Some(v) => ok("v", |w| w.variant(v.sig(), |w| v.write(w))),
                None => Err((
                    "org.freedesktop.DBus.Error.UnknownProperty",
                    format!("{i}.{n}"),
                )),
            };
        }
        ("org.freedesktop.DBus.Properties", "GetAll") => {
            let i = r.string().map_err(bad)?;
            let props = c.props_of(&i);
            return ok("a{sv}", |w| {
                w.array(8, |w| {
                    for (n, v) in &props {
                        w.align(8);
                        w.string(n);
                        w.variant(v.sig(), |w| v.write(w));
                    }
                })
            });
        }
        ("org.freedesktop.DBus.Properties", "Set") => {
            let (i, n) = (r.string().map_err(bad)?, r.string().map_err(bad)?);
            if i == "org.a11y.atspi.Value" && n == "CurrentValue" {
                let sig = r.signature().map_err(bad)?;
                let target = r.f64().map_err(bad)?;
                if sig == "d" {
                    if let Some(node) = c.node() {
                        let cur = number(node);
                        if target > cur {
                            c.request(AccessAction::Increment);
                        } else if target < cur {
                            c.request(AccessAction::Decrement);
                        }
                        return ok("", |_| {});
                    }
                }
            }
            return Err((
                "org.freedesktop.DBus.Error.PropertyReadOnly",
                format!("{i}.{n}"),
            ));
        }
        _ => {}
    }

    let Some(short) = iface.strip_prefix(IFACE) else {
        return unknown(iface);
    };
    let node = c.node();
    match (short, member) {
        // --- Accessible
        ("Accessible", "GetChildAtIndex") => {
            let i = r.i32().map_err(bad)?;
            let ch = c.children();
            match usize::try_from(i).ok().and_then(|i| ch.get(i)) {
                Some(o) => {
                    let v = c.reference(*o);
                    ok("(so)", |w| v.write(w))
                }
                None => Err((
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "no such child".into(),
                )),
            }
        }
        ("Accessible", "GetChildren") => {
            let refs: Vec<V> = c.children().into_iter().map(|o| c.reference(o)).collect();
            ok("a(so)", |w| {
                w.array(8, |w| {
                    for v in &refs {
                        v.write(w);
                    }
                })
            })
        }
        ("Accessible", "GetIndexInParent") => {
            let idx = match obj {
                Obj::Root => 0,
                Obj::Window(k) => s.windows.range(..k).count(),
                Obj::Node(k, id) => s
                    .windows
                    .get(&k)
                    .and_then(|w| w.tree.nodes.iter().position(|n| n.id == id))
                    .unwrap_or(0),
            };
            ok("i", |w| w.i32(idx as i32))
        }
        ("Accessible", "GetRelationSet") => ok("a(ua(so))", |w| w.array(8, |_| {})),
        ("Accessible", "GetRole") => ok("u", |w| w.u32(c.role())),
        ("Accessible", "GetRoleName") | ("Accessible", "GetLocalizedRoleName") => {
            ok("s", |w| w.string(c.role_name()))
        }
        ("Accessible", "GetState") => {
            let st = c.states();
            ok("au", |w| {
                w.array(4, |w| {
                    w.u32(st[0]);
                    w.u32(st[1]);
                })
            })
        }
        ("Accessible", "GetAttributes") => ok("a{ss}", |w| w.array(8, |_| {})),
        ("Accessible", "GetApplication") => {
            let v = c.reference(Obj::Root);
            ok("(so)", |w| v.write(w))
        }
        ("Accessible", "GetInterfaces") => {
            let list = c.interfaces();
            ok("as", |w| {
                w.array(4, |w| {
                    for i in &list {
                        w.string(&format!("{IFACE}{i}"));
                    }
                })
            })
        }
        // --- Application
        ("Application", "GetLocale") => ok("s", |w| w.string("en_US")),
        ("Application", "GetApplicationBusAddress") => ok("s", |w| w.string("")),
        // --- Component
        ("Component", "GetExtents") => {
            let (x, y, wd, h) = c.extents(r.u32().map_err(bad)?);
            ok("(iiii)", |w| {
                w.align(8);
                w.i32(x);
                w.i32(y);
                w.i32(wd);
                w.i32(h);
            })
        }
        ("Component", "GetPosition") => {
            let (x, y, ..) = c.extents(r.u32().map_err(bad)?);
            ok("(ii)", |w| {
                w.align(8);
                w.i32(x);
                w.i32(y);
            })
        }
        ("Component", "GetSize") => {
            let (_, _, wd, h) = c.extents(1);
            ok("(ii)", |w| {
                w.align(8);
                w.i32(wd);
                w.i32(h);
            })
        }
        ("Component", "Contains") => {
            let (px, py, coord) = (
                r.i32().map_err(bad)?,
                r.i32().map_err(bad)?,
                r.u32().map_err(bad)?,
            );
            let (x, y, wd, h) = c.extents(coord);
            ok("b", |w| {
                w.bool(px >= x && py >= y && px < x + wd && py < y + h)
            })
        }
        ("Component", "GetAccessibleAtPoint") => {
            let (px, py, coord) = (
                r.i32().map_err(bad)?,
                r.i32().map_err(bad)?,
                r.u32().map_err(bad)?,
            );
            let hit = match obj {
                Obj::Window(k) => s.windows.get(&k).and_then(|w| {
                    // Later nodes are drawn on top.
                    w.tree.nodes.iter().rev().find_map(|n| {
                        let cc = Ctx {
                            s,
                            obj: Obj::Node(k, n.id),
                        };
                        let (x, y, wd, h) = cc.extents(coord);
                        (px >= x && py >= y && px < x + wd && py < y + h).then_some(cc.obj)
                    })
                }),
                _ => None,
            };
            let v = hit.map_or(V::O(String::new(), NULL_PATH.into()), |o| c.reference(o));
            ok("(so)", |w| v.write(w))
        }
        ("Component", "GetLayer") => ok("u", |w| w.u32(3)), // widget layer
        ("Component", "GetMDIZOrder") => ok("n", |w| w.i16(0)),
        ("Component", "GetAlpha") => ok("d", |w| w.f64(1.0)),
        ("Component", "GrabFocus") => {
            let done = c.request(AccessAction::Focus);
            ok("b", |w| w.bool(done))
        }
        // --- Action
        ("Action", "GetNActions") => ok("i", |w| w.i32(1)),
        ("Action", "GetName") | ("Action", "GetLocalizedName") => {
            let n = node.map_or("click", |n| action_name(n.role));
            ok("s", |w| w.string(n))
        }
        ("Action", "GetDescription") | ("Action", "GetKeyBinding") => ok("s", |w| w.string("")),
        ("Action", "GetActions") => {
            let n = node.map_or("click", |n| action_name(n.role));
            ok("a(sss)", |w| {
                w.array(8, |w| {
                    w.align(8);
                    w.string(n);
                    w.string("");
                    w.string("");
                })
            })
        }
        ("Action", "DoAction") => {
            let i = r.i32().map_err(bad)?;
            let done = i == 0 && node.is_some_and(|n| n.enabled) && c.request(AccessAction::Press);
            ok("b", |w| w.bool(done))
        }
        // --- Text / EditableText
        ("Text", "GetText") => {
            let (a, b) = (r.i32().map_err(bad)?, r.i32().map_err(bad)?);
            let t = node.and_then(|n| n.value.as_deref()).unwrap_or("");
            let len = t.chars().count() as i32;
            let (a, b) = (a.clamp(0, len), if b < 0 { len } else { b.clamp(0, len) });
            let sub: String = t
                .chars()
                .skip(a as usize)
                .take((b - a).max(0) as usize)
                .collect();
            ok("s", |w| w.string(&sub))
        }
        ("Text", "GetCaretOffset") => {
            let n = node
                .and_then(|n| n.value.as_deref())
                .map_or(0, |t| t.chars().count());
            ok("i", |w| w.i32(n as i32))
        }
        ("Text", "GetNSelections") => ok("i", |w| w.i32(0)),
        ("EditableText", "SetTextContents") => {
            let t = r.string().map_err(bad)?;
            let done = node.is_some_and(|n| n.enabled) && c.request(AccessAction::SetValue(t));
            ok("b", |w| w.bool(done))
        }
        _ => unknown(&format!("{iface}.{member}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_round_trip() {
        assert_eq!(parse_path(ROOT_PATH), Some(Obj::Root));
        assert_eq!(parse_path(&window_path(3)), Some(Obj::Window(3)));
        assert_eq!(
            parse_path(&node_path(3, AccessId(77))),
            Some(Obj::Node(3, AccessId(77)))
        );
        assert_eq!(parse_path("/other"), None);
    }

    #[test]
    fn state_bits() {
        let w = state_words(&[ST_ENABLED, ST_VISIBLE]);
        assert_eq!(w, [1 << 8 | 1 << 30, 0]);
        let w = state_words(&[33]);
        assert_eq!(w, [0, 2]);
    }
}
