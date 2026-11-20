//! Linux tray via the StatusNotifierItem D-Bus protocol (what KDE, and GNOME
//! with the AppIndicator extension, implement). Built on our own minimal D-Bus
//! client in `dbus.rs`.
//!
//! Supported: icon, tooltip, left-click (`Activate`), and a context menu
//! exported over `com.canonical.dbusmenu` (labels, separators, submenus,
//! disabled items; choosing an item reports `MenuAction(id)`).

use super::dbus::{Connection, Kind, Message, Reader, Writer};
use crate::{
    menu::{Menu, MenuItem},
    tray::TraySettings,
    PlatformError, PlatformEvent,
};
use std::{net::Shutdown, os::unix::net::UnixStream, sync::Mutex};

const ITEM_PATH: &str = "/StatusNotifierItem";
const ITEM_IFACE: &str = "org.kde.StatusNotifierItem";
const PROPS: &str = "org.freedesktop.DBus.Properties";

static RUNNING: Mutex<Option<UnixStream>> = Mutex::new(None);

/// Convert RGBA8 to the ARGB32 network-order bytes the protocol wants.
pub(crate) fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect()
}

const MENU_PATH: &str = "/MenuBar";
const MENU_IFACE: &str = "com.canonical.dbusmenu";

/// One node of the exported menu tree. Id 0 is the invisible root.
struct Node {
    id: i32,
    label: String,
    enabled: bool,
    separator: bool,
    children: Vec<Node>,
}

/// Build the dbusmenu tree, numbering nodes from 1 and recording action ids.
fn build_tree(items: &[MenuItem], next: &mut i32, actions: &mut Vec<(i32, String)>) -> Vec<Node> {
    items
        .iter()
        .map(|item| {
            *next += 1;
            let id = *next;
            match item {
                MenuItem::Separator => Node {
                    id,
                    label: String::new(),
                    enabled: true,
                    separator: true,
                    children: vec![],
                },
                MenuItem::Submenu(m) => Node {
                    id,
                    label: m.title.clone(),
                    enabled: true,
                    separator: false,
                    children: build_tree(&m.items, next, actions),
                },
                MenuItem::Action {
                    id: aid,
                    label,
                    enabled,
                    ..
                } => {
                    actions.push((id, aid.clone()));
                    Node {
                        id,
                        label: label.clone(),
                        enabled: *enabled,
                        separator: false,
                        children: vec![],
                    }
                }
            }
        })
        .collect()
}

/// Dbusmenu labels use `_` as a mnemonic marker; double it to show a literal one.
fn escape_label(s: &str) -> String {
    s.replace('_', "__")
}

/// Write `(ia{sv}av)` for `node`, descending `depth` levels (-1 = all).
fn write_node(w: &mut Writer, node: &Node, depth: i32, root: bool) {
    w.align(8);
    w.i32(node.id);
    w.array(8, |w| {
        let prop = |w: &mut Writer, k: &str, sig: &str, f: &dyn Fn(&mut Writer)| {
            w.align(8);
            w.string(k);
            w.variant(sig, |w| f(w));
        };
        if root {
            prop(w, "children-display", "s", &|w| w.string("submenu"));
        } else if node.separator {
            prop(w, "type", "s", &|w| w.string("separator"));
        } else {
            prop(w, "label", "s", &|w| w.string(&escape_label(&node.label)));
            if !node.enabled {
                prop(w, "enabled", "b", &|w| w.bool(false));
            }
            if !node.children.is_empty() {
                prop(w, "children-display", "s", &|w| w.string("submenu"));
            }
        }
    });
    w.array(1, |w| {
        if depth != 0 {
            for c in &node.children {
                w.variant("(ia{sv}av)", |w| write_node(w, c, depth - 1, false));
            }
        }
    });
}

struct Item {
    menu: Option<Node>,
    actions: Vec<(i32, String)>,
    tooltip: String,
    argb: Vec<u8>,
    w: u32,
    h: u32,
    id: String,
}

impl Item {
    fn pixmap(&self, w: &mut Writer) {
        w.array(8, |w| {
            w.align(8);
            w.i32(self.w as i32);
            w.i32(self.h as i32);
            w.bytes(&self.argb);
        });
    }

    /// Write the variant for property `name`, or None if unknown.
    fn property(&self, name: &str, w: &mut Writer) -> bool {
        match name {
            "Category" => w.variant("s", |w| w.string("ApplicationStatus")),
            "Id" => w.variant("s", |w| w.string(&self.id)),
            "Title" => w.variant("s", |w| w.string(&self.tooltip)),
            "Status" => w.variant("s", |w| w.string("Active")),
            "WindowId" => w.variant("u", |w| w.u32(0)),
            "IconName" => w.variant("s", |w| w.string("")),
            "IconPixmap" => w.variant("a(iiay)", |w| self.pixmap(w)),
            "ToolTip" => w.variant("(sa(iiay)ss)", |w| {
                w.align(8);
                w.string("");
                w.array(8, |_| {});
                w.string(&self.tooltip);
                w.string("");
            }),
            "ItemIsMenu" => w.variant("b", |w| w.bool(false)),
            "Menu" => w.variant("o", |w| {
                w.string(if self.menu.is_some() {
                    MENU_PATH
                } else {
                    "/NO_DMENU"
                })
            }),
            _ => return false,
        }
        true
    }
}

const NAMES: [&str; 10] = [
    "Category",
    "Id",
    "Title",
    "Status",
    "WindowId",
    "IconName",
    "IconPixmap",
    "ToolTip",
    "ItemIsMenu",
    "Menu",
];

const INTROSPECT: &str = r#"<node><interface name="org.kde.StatusNotifierItem">
<method name="Activate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
<method name="ContextMenu"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
<method name="SecondaryActivate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
</interface></node>"#;

fn handle_menu(
    conn: &mut Connection,
    item: &Item,
    m: &Message,
) -> Result<(), super::dbus::DbusError> {
    let Some(root) = &item.menu else {
        return conn.reply_error(m, "org.freedesktop.DBus.Error.UnknownObject", MENU_PATH);
    };
    let member = m.member.as_deref().unwrap_or("");
    match (m.interface.as_deref().unwrap_or(""), member) {
        (MENU_IFACE, "GetLayout") => {
            let mut r = Reader::new(&m.body);
            let parent = r.i32()?;
            let depth = r.i32()?;
            let Some(node) = find(root, parent) else {
                return conn.reply_error(
                    m,
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "no such menu id",
                );
            };
            let mut w = Writer::default();
            w.u32(1); // revision
            write_node(&mut w, node, depth, parent == 0);
            conn.reply(m, "u(ia{sv}av)", &w.buf)
        }
        (MENU_IFACE, "Event") => {
            let mut r = Reader::new(&m.body);
            let id = r.i32()?;
            let kind = r.string()?;
            if kind == "clicked" {
                if let Some((_, a)) = item.actions.iter().find(|(i, _)| *i == id) {
                    super::push_pending(PlatformEvent::MenuAction(a.clone()));
                }
            }
            conn.reply(m, "", &[])
        }
        (MENU_IFACE, "AboutToShow") => {
            let mut w = Writer::default();
            w.bool(false);
            conn.reply(m, "b", &w.buf)
        }
        (PROPS, "Get") => {
            let mut r = Reader::new(&m.body);
            let _ = r.string()?;
            let name = r.string()?;
            let mut w = Writer::default();
            match name.as_str() {
                "Version" => w.variant("u", |w| w.u32(3)),
                "TextDirection" => w.variant("s", |w| w.string("ltr")),
                "Status" => w.variant("s", |w| w.string("normal")),
                _ => {
                    return conn.reply_error(m, "org.freedesktop.DBus.Error.UnknownProperty", &name)
                }
            }
            conn.reply(m, "v", &w.buf)
        }
        _ => conn.reply_error(m, "org.freedesktop.DBus.Error.UnknownMethod", member),
    }
}

fn find(n: &Node, id: i32) -> Option<&Node> {
    if n.id == id {
        return Some(n);
    }
    n.children.iter().find_map(|c| find(c, id))
}

fn handle(conn: &mut Connection, item: &Item, m: &Message) -> Result<(), super::dbus::DbusError> {
    if m.kind != Kind::Call {
        return Ok(());
    }
    if m.path.as_deref() == Some(MENU_PATH) {
        return handle_menu(conn, item, m);
    }
    if m.path.as_deref() != Some(ITEM_PATH) {
        return Ok(());
    }
    let (iface, member) = (
        m.interface.as_deref().unwrap_or(""),
        m.member.as_deref().unwrap_or(""),
    );
    match (iface, member) {
        (ITEM_IFACE, "Activate" | "ContextMenu" | "SecondaryActivate") => {
            super::push_pending(PlatformEvent::TrayClicked);
            conn.reply(m, "", &[])
        }
        (PROPS, "Get") => {
            let mut r = Reader::new(&m.body);
            let _iface = r.string()?;
            let name = r.string()?;
            let mut w = Writer::default();
            if item.property(&name, &mut w) {
                conn.reply(m, "v", &w.buf)
            } else {
                conn.reply_error(m, "org.freedesktop.DBus.Error.UnknownProperty", &name)
            }
        }
        (PROPS, "GetAll") => {
            let mut w = Writer::default();
            w.array(8, |w| {
                for n in NAMES {
                    w.align(8);
                    w.string(n);
                    item.property(n, w);
                }
            });
            conn.reply(m, "a{sv}", &w.buf)
        }
        ("org.freedesktop.DBus.Introspectable", "Introspect") => {
            let mut w = Writer::default();
            w.string(INTROSPECT);
            conn.reply(m, "s", &w.buf)
        }
        _ => conn.reply_error(m, "org.freedesktop.DBus.Error.UnknownMethod", member),
    }
}

/// Start serving the tray item. `register` announces it to the
/// StatusNotifierWatcher (disabled only by tests with no watcher).
pub(crate) fn start(t: &TraySettings, register: bool) -> Result<(), PlatformError> {
    if !t.icon_valid() || t.icon_width == 0 {
        return Err(PlatformError(
            "tray icon buffer does not match its size".into(),
        ));
    }
    stop();
    let fail = |e: super::dbus::DbusError| PlatformError(format!("tray: {}", e.0));
    let mut conn = Connection::session().map_err(fail)?;
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let mut w = Writer::default();
    w.string(&name);
    w.u32(4); // DBUS_NAME_FLAG_DO_NOT_QUEUE
    conn.call(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "RequestName",
        "su",
        &w.buf,
    )
    .map_err(fail)?;
    if register {
        let mut w = Writer::default();
        w.string(&name);
        conn.call(
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
            "RegisterStatusNotifierItem",
            "s",
            &w.buf,
        )
        .map_err(|_| {
            PlatformError(
                "tray: no StatusNotifierWatcher on the session bus (desktop has no tray)".into(),
            )
        })?;
    }
    let mut actions = Vec::new();
    let menu = t.menu.as_ref().map(|m: &Menu| Node {
        id: 0,
        label: m.title.clone(),
        enabled: true,
        separator: false,
        children: build_tree(&m.items, &mut 0, &mut actions),
    });
    let item = Item {
        menu,
        actions,
        tooltip: t.tooltip.clone(),
        argb: rgba_to_argb(&t.icon_rgba),
        w: t.icon_width,
        h: t.icon_height,
        id: format!("alloy-{}", std::process::id()),
    };
    let stream = conn.try_clone_stream().map_err(fail)?;
    *RUNNING
        .lock()
        .map_err(|_| PlatformError("state poisoned".into()))? = Some(stream);
    std::thread::Builder::new()
        .name("alloy-tray".into())
        .spawn(move || {
            // Ends when `stop` shuts the socket down or the bus goes away.
            while let Ok(m) = conn.read() {
                if handle(&mut conn, &item, &m).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| PlatformError(format!("tray thread: {e}")))?;
    Ok(())
}

/// Remove the tray item (the bus drops it when our connection closes).
pub(crate) fn stop() {
    if let Ok(mut g) = RUNNING.lock() {
        if let Some(s) = g.take() {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_order() {
        assert_eq!(rgba_to_argb(&[1, 2, 3, 4]), vec![4, 1, 2, 3]);
    }

    /// Needs a session bus: run under `dbus-run-session`. Skipped otherwise.
    #[test]
    fn serves_properties_on_real_bus() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            eprintln!("skipped: no session bus");
            return;
        }
        let t = TraySettings {
            tooltip: "Alloy test".into(),
            icon_rgba: vec![255; 4 * 4 * 4],
            icon_width: 4,
            icon_height: 4,
            menu: Some(
                Menu::new("t")
                    .action("open", "Open_me", None)
                    .separator()
                    .submenu(Menu::new("More").action("deep", "Deep", None)),
            ),
        };
        start(&t, false).expect("start");
        let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
        let get = |prop: &str| {
            let o = std::process::Command::new("gdbus")
                .args([
                    "call",
                    "--session",
                    "--dest",
                    &name,
                    "--object-path",
                    ITEM_PATH,
                ])
                .args([
                    "--method",
                    "org.freedesktop.DBus.Properties.Get",
                    ITEM_IFACE,
                    prop,
                ])
                .output()
                .expect("gdbus");
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        assert!(get("Status").contains("Active"), "{}", get("Status"));
        assert!(get("Title").contains("Alloy test"));
        assert!(get("IconPixmap").contains("4, 4"), "{}", get("IconPixmap"));
        let o = std::process::Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                &name,
                "--object-path",
                ITEM_PATH,
            ])
            .args(["--method", "org.kde.StatusNotifierItem.Activate", "0", "0"])
            .output()
            .unwrap();
        assert!(o.status.success());
        assert!(super::super::take_pending().contains(&PlatformEvent::TrayClicked));
        // Context menu over dbusmenu.
        let call = |args: &[&str]| {
            let o = std::process::Command::new("busctl")
                .args([
                    "--user",
                    "call",
                    "--",
                    &name,
                    MENU_PATH,
                    "com.canonical.dbusmenu",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        let layout = call(&["GetLayout", "iias", "0", "-1", "0"]);
        assert!(layout.contains("Open__me"), "{layout}");
        assert!(layout.contains("separator"), "{layout}");
        assert!(layout.contains("Deep"), "{layout}");
        call(&["Event", "isvu", "4", "clicked", "i", "0", "0"]);
        assert!(super::super::take_pending().contains(&PlatformEvent::MenuAction("deep".into())));
        stop();
    }
}
