//! End-to-end check of the Linux AT-SPI2 server against the real thing: the
//! reference AT-SPI client library (libatspi, via PyGObject) talking to an
//! accessibility bus. Skipped unless `ALLOY_ATSPI_E2E=1`, because it needs a
//! session bus with `at-spi-bus-launcher`; `tests/atspi_e2e.sh` sets that up.
#![cfg(target_os = "linux")]

use alloy_ui_core::{Point, Rectangle, Size};
use alloy_ui_platform::access::{
    atspi_adapter_for_tests, AccessAction, AccessId, AccessNode, AccessRange, AccessRequest,
    AccessRole, AccessTree,
};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

fn node(id: u64, role: AccessRole, label: &str, x: f32, y: f32) -> AccessNode {
    AccessNode {
        id: AccessId(id),
        role,
        label: label.into(),
        value: None,
        checked: None,
        focused: false,
        enabled: true,
        bounds: Rectangle {
            origin: Point { x, y },
            size: Size {
                width: 80.0,
                height: 24.0,
            },
        },
        range: None,
    }
}

fn tree(save_label: &str, focus: Option<u64>) -> AccessTree {
    let mut nodes = vec![
        node(1, AccessRole::Button, save_label, 10.0, 10.0),
        node(2, AccessRole::Checkbox, "Dark mode", 10.0, 50.0),
        node(3, AccessRole::Slider, "Volume", 10.0, 90.0),
        node(4, AccessRole::TextInput, "Name", 10.0, 130.0),
    ];
    nodes[1].checked = Some(true);
    nodes[2].value = Some("30".into());
    nodes[2].range = Some(AccessRange {
        min: 0.0,
        max: 100.0,
        step: 5.0,
    });
    nodes[3].value = Some("Ada".into());
    for n in &mut nodes {
        n.focused = Some(n.id.0) == focus;
    }
    AccessTree {
        title: "Test window".into(),
        nodes,
    }
}

fn find<'a>(v: &'a serde_like::Json, name: &str) -> Option<&'a serde_like::Json> {
    v.get("children")?
        .as_array()?
        .iter()
        .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(name))
}

/// Tiny JSON reader: the test must not pull a JSON crate into the workspace.
mod serde_like {
    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Num(f64),
        Str(String),
        Arr(Vec<Json>),
        Obj(Vec<(String, Json)>),
    }
    impl Json {
        pub fn get(&self, k: &str) -> Option<&Json> {
            match self {
                Json::Obj(o) => o.iter().find(|(n, _)| n == k).map(|(_, v)| v),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            if let Json::Str(s) = self {
                Some(s)
            } else {
                None
            }
        }
        pub fn as_array(&self) -> Option<&Vec<Json>> {
            if let Json::Arr(a) = self {
                Some(a)
            } else {
                None
            }
        }
        pub fn as_f64(&self) -> Option<f64> {
            if let Json::Num(n) = self {
                Some(*n)
            } else {
                None
            }
        }
        pub fn has_str(&self, s: &str) -> bool {
            self.as_array()
                .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(s)))
        }
    }
    pub fn parse(s: &str) -> Json {
        let b: Vec<char> = s.chars().collect();
        let mut i = 0;
        val(&b, &mut i)
    }
    fn ws(b: &[char], i: &mut usize) {
        while *i < b.len() && b[*i].is_whitespace() {
            *i += 1;
        }
    }
    fn val(b: &[char], i: &mut usize) -> Json {
        ws(b, i);
        match b[*i] {
            '{' => {
                *i += 1;
                let mut o = Vec::new();
                loop {
                    ws(b, i);
                    if b[*i] == '}' {
                        *i += 1;
                        return Json::Obj(o);
                    }
                    if b[*i] == ',' {
                        *i += 1;
                        continue;
                    }
                    let Json::Str(k) = val(b, i) else {
                        panic!("key")
                    };
                    ws(b, i);
                    *i += 1; // ':'
                    o.push((k, val(b, i)));
                }
            }
            '[' => {
                *i += 1;
                let mut a = Vec::new();
                loop {
                    ws(b, i);
                    if b[*i] == ']' {
                        *i += 1;
                        return Json::Arr(a);
                    }
                    if b[*i] == ',' {
                        *i += 1;
                        continue;
                    }
                    a.push(val(b, i));
                }
            }
            '"' => {
                *i += 1;
                let mut s = String::new();
                while b[*i] != '"' {
                    if b[*i] == '\\' {
                        *i += 1;
                        match b[*i] {
                            'n' => s.push('\n'),
                            'u' => {
                                let h: String = b[*i + 1..*i + 5].iter().collect();
                                s.push(
                                    char::from_u32(u32::from_str_radix(&h, 16).unwrap())
                                        .unwrap_or('?'),
                                );
                                *i += 4;
                            }
                            c => s.push(c),
                        }
                    } else {
                        s.push(b[*i]);
                    }
                    *i += 1;
                }
                *i += 1;
                Json::Str(s)
            }
            't' => {
                *i += 4;
                Json::Bool(true)
            }
            'f' => {
                *i += 5;
                Json::Bool(false)
            }
            'n' => {
                *i += 4;
                Json::Null
            }
            _ => {
                let st = *i;
                while *i < b.len() && "+-.eE0123456789".contains(b[*i]) {
                    *i += 1;
                }
                Json::Num(b[st..*i].iter().collect::<String>().parse().unwrap())
            }
        }
    }
}

#[test]
fn screen_reader_sees_and_drives_the_app() {
    if std::env::var("ALLOY_ATSPI_E2E").is_err() {
        eprintln!("skipped: set ALLOY_ATSPI_E2E=1 (see tests/atspi_e2e.sh)");
        return;
    }
    let requests: Arc<Mutex<Vec<AccessRequest>>> = Arc::default();
    let sink = {
        let r = requests.clone();
        Arc::new(move |req: AccessRequest| r.lock().unwrap().push(req))
    };
    let mut adapter = atspi_adapter_for_tests(7, sink);
    adapter.update(&tree("Save", None));

    let exe = std::env::current_exe().unwrap();
    let app_name = exe.file_name().unwrap().to_string_lossy().into_owned();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/atspi_client.py");
    // The system Python, which is the one that has `gi` (apt python3-gi); `ALLOY_ATSPI_PYTHON` overrides.
    let python = std::env::var("ALLOY_ATSPI_PYTHON").unwrap_or_else(|_| "/usr/bin/python3".into());
    let mut child = Command::new(python)
        .args([script, &app_name])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("system python3 with gi.Atspi (apt: python3-gi gir1.2-atspi-2.0)");
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut stdin = child.stdin.take().unwrap();
    let mut result = None;
    while let Some(Ok(line)) = lines.next() {
        if line == "READY" {
            // The client listens now: change the tree under it.
            adapter.update(&tree("Save as", Some(2)));
            stdin.write_all(b"go\n").unwrap();
        } else if let Some(j) = line.strip_prefix("RESULT ") {
            result = Some(serde_like::parse(j));
        } else {
            eprintln!("client: {line}");
        }
    }
    assert!(child.wait().unwrap().success(), "client failed");
    let out = result.expect("no result from client");

    // What the screen reader saw.
    let app = out.get("app").unwrap();
    let win = &app.get("children").unwrap().as_array().unwrap()[0];
    assert_eq!(win.get("name").unwrap().as_str(), Some("Test window"));
    assert_eq!(win.get("role").unwrap().as_str(), Some("frame"));
    let save = find(win, "Save").expect("Save button");
    assert_eq!(save.get("role").unwrap().as_str(), Some("push button"));
    assert!(save.get("states").unwrap().has_str("focusable"));
    let dark = find(win, "Dark mode").unwrap();
    assert_eq!(dark.get("role").unwrap().as_str(), Some("check box"));
    assert!(dark.get("states").unwrap().has_str("checked"));
    let name = find(win, "Name").unwrap();
    assert_eq!(name.get("role").unwrap().as_str(), Some("entry"));
    assert!(name.get("states").unwrap().has_str("editable"));
    assert_eq!(out.get("name_text").unwrap().as_str(), Some("Ada"));
    let vol: Vec<f64> = out
        .get("volume")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(vol, [0.0, 100.0, 30.0]);
    assert_eq!(out.get("hit").unwrap().as_str(), Some("Save"));
    // Extents are screen coordinates: window origin (100, 50) + node origin.
    let ext = save.get("extents").unwrap().as_array().unwrap();
    assert_eq!(
        (ext[0].as_f64(), ext[1].as_f64()),
        (Some(110.0), Some(60.0))
    );

    // What it asked the app to do.
    let got = requests.lock().unwrap().clone();
    assert!(
        got.contains(&AccessRequest {
            target: AccessId(1),
            action: AccessAction::Press
        }),
        "{got:?}"
    );
    assert!(
        got.contains(&AccessRequest {
            target: AccessId(4),
            action: AccessAction::SetValue("Grace".into())
        }),
        "{got:?}"
    );
    assert!(
        got.contains(&AccessRequest {
            target: AccessId(3),
            action: AccessAction::Increment
        }),
        "{got:?}"
    );
    assert!(
        got.contains(&AccessRequest {
            target: AccessId(2),
            action: AccessAction::Focus
        }),
        "{got:?}"
    );

    // What it heard when the tree changed.
    let events = out.get("events").unwrap().as_array().unwrap();
    let has = |ty: &str, src: &str| {
        events.iter().any(|e| {
            e.get("type")
                .and_then(|t| t.as_str())
                .is_some_and(|t| t.starts_with(ty))
                && e.get("source").and_then(|s| s.as_str()) == Some(src)
        })
    };
    assert!(
        has("object:state-changed:focused", "Dark mode"),
        "{events:?}"
    );
    assert!(
        has("object:property-change:accessible-name", "Save as"),
        "{events:?}"
    );
}
