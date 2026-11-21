//! Single-instance apps: a second launch hands its arguments (a deep link, a
//! file to open) to the running instance and exits.
//!
//! Mechanism, the same on every OS and in plain `std`: the first instance
//! listens on a loopback TCP port and records `port` and a random `token` in a
//! lock file inside the user's private runtime directory (mode 0600 on Unix).
//! A later launch reads the file, connects, proves it can read the file by
//! sending the token, forwards its arguments, and exits. A lock file whose
//! port no longer answers is stale (the app crashed) and is replaced. The
//! token keeps other local users and web pages from injecting arguments.

use crate::clipboard::{percent_decode, percent_encode};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

/// The result of [`single_instance`].
#[derive(Debug)]
pub enum Single {
    /// This process is the (only) instance. Keep the guard alive for as long
    /// as the app runs; dropping it releases the lock.
    Primary(Guard),
    /// Another instance is running and now has this launch's arguments.
    /// Exit without opening a window.
    Forwarded,
}

/// Holds the lock file; removes it when dropped.
#[derive(Debug)]
pub struct Guard {
    path: PathBuf,
    token: String,
}

impl Drop for Guard {
    fn drop(&mut self) {
        // Only remove our own lock, never a successor's.
        if std::fs::read_to_string(&self.path).is_ok_and(|s| s.lines().nth(1) == Some(&self.token))
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn lock_dir() -> PathBuf {
    #[cfg(unix)]
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(not(unix))]
    let base = std::env::temp_dir();
    base.join("alloy-ui-instances")
}

/// A file name for `app_id` that is the same on every OS.
fn lock_path(app_id: &str) -> PathBuf {
    let safe: String = app_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "-._".contains(c) { c } else { '_' })
        .collect();
    lock_dir().join(format!("{safe}.lock"))
}

fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    // `RandomState` is seeded from the OS's random source.
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Send `args` to the instance described by `lock`. True once it acknowledged.
fn forward(lock: &str, args: &[String]) -> bool {
    let mut lines = lock.lines();
    let (Some(port), Some(token)) = (
        lines.next().and_then(|p| p.parse::<u16>().ok()),
        lines.next(),
    ) else {
        return false;
    };
    let Ok(mut s) = TcpStream::connect_timeout(
        &(Ipv4Addr::LOCALHOST, port).into(),
        Duration::from_millis(500),
    ) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
    let mut msg = format!("{token} {}\n", args.len());
    for a in args {
        msg.push_str(&percent_encode(a));
        msg.push('\n');
    }
    if s.write_all(msg.as_bytes()).is_err() {
        return false;
    }
    let mut ack = String::new();
    BufReader::new(s).read_line(&mut ack).is_ok() && ack.trim() == "OK"
}

/// Read one forwarded launch from `s`: the arguments if the token matches.
fn receive(s: TcpStream, token: &str) -> Option<Vec<String>> {
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let mut r = BufReader::new(s.try_clone().ok()?);
    let mut head = String::new();
    r.read_line(&mut head).ok()?;
    let (got, count) = head.trim_end().split_once(' ')?;
    // Constant-time-ish compare is unnecessary on loopback, but cheap.
    if got.len() != token.len() || !got.bytes().zip(token.bytes()).fold(true, |a, (x, y)| a & (x == y)) {
        return None;
    }
    let n: usize = count.parse().ok().filter(|&n| n <= 256)?;
    let mut args = Vec::with_capacity(n);
    for _ in 0..n {
        let mut l = String::new();
        r.read_line(&mut l).ok()?;
        args.push(percent_decode(l.trim_end_matches(['\r', '\n'])));
    }
    let _ = (&s).write_all(b"OK\n");
    Some(args)
}

/// Claim the single instance of `app_id` (a reverse-DNS name is customary), or
/// forward this launch's arguments to the one already running.
///
/// Call it first thing in `main`, before opening a window. As the primary
/// instance, arguments forwarded by later launches arrive as
/// [`crate::PlatformEvent::Open`]. If anything about the lock mechanism fails
/// (read-only runtime dir, no loopback), the app simply runs as an ordinary
/// instance: that is the safe direction to fail in.
pub fn single_instance(app_id: &str) -> Single {
    let path = lock_path(app_id);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Some(dir) = path.parent() {
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
    }
    for _ in 0..6 {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) else {
                    let _ = std::fs::remove_file(&path);
                    return Single::Primary(Guard { path, token: String::new() });
                };
                let port = listener.local_addr().map_or(0, |a| a.port());
                let token = random_token();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
                }
                let _ = write!(f, "{port}\n{token}\n");
                drop(f);
                let t = token.clone();
                let _ = std::thread::Builder::new()
                    .name("alloy-instance".into())
                    .spawn(move || {
                        for s in listener.incoming().flatten() {
                            if let Some(args) = receive(s, &t) {
                                crate::os::push_pending(crate::PlatformEvent::Open(args));
                            }
                        }
                    });
                return Single::Primary(Guard { path, token });
            }
            Err(_) => {
                // A lock exists. Another launch may still be writing it, so retry briefly
                // before judging it stale.
                let lock = std::fs::read_to_string(&path).unwrap_or_default();
                if lock.lines().count() >= 2 {
                    if forward(&lock, &args) {
                        return Single::Forwarded;
                    }
                    let _ = std::fs::remove_file(&path); // stale: nothing answers
                } else {
                    std::thread::sleep(Duration::from_millis(40));
                }
            }
        }
    }
    // Could not settle it: run normally rather than refuse to start.
    Single::Primary(Guard { path, token: String::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique(name: &str) -> String {
        format!("test.{name}.{}", std::process::id())
    }

    #[test]
    fn second_launch_is_forwarded() {
        let id = unique("fwd");
        let Single::Primary(guard) = single_instance(&id) else {
            panic!("first launch must be primary");
        };
        // The second "launch" runs in this process; its args come from the
        // process (empty under the test harness besides flags), so use the
        // lower-level pieces for specific content.
        let lock = std::fs::read_to_string(&guard.path).unwrap();
        assert!(forward(&lock, &["myapp://x?a=1&b=ü".into(), "two words".into()]));
        let Single::Forwarded = single_instance(&id) else {
            panic!("second launch must be forwarded");
        };
        drop(guard);
        assert!(!lock_path(&id).exists(), "lock removed on drop");
    }

    #[test]
    fn wrong_token_is_refused() {
        let id = unique("tok");
        let Single::Primary(guard) = single_instance(&id) else {
            panic!()
        };
        let lock = std::fs::read_to_string(&guard.path).unwrap();
        let port = lock.lines().next().unwrap();
        let forged = format!("{port}\n{}\n", "0".repeat(32));
        assert!(!forward(&forged, &["x".into()]));
        assert!(forward(&lock, &["x".into()]));
    }

    #[test]
    fn stale_lock_is_replaced() {
        let id = unique("stale");
        let path = lock_path(&id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A lock whose port nothing listens on.
        let dead = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = dead.local_addr().unwrap().port();
        drop(dead);
        std::fs::write(&path, format!("{port}\nabc\n")).unwrap();
        match single_instance(&id) {
            Single::Primary(_) => {}
            Single::Forwarded => panic!("a dead lock must not swallow the launch"),
        }
    }
}
