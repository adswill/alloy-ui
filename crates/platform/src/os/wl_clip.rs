//! Native Wayland clipboard over `wayland-client` (no `wl-copy`/`wl-paste`).
//!
//! Uses the *data-control* protocols (`ext_data_control_v1`, falling back to
//! `zwlr_data_control_v1`). Unlike core `wl_data_device`, they let a client read
//! and own the selection without keyboard focus or an input serial, which is
//! what a separate connection from the windowing one has. They are offered by
//! wlroots compositors (sway, Hyprland, river), KDE Plasma, COSMIC and niri.
//! GNOME's Mutter offers neither; there the XWayland path in `x11_clip` or
//! the in-process buffer is used (see `linux.rs`).
//!
//! `read` opens a short-lived connection, finds the current selection offer and
//! pipes it. `write` parks a thread that owns the selection until another
//! client replaces it (`cancelled`), serving `send` requests like `x11_clip`.
//! Both work on any MIME type, so HTML, PNG images and file lists use the same
//! path as text (`read_mime`, `write_formats`, `mimes`).

use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::sync::mpsc;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_millis(700);
/// Preference order of the text mime types we accept when reading.
const TEXT_MIMES: [&str; 4] = [
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "TEXT",
];

fn pick_mime(offered: &[String]) -> Option<&'static str> {
    TEXT_MIMES
        .iter()
        .copied()
        .find(|want| offered.iter().any(|m| m == want))
}

/// Pipe `fd` to `text` on a thread, so a slow reader cannot stall the owner loop.
fn serve(fd: OwnedFd, data: std::sync::Arc<Vec<u8>>) {
    let _ = std::thread::Builder::new()
        .name("alloy-wl-send".into())
        .spawn(move || {
            let mut f = std::fs::File::from(fd);
            let _ = f.write_all(&data);
        });
}

/// Read everything from `fd`, giving up after [`TIMEOUT`].
fn drain(fd: OwnedFd) -> Option<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("alloy-wl-recv".into())
        .spawn(move || {
            let mut buf = Vec::new();
            let _ = std::fs::File::from(fd).read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
    // Images are large; allow far longer than for text before giving up.
    rx.recv_timeout(TIMEOUT * 10).ok()
}

/// Read the clipboard text. `None` when no data-control protocol is offered,
/// the selection is empty, or it has no text flavour.
pub fn read() -> Option<String> {
    let mimes = mimes()?;
    let m = pick_mime(&mimes)?;
    Some(String::from_utf8_lossy(&read_mime(m)?).into_owned())
}

/// MIME types the current selection offers.
pub fn mimes() -> Option<Vec<String>> {
    ext::mimes().or_else(wlr::mimes)
}

/// The selection's bytes in one MIME type.
pub fn read_mime(mime: &str) -> Option<Vec<u8>> {
    ext::read_mime(mime).or_else(|| wlr::read_mime(mime))
}

/// Take ownership of the clipboard with `text`. Returns false when no
/// data-control protocol is offered (the caller then tries another path).
/// The previous owner thread of ours, if any, is told `cancelled` by the
/// compositor when we replace the selection, and exits by itself.
pub fn write(text: &str) -> bool {
    write_formats(crate::clipboard::to_formats(&crate::clipboard::ClipContent::text(
        text,
    )))
}

/// Take ownership offering every `(mime, bytes)` pair.
pub fn write_formats(formats: Vec<(String, Vec<u8>)>) -> bool {
    let data: std::collections::HashMap<String, std::sync::Arc<Vec<u8>>> = formats
        .into_iter()
        .map(|(m, d)| (m, std::sync::Arc::new(d)))
        .collect();
    ext::write(&data) || wlr::write(&data)
}

/// Expands to one protocol flavour. The two protocols are identical in shape
/// but generate distinct Rust types, hence the macro.
macro_rules! data_control {
    ($name:ident, $manager:path, $device:path, $source:path, $offer:path,
     $dev_mod:path, $src_mod:path, $off_mod:path) => {
        mod $name {
            use super::*;
            use std::collections::HashMap;
            use std::sync::Arc;
            use wayland_client::{
                event_created_child,
                globals::{registry_queue_init, GlobalListContents},
                protocol::{wl_registry, wl_seat::WlSeat},
                Connection, Dispatch, EventQueue, Proxy, QueueHandle,
            };
            type Manager = $manager;
            type Device = $device;
            type Source = $source;
            type Offer = $offer;
            use $dev_mod as dev_mod;
            use $off_mod as off_mod;
            use $src_mod as src_mod;

            #[derive(Default)]
            struct St {
                mimes: HashMap<wayland_client::backend::ObjectId, Vec<String>>,
                selection: Option<Option<Offer>>,
                data: HashMap<String, Arc<Vec<u8>>>,
                cancelled: bool,
            }

            struct Session {
                queue: EventQueue<St>,
                st: St,
                manager: Manager,
                device: Device,
            }

            fn open() -> Option<Session> {
                let conn = Connection::connect_to_env().ok()?;
                let (globals, mut queue) = registry_queue_init::<St>(&conn).ok()?;
                let qh = queue.handle();
                let manager: Manager = globals.bind(&qh, 1..=1, ()).ok()?;
                let seat: WlSeat = globals.bind(&qh, 1..=1, ()).ok()?;
                let device = manager.get_data_device(&seat, &qh, ());
                let mut st = St::default();
                // First roundtrip delivers `data_offer`s, the second the `selection`.
                queue.roundtrip(&mut st).ok()?;
                queue.roundtrip(&mut st).ok()?;
                Some(Session {
                    queue,
                    st,
                    manager,
                    device,
                })
            }

            pub fn mimes() -> Option<Vec<String>> {
                let mut s = open()?;
                let offer = s.st.selection.take()??;
                s.st.mimes.get(&offer.id()).cloned()
            }

            pub fn read_mime(mime: &str) -> Option<Vec<u8>> {
                let mut s = open()?;
                let offer = s.st.selection.take()??;
                if !s.st.mimes.get(&offer.id())?.iter().any(|m| m == mime) {
                    return None;
                }
                // A socketpair stands in for a pipe (`io::pipe` is newer than our MSRV).
                let (rd, wr) = std::os::unix::net::UnixStream::pair().ok()?;
                offer.receive(mime.to_owned(), wr.as_fd());
                drop(wr); // EOF once the owner closes its end
                s.queue.flush().ok()?;
                drain(OwnedFd::from(rd))
            }

            pub fn write(data: &HashMap<String, Arc<Vec<u8>>>) -> bool {
                let Some(mut s) = open() else {
                    return false;
                };
                let qh = s.queue.handle();
                let source = s.manager.create_data_source(&qh, ());
                for m in data.keys() {
                    source.offer(m.clone());
                }
                s.device.set_selection(Some(&source));
                s.st.data = data.clone();
                if s.queue.roundtrip(&mut s.st).is_err() || s.st.cancelled {
                    return false;
                }
                // Park the owner thread until the compositor says `cancelled`.
                spawn_owner(s, source);
                true
            }

            fn spawn_owner(mut s: Session, source: Source) {
                let _ = std::thread::Builder::new()
                    .name("alloy-wl-clip".into())
                    .spawn(move || {
                        let _keep = &source;
                        while !s.st.cancelled {
                            if s.queue.blocking_dispatch(&mut s.st).is_err() {
                                break;
                            }
                        }
                    });
            }

            impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for St {
                fn event(
                    _: &mut Self,
                    _: &wl_registry::WlRegistry,
                    _: wl_registry::Event,
                    _: &GlobalListContents,
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }
            impl Dispatch<WlSeat, ()> for St {
                fn event(
                    _: &mut Self,
                    _: &WlSeat,
                    _: <WlSeat as Proxy>::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }
            impl Dispatch<Manager, ()> for St {
                fn event(
                    _: &mut Self,
                    _: &Manager,
                    _: <Manager as Proxy>::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                }
            }
            impl Dispatch<Device, ()> for St {
                fn event(
                    st: &mut Self,
                    _: &Device,
                    ev: dev_mod::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                    match ev {
                        dev_mod::Event::DataOffer { id } => {
                            st.mimes.insert(id.id(), Vec::new());
                        }
                        dev_mod::Event::Selection { id } => st.selection = Some(id),
                        _ => {}
                    }
                }
                event_created_child!(St, Device, [
                    0 => (Offer, ())
                ]);
            }
            impl Dispatch<Offer, ()> for St {
                fn event(
                    st: &mut Self,
                    offer: &Offer,
                    ev: off_mod::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                    if let off_mod::Event::Offer { mime_type } = ev {
                        st.mimes.entry(offer.id()).or_default().push(mime_type);
                    }
                }
            }
            impl Dispatch<Source, ()> for St {
                fn event(
                    st: &mut Self,
                    _: &Source,
                    ev: src_mod::Event,
                    _: &(),
                    _: &Connection,
                    _: &QueueHandle<Self>,
                ) {
                    match ev {
                        src_mod::Event::Send { fd, mime_type } => match st.data.get(&mime_type) {
                            Some(d) => serve(fd, d.clone()),
                            None => drop(fd),
                        },
                        src_mod::Event::Cancelled => st.cancelled = true,
                        _ => {}
                    }
                }
            }
        }
    };
}

data_control!(
    ext,
    wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::ExtDataControlManagerV1,
    wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::ExtDataControlDeviceV1,
    wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::ExtDataControlSourceV1,
    wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::ExtDataControlOfferV1,
    wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1,
    wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1,
    wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1
);

data_control!(
    wlr,
    wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
    wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
    wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
    wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_device_v1,
    wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_source_v1,
    wayland_protocols_wlr::data_control::v1::client::zwlr_data_control_offer_v1
);

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a Wayland compositor with data-control (sway, Hyprland, KDE...).
    /// Skips silently otherwise. `read` opens its own connection, so this
    /// goes through the compositor between two clients.
    #[test]
    fn roundtrip_through_compositor() {
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            eprintln!("skipped: no WAYLAND_DISPLAY");
            return;
        }
        if !write("alloy wayland ✓ clipboard") {
            eprintln!("skipped: compositor has no data-control protocol");
            return;
        }
        assert_eq!(read().as_deref(), Some("alloy wayland ✓ clipboard"));
        // A second write replaces the first (the old owner thread is cancelled).
        assert!(write("second"));
        assert_eq!(read().as_deref(), Some("second"));
        // Large values go through the pipe in pieces.
        let big = "x".repeat(300_000);
        assert!(write(&big));
        assert_eq!(read().map(|s| s.len()), Some(big.len()));
        // Several formats at once, binary included.
        let png = vec![0x89u8, b'P', b'N', b'G', 0, 1, 2, 255];
        assert!(write_formats(vec![
            ("image/png".into(), png.clone()),
            ("text/html".into(), b"<b>x</b>".to_vec()),
        ]));
        let offered = mimes().unwrap();
        assert!(offered.iter().any(|m| m == "image/png"));
        assert_eq!(read_mime("image/png"), Some(png));
        assert_eq!(read_mime("text/html"), Some(b"<b>x</b>".to_vec()));
        assert_eq!(read_mime("text/uri-list"), None);
    }
}
