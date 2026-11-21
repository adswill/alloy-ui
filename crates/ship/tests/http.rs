//! End to end over a real socket: a throwaway HTTP server serves a signed
//! release, the updater checks, downloads, verifies and replaces a file.

use alloy_ui_ship::manifest::{Artifact, Manifest};
use alloy_ui_ship::update::{Applied, Config, Updater};
use alloy_ui_ship::{crash, keys, UreqFetch};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

type Posts = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// Start a server. `routes` gets the base URL (so a manifest can point back at
/// the server) and returns the GET routes. POST bodies are recorded.
fn serve(routes: impl FnOnce(&str) -> HashMap<String, Vec<u8>>) -> (String, Posts) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let routes = Arc::new(routes(&base));
    let posts: Posts = Arc::default();
    let recorded = posts.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (routes, posts) = (routes.clone(), recorded.clone());
            std::thread::spawn(move || handle(s, &routes, &posts));
        }
    });
    (base, posts)
}

fn handle(mut s: std::net::TcpStream, routes: &HashMap<String, Vec<u8>>, posts: &Posts) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut first = head.lines().next().unwrap().split(' ');
    let (method, path) = (first.next().unwrap(), first.next().unwrap().to_string());
    let (status, body) = if method == "POST" {
        let len: usize = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse().unwrap())
            })
            .unwrap_or(0);
        while buf.len() < head_end + len {
            let n = s.read(&mut chunk).unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        posts
            .lock()
            .unwrap()
            .push((path, buf[head_end..head_end + len].to_vec()));
        ("200 OK", b"ok".to_vec())
    } else {
        match routes.get(&path) {
            Some(b) => ("200 OK", b.clone()),
            None => ("404 Not Found", vec![]),
        }
    };
    let _ = write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(&body);
}

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("alloy-ship-{tag}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn update_over_http_downloads_verifies_and_replaces() {
    let k = keys::generate().unwrap();
    let payload: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let (base, _) = serve(|base| {
        let m = Manifest {
            schema: 1,
            app: "dev.x.app".into(),
            version: "2.0.0".into(),
            notes: "n".into(),
            artifacts: vec![Artifact {
                os: "linux".into(),
                arch: "x86_64".into(),
                format: "appimage".into(),
                url: format!("{base}/dl/app-2.bin"),
                sha256: keys::hex(&Sha256::digest(&payload)),
                size: payload.len() as u64,
            }],
        };
        let json = m.to_json();
        let sig = keys::sign(&k.private_hex, json.as_bytes()).unwrap();
        HashMap::from([
            ("/latest.json".to_string(), json.into_bytes()),
            ("/latest.json.sig".to_string(), sig.into_bytes()),
            ("/dl/app-2.bin".to_string(), payload.clone()),
        ])
    });
    let mut cfg = Config::new(
        "dev.x.app",
        "1.0.0",
        &format!("{base}/latest.json"),
        &k.public_hex,
    );
    cfg.os = "linux".into();
    cfg.arch = "x86_64".into();
    cfg.formats = vec!["appimage".into()];
    let u = Updater::new(cfg, UreqFetch::default());
    let up = u.check().unwrap().expect("update available");
    assert_eq!(up.version, "2.0.0");
    let dir = tmp("http");
    let file = u.download(&up, &dir).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), payload);

    // `apply` for an AppImage replaces the file named by $APPIMAGE.
    #[cfg(target_os = "linux")]
    {
        let app = dir.join("Demo.AppImage");
        std::fs::write(&app, b"old").unwrap();
        std::env::set_var("APPIMAGE", &app);
        let r = alloy_ui_ship::update::apply(&file, &up.artifact).unwrap();
        assert_eq!(r, Applied::Replaced(app.clone()));
        assert_eq!(std::fs::read(&app).unwrap(), payload);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = Applied::Replaced(file);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_404_manifest_is_a_network_error_not_a_panic() {
    let (base, _) = serve(|_| HashMap::new());
    let k = keys::generate().unwrap();
    let cfg = Config::new("a", "1.0.0", &format!("{base}/latest.json"), &k.public_hex);
    assert!(matches!(
        Updater::new(cfg, UreqFetch::default()).check(),
        Err(alloy_ui_ship::Error::Network(_))
    ));
}

#[test]
fn crash_upload_over_http() {
    let (base, posts) = serve(|_| HashMap::new());
    let dir = tmp("crash-http");
    let r = crash::Report {
        app: "a".into(),
        version: "1".into(),
        os: "linux".into(),
        arch: "x86_64".into(),
        time: 5,
        thread: "main".into(),
        message: "boom".into(),
        location: "x.rs:1:1".into(),
        backtrace: "bt".into(),
    };
    crash::write_report(&dir, &r, 5).unwrap();
    let n = crash::upload_pending(&dir, &format!("{base}/crash"), &UreqFetch::default()).unwrap();
    assert_eq!(n, 1);
    let got = posts.lock().unwrap();
    assert_eq!(got[0].0, "/crash");
    assert!(String::from_utf8_lossy(&got[0].1).contains("boom"));
    assert!(crash::list(&dir).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}
