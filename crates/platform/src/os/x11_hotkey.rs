//! Global hotkeys on X11 with `XGrabKey` over `x11rb` (no `unsafe`). Each hotkey
//! gets its own connection and thread; `release_all` wakes the threads so they
//! drop their connections, which releases the grabs.

use super::push_pending;
use crate::{hotkey::x11_keysym, shortcut::Shortcut, PlatformError, PlatformEvent};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{
            AtomEnum, ClientMessageEvent, ConnectionExt, CreateWindowAux, EventMask, GrabMode,
            ModMask, WindowClass,
        },
        Event,
    },
    rust_connection::RustConnection,
    COPY_DEPTH_FROM_PARENT,
};

const SHIFT: u16 = 1;
const LOCK: u16 = 2;
const CONTROL: u16 = 4;
const MOD1: u16 = 8;
const MOD2: u16 = 16;
const MOD4: u16 = 64;

/// Caps Lock and Num Lock must not stop a hotkey from matching.
const IGNORED: [u16; 4] = [0, LOCK, MOD2, LOCK | MOD2];

struct Grab {
    conn: Arc<RustConnection>,
    /// Window the thread owns; an empty `SendEvent` to it wakes the thread.
    wake: u32,
    stop: Arc<AtomicBool>,
}

static OPEN: Mutex<Vec<Grab>> = Mutex::new(Vec::new());

fn mask(s: &Shortcut) -> u16 {
    let mut m = 0;
    if s.mods.shift {
        m |= SHIFT;
    }
    if s.mods.ctrl {
        m |= CONTROL;
    }
    if s.mods.alt {
        m |= MOD1;
    }
    if s.mods.logo {
        m |= MOD4;
    }
    m
}

pub(crate) fn register(keys: &str, sc: &Shortcut) -> Result<(), PlatformError> {
    let fail = |m: String| PlatformError(format!("global hotkey {keys}: {m}"));
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Err(fail(
            "Wayland has no way to grab keys for every window; use an in-window shortcut".into(),
        ));
    }
    let keysym = x11_keysym(&sc.key).ok_or_else(|| fail(format!("unknown key {:?}", sc.key)))?;
    let (c, screen) = RustConnection::connect(None).map_err(|e| fail(e.to_string()))?;
    let root = c.setup().roots[screen].root;
    let (min, max) = (c.setup().min_keycode, c.setup().max_keycode);
    let map = c
        .get_keyboard_mapping(min, max - min + 1)
        .map_err(|e| fail(e.to_string()))?
        .reply()
        .map_err(|e| fail(e.to_string()))?;
    let per = (map.keysyms_per_keycode as usize).max(1);
    let code = map
        .keysyms
        .chunks(per)
        .position(|syms| syms.contains(&keysym))
        .map(|i| min + i as u8)
        .ok_or_else(|| fail("the keyboard layout has no such key".into()))?;
    let base = mask(sc);
    for extra in IGNORED {
        c.grab_key(
            false,
            root,
            ModMask::from(base | extra),
            code,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )
        .map_err(|e| fail(e.to_string()))?
        .check()
        .map_err(|_| fail("already taken by another program".into()))?;
    }
    let wake = c.generate_id().map_err(|e| fail(e.to_string()))?;
    c.create_window(
        COPY_DEPTH_FROM_PARENT,
        wake,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new(),
    )
    .map_err(|e| fail(e.to_string()))?;
    c.flush().map_err(|e| fail(e.to_string()))?;
    let conn = Arc::new(c);
    let stop = Arc::new(AtomicBool::new(false));
    let (thread_conn, thread_stop, name) = (conn.clone(), stop.clone(), keys.to_string());
    std::thread::Builder::new()
        .name("alloy-hotkey".into())
        .spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match thread_conn.wait_for_event() {
                    Ok(Event::KeyPress(k)) => {
                        let held = u16::from(k.state) & !(LOCK | MOD2) & 0xff;
                        if k.detail == code && held == base {
                            push_pending(PlatformEvent::GlobalHotkey(name.clone()));
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| fail(e.to_string()))?;
    if let Ok(mut open) = OPEN.lock() {
        open.push(Grab { conn, wake, stop });
    }
    Ok(())
}

pub(crate) fn release_all() {
    let grabs = OPEN
        .lock()
        .map(|mut o| std::mem::take(&mut *o))
        .unwrap_or_default();
    for g in grabs {
        g.stop.store(true, Ordering::Release);
        // Zero event mask: delivered to whoever created `wake`, i.e. the thread's own client.
        let ev = ClientMessageEvent::new(32, g.wake, AtomEnum::STRING, [0u32; 5]);
        let _ = g.conn.send_event(false, g.wake, EventMask::NO_EVENT, ev);
        let _ = g.conn.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs an X server (`DISPLAY`) and `xdotool` to type the key. Skipped otherwise.
    #[test]
    fn grabbed_key_reports_even_without_a_window() {
        let have_xdotool = std::process::Command::new("xdotool")
            .arg("version")
            .output()
            .is_ok();
        if std::env::var_os("DISPLAY").is_none()
            || std::env::var_os("WAYLAND_DISPLAY").is_some()
            || !have_xdotool
        {
            eprintln!("skipped: needs X11 and xdotool");
            return;
        }
        let _guard = super::super::BUS_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let sc = Shortcut::parse("ctrl+alt+h").unwrap();
        register("ctrl+alt+h", &sc).expect("grab");
        // A second grab of the same key from another connection must be refused.
        assert!(register("ctrl+alt+h", &sc).is_err());
        let typed = std::process::Command::new("xdotool")
            .args(["key", "ctrl+alt+h"])
            .status()
            .unwrap();
        assert!(typed.success());
        let mut seen = false;
        for _ in 0..50 {
            if super::super::take_pending()
                .contains(&PlatformEvent::GlobalHotkey("ctrl+alt+h".into()))
            {
                seen = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        release_all();
        assert!(seen, "hotkey press never arrived");
        // After release the key is free again.
        std::thread::sleep(std::time::Duration::from_millis(100));
        register("ctrl+alt+h", &sc).expect("grab again after release");
        release_all();
    }
}
