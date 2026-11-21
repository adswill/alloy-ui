//! Linux menu bar through the `com.canonical.AppMenu.Registrar` protocol: the
//! menu is exported over dbusmenu (the same code as the tray menu) and the
//! desktop's global-menu panel shows it. KDE Plasma (with the Global Menu
//! widget), Unity and MATE do; GNOME and most bare window managers have no menu
//! bar to put it in, and the call then reports that honestly.

use super::dbus::{Connection, Kind, Writer};
use super::linux_tray::{build_tree, handle_menu, Node, MENU_PATH};
use crate::{
    menu::{Menu, MenuItem},
    PlatformError,
};
use std::{net::Shutdown, os::unix::net::UnixStream, sync::Mutex};

static RUNNING: Mutex<Option<UnixStream>> = Mutex::new(None);

/// Top-level menus as dbusmenu nodes under the invisible root (id 0).
pub(crate) fn tree(menus: &[Menu], actions: &mut Vec<(i32, String)>) -> Node {
    let items: Vec<MenuItem> = menus.iter().cloned().map(MenuItem::Submenu).collect();
    Node {
        id: 0,
        label: String::new(),
        enabled: true,
        separator: false,
        children: build_tree(&items, &mut 0, actions),
    }
}

/// X11 window id for the registrar, which keys menus by it.
pub(crate) fn x11_window_id(w: &winit::window::Window) -> Option<u32> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match w.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(h) => u32::try_from(h.window).ok(),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}

/// Export `menus` and register them for `window_id`. Replaces any earlier menu bar.
pub(crate) fn start(window_id: u32, menus: &[Menu], register: bool) -> Result<(), PlatformError> {
    stop();
    let fail = |e: super::dbus::DbusError| PlatformError(format!("menu bar: {}", e.0));
    let mut conn = Connection::session().map_err(fail)?;
    let name = format!("org.alloyui.MenuBar{}", std::process::id());
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
        w.u32(window_id);
        w.string(MENU_PATH);
        conn.call(
            "com.canonical.AppMenu.Registrar",
            "/com/canonical/AppMenu/Registrar",
            "com.canonical.AppMenu.Registrar",
            "RegisterWindow",
            "uo",
            &w.buf,
        )
        .map_err(|_| {
            PlatformError(
                "menu bar: this desktop has no global menu (shortcuts and the command palette still work)"
                    .into(),
            )
        })?;
    }
    let mut actions = Vec::new();
    let root = tree(menus, &mut actions);
    let stream = conn.try_clone_stream().map_err(fail)?;
    *RUNNING
        .lock()
        .map_err(|_| PlatformError("state poisoned".into()))? = Some(stream);
    std::thread::Builder::new()
        .name("alloy-menubar".into())
        .spawn(move || {
            // Ends when `stop` shuts the socket down or the bus goes away.
            while let Ok(m) = conn.read() {
                if m.kind == Kind::Call
                    && m.path.as_deref() == Some(MENU_PATH)
                    && handle_menu(&mut conn, Some(&root), &actions, &m).is_err()
                {
                    break;
                }
            }
        })
        .map_err(|e| PlatformError(format!("menu bar thread: {e}")))?;
    Ok(())
}

/// Drop the exported menu (the bus forgets it when the connection closes).
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
    use crate::PlatformEvent;

    /// Needs a session bus: run under `dbus-run-session`. Skipped otherwise.
    #[test]
    fn exports_menu_bar_over_dbusmenu() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            eprintln!("skipped: no session bus");
            return;
        }
        let menus = [
            Menu::new("File").action("mod+o", "Open", Some("mod+o")),
            Menu::new("Edit").action("cmd:Undo", "Undo", None),
        ];
        let _guard = super::super::BUS_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        start(0x42, &menus, false).expect("start");
        let name = format!("org.alloyui.MenuBar{}", std::process::id());
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
        assert!(
            layout.contains("File") && layout.contains("Edit"),
            "{layout}"
        );
        assert!(
            layout.contains("Open") && layout.contains("Undo"),
            "{layout}"
        );
        // Ids are numbered depth first: File=1, Open=2, Edit=3, Undo=4.
        call(&["Event", "isvu", "4", "clicked", "i", "0", "0"]);
        assert!(
            super::super::take_pending().contains(&PlatformEvent::MenuAction("cmd:Undo".into()))
        );
        stop();
    }
}
