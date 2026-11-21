# Shell and testing

## 26. Test an app without opening a window

`Harness` is the real runtime with scripted input. Click by visible text, type, press chords, then assert on what is on screen.

```rust
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct Greeter {
    name: String,
}

fn view(s: &Greeter) -> impl View<Act<Greeter>> {
    col![
        input(at!(s.name)).hint("Your name"),
        text(if s.name.is_empty() { "Hello, stranger".to_string() } else { format!("Hello, {}", s.name) }),
    ]
    .gap(8.0)
    .pad(16.0)
}

fn main() {
    let mut ui = Harness::new(sketch(Greeter::default(), view), 320.0, 160.0);
    assert!(ui.has_text("Hello, stranger"));
    ui.click("Your name").type_text("Ada");
    assert!(ui.has_text("Hello, Ada"));
}
```

## 27. Save a screenshot from a test

The same harness renders a PNG on the CPU with no window or GPU. Commit it as a golden image, or attach it to a bug report.

```rust
use alloy_ui::{prelude::*, testing::Harness};

fn main() {
    let app = sketch(0i64, |n| col![heading(n.to_string()), button("Add").primary().act(|n| *n += 1)].gap(8.0).pad(20.0));
    let ui = Harness::new(app, 240.0, 140.0);
    let png = ui.png();
    assert_eq!(&png[1..4], b"PNG");
}
```

## 28. A second window

Windows share one state. Open one with `Cmd::open_window(id, title, width, height)` and draw it from `view_window(id)`. `.boxed()` lets the two views have different types.

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Default)]
struct Editor {
    text: String,
}

#[derive(Clone)]
enum Msg {
    Edit(String),
    Stats,
}

impl App for Editor {
    type Msg = Msg;
    fn update_cmd(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Edit(t) => {
                self.text = t;
                Cmd::none()
            }
            Msg::Stats => Cmd::open_window(1, "Statistics", 240.0, 120.0),
        }
    }
    fn view(&self) -> impl View<Msg> {
        col![input(&self.text).on(Msg::Edit), button("Statistics").on(Msg::Stats)].gap(8.0).pad(20.0)
    }
    fn view_window(&self, id: u64) -> impl View<Msg> {
        if id == 0 {
            return self.view().boxed();
        }
        text(format!("{} characters", self.text.chars().count())).pad(20.0).boxed()
    }
}

fn main() {
    Editor::default().run_sized("Editor", 360.0, 160.0)
}
```

## 29. Deny by default

A capability file lists what the app may touch, and everything else is refused. Check a request before acting on it.

```rust
use alloy_ui::ipc::{Capability, CapabilitySet};
use std::path::Path;

fn main() {
    // Absolute paths need a drive letter on Windows; this keeps the example runnable there.
    let home = if cfg!(windows) { "C:/home/me" } else { "/home/me" };
    let caps = CapabilitySet::from_toml(&format!(
        "capabilities = [\"fs_read\"]\n[fs]\nread = [\"{home}/notes\"]"
    ))
    .unwrap();
    assert!(caps.check_fs_read(Path::new(&format!("{home}/notes/today.md"))).is_ok());
    assert!(caps.check_fs_read(Path::new("/etc/passwd")).is_err());
    assert!(caps.check(&Capability::Clipboard).is_err());
}
```

## 30. A typed command between the UI and a task

A `CommandRegistry` holds named async commands with serde arguments and results. Each declares the capabilities it needs, and invoking it checks them first.

```rust
use alloy_ui::ipc::{Capability, CapabilitySet, CommandRegistry};
use serde_json::json;

fn main() {
    let mut commands = CommandRegistry::new();
    commands.register("word_count", &[Capability::FsRead], |text: String| async move {
        Ok::<_, String>(text.split_whitespace().count())
    });
    let home = if cfg!(windows) { "C:/home/me" } else { "/home/me" };
    let granted = CapabilitySet::from_toml(&format!(
        "capabilities = [\"fs_read\"]\n[fs]\nread = [\"{home}\"]"
    ))
    .unwrap();
    let run = commands.invoke("word_count", json!("one two three"), &granted).unwrap();
    assert_eq!(alloy_ui::block_on(run).unwrap(), json!(3));
}
```
