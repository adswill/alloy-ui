//! Linux notifications over `org.freedesktop.Notifications`, which GNOME, KDE,
//! XFCE, Cinnamon and most other desktops serve. Uses our own D-Bus client, with
//! `notify-send` as the fallback when the bus is not reachable.

use super::dbus::{Connection, Writer};
use crate::{notify::Notification, PlatformError};

const DEST: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

fn app_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "alloy".into())
}

/// Post `n`; the service answers with the notification's id.
pub(crate) fn send(n: &Notification) -> Result<(), PlatformError> {
    let via_bus = || -> Result<(), super::dbus::DbusError> {
        let mut conn = Connection::session()?;
        let mut w = Writer::default();
        w.string(&app_name());
        w.u32(0); // replaces_id: always a new notification
        w.string(""); // app_icon: the desktop picks one from the .desktop entry
        w.string(&n.title);
        w.string(&n.body);
        w.array(4, |_| {}); // actions
        w.array(8, |_| {}); // hints
        w.i32(n.timeout_ms.map_or(-1, |t| t.min(i32::MAX as u32) as i32));
        conn.call(DEST, PATH, DEST, "Notify", "susssasa{sv}i", &w.buf)?;
        Ok(())
    };
    match via_bus() {
        Ok(()) => Ok(()),
        Err(e) => {
            let mut args = vec!["--app-name".to_string(), app_name()];
            if let Some(t) = n.timeout_ms {
                args.push(format!("--expire-time={t}"));
            }
            args.extend(["--".to_string(), n.title.clone(), n.body.clone()]);
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            super::capture("notify-send", &refs)
                .map(|_| ())
                .ok_or_else(|| PlatformError(format!("notification: {} (and no notify-send)", e.0)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::dbus::{Kind, Reader};

    /// Needs a session bus: run under `dbus-run-session`. Skipped otherwise.
    #[test]
    fn notify_reaches_a_notification_service() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            eprintln!("skipped: no session bus");
            return;
        }
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
            .expect("own name");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(m) = server.read() {
                if m.kind == Kind::Call && m.member.as_deref() == Some("Notify") {
                    let mut r = Reader::new(&m.body);
                    let app = r.string().unwrap();
                    let _replaces = r.u32().unwrap();
                    let _icon = r.string().unwrap();
                    let title = r.string().unwrap();
                    let body = r.string().unwrap();
                    let mut w = Writer::default();
                    w.u32(7);
                    server.reply(&m, "u", &w.buf).unwrap();
                    tx.send((app, title, body)).unwrap();
                    break;
                }
            }
        });
        send(&Notification {
            title: "Build finished".into(),
            body: "0 errors".into(),
            timeout_ms: Some(3000),
        })
        .expect("send");
        let (app, title, body) = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("service saw the call");
        assert!(!app.is_empty());
        assert_eq!(
            (title.as_str(), body.as_str()),
            ("Build finished", "0 errors")
        );
    }
}
