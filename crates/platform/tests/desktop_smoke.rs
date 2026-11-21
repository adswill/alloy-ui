//! Runtime smoke test for the desktop integration: opens a real window and drives the
//! native menu bar, tray icon, notifications and file dialogs through the OS.
//!
//! macOS and Windows run it on CI for real. Linux runs it under Xvfb with a session bus; the
//! menu bar and tray there live in another process (a global-menu panel, a tray watcher), so
//! the Linux checks only pin down that the calls fail cleanly when nobody answers.
//! Without a display the test skips itself.
//!
//! It has its own `main` (`harness = false`) because windowing must run on the main thread.

use alloy_ui_core::Size;
use alloy_ui_platform::{
    menu::Menu,
    notify::{self, Notification},
    testing,
    tray::TraySettings,
    PlatformEvent, ShellContext, ShellHandler, WindowSettings,
};
use std::time::Duration;

fn menus() -> Vec<Menu> {
    vec![
        Menu::new("File")
            .action("file:new", "New", Some("Mod+N"))
            .separator()
            .submenu(Menu::new("Recent").action("file:recent", "a.txt", None))
            .action("file:quit", "Quit", Some("Mod+Q")),
        Menu::new("Edit").action("edit:undo", "Undo", Some("Mod+Z")),
    ]
}

fn tray() -> TraySettings {
    let (w, h) = (16u32, 16u32);
    TraySettings {
        tooltip: "Alloy smoke test".into(),
        icon_rgba: [30u8, 120, 255, 255].repeat((w * h) as usize),
        icon_width: w,
        icon_height: h,
        menu: Some(Menu::new("Tray").action("tray:show", "Show", None).action(
            "tray:quit",
            "Quit",
            None,
        )),
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Phase {
    WaitMenu,
    WaitTray,
    Done,
}

struct Smoke {
    phase: Phase,
    failures: Vec<String>,
}

impl Smoke {
    fn check(&mut self, ok: bool, what: &str) {
        println!("  {} {what}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            self.failures.push(what.to_string());
        }
    }
}

const NATIVE_MENU_AND_TRAY: bool = cfg!(any(target_os = "macos", windows));

impl ShellHandler for Smoke {
    fn started(&mut self, ctx: &mut ShellContext) {
        println!("window:");
        let size = ctx.logical_size();
        self.check(size.width > 0.0 && size.height > 0.0, "window has a size");
        self.check(ctx.scale_factor() > 0.0, "window has a scale factor");

        println!("menu bar:");
        let r = ctx.set_menu(&menus());
        if NATIVE_MENU_AND_TRAY {
            self.check(r.is_ok(), &format!("set_menu succeeds ({r:?})"));
            self.check(
                testing::menu_outline() == ["File", "Edit"],
                &format!(
                    "OS holds the File and Edit menus ({:?})",
                    testing::menu_outline()
                ),
            );
            // Installing it again (the app does so on every change) must replace, not stack.
            self.check(ctx.set_menu(&menus()).is_ok(), "set_menu twice succeeds");
            self.check(
                testing::menu_outline().len() == 2,
                "second set_menu replaced the first",
            );
        } else {
            // No global-menu panel on a bare Xvfb: the call must say so rather than hang or crash.
            println!("  info set_menu without a global-menu panel -> {r:?}");
        }

        println!("tray:");
        let r = ctx.set_tray(&tray());
        if NATIVE_MENU_AND_TRAY {
            self.check(r.is_ok(), &format!("set_tray succeeds ({r:?})"));
            self.check(testing::tray_visible(), "OS shows the tray icon");
        } else {
            println!("  info set_tray without a tray watcher -> {r:?}");
        }
        let bad = TraySettings {
            icon_rgba: vec![0; 3],
            icon_width: 2,
            icon_height: 2,
            ..tray()
        };
        self.check(
            ctx.set_tray(&bad).is_err(),
            "set_tray rejects a mismatched icon buffer",
        );

        println!("notification:");
        let r = notify::send(&Notification {
            title: "Alloy smoke test".into(),
            body: "native notification \"quoted\" & <escaped>".into(),
            timeout_ms: Some(1000),
        });
        if cfg!(target_os = "linux") {
            println!("  info notify without a daemon -> {r:?}");
        } else {
            self.check(r.is_ok(), &format!("notification posts ({r:?})"));
        }

        println!("file dialogs:");
        dialogs(self);

        if NATIVE_MENU_AND_TRAY {
            // Last on purpose: a modal dialog above runs its own message loop, which would
            // swallow a click posted before it (the shell's message hook never sees it).
            self.check(testing::activate_menu_item("file:new"), "click File > New");
        }
        if !NATIVE_MENU_AND_TRAY {
            self.phase = Phase::Done;
            ctx.exit();
        }
    }

    fn event(&mut self, ctx: &mut ShellContext, event: PlatformEvent) {
        match (self.phase, event) {
            (Phase::WaitMenu, PlatformEvent::MenuAction(id)) => {
                self.check(
                    id == "file:new",
                    &format!("menu click arrives as MenuAction(file:new), got {id}"),
                );
                self.check(
                    testing::activate_tray_item("tray:show"),
                    "click tray > Show",
                );
                self.phase = Phase::WaitTray;
            }
            (Phase::WaitTray, PlatformEvent::MenuAction(id)) => {
                self.check(
                    id == "tray:show",
                    &format!("tray click arrives as MenuAction(tray:show), got {id}"),
                );
                // Re-installing the menu bar must not break the tray menu (separate id tables).
                self.check(
                    ctx.set_menu(&menus()).is_ok(),
                    "menu bar re-install after tray",
                );
                self.check(
                    testing::activate_tray_item("tray:quit"),
                    "tray still works after it",
                );
                self.phase = Phase::Done;
            }
            (Phase::Done, PlatformEvent::MenuAction(id)) => {
                self.check(
                    id == "tray:quit",
                    &format!("tray id table survived the menu re-install, got {id}"),
                );
                ctx.clear_tray();
                self.check(!testing::tray_visible(), "clear_tray removes the icon");
                ctx.exit();
            }
            _ => {}
        }
    }

    fn redraw(&mut self, _ctx: &mut ShellContext) {}
}

#[cfg(target_os = "macos")]
fn dialogs(s: &mut Smoke) {
    use alloy_ui_platform::dialog::{DialogKind, FileDialogOptions, FileFilter};
    // Showing a modal panel needs a person; build the real AppKit panels and read them back.
    let open = FileDialogOptions {
        kind: DialogKind::OpenFiles,
        title: Some("Pick".into()),
        filters: vec![FileFilter {
            name: "Text".into(),
            extensions: vec!["txt".into()],
        }],
        ..Default::default()
    };
    let got = testing::dialog_panel_summary(&open).unwrap_or_default();
    s.check(
        got == "open files=true dirs=false multi=true message=Pick",
        &format!("open panel configured: {got}"),
    );
    let folder = FileDialogOptions {
        kind: DialogKind::Folder,
        ..Default::default()
    };
    let got = testing::dialog_panel_summary(&folder).unwrap_or_default();
    s.check(
        got.starts_with("open files=false dirs=true multi=false"),
        &format!("folder panel configured: {got}"),
    );
    let save = FileDialogOptions {
        kind: DialogKind::Save,
        name: "notes.txt".into(),
        ..Default::default()
    };
    let got = testing::dialog_panel_summary(&save).unwrap_or_default();
    s.check(
        got.starts_with("save name=notes.txt"),
        &format!("save panel configured: {got}"),
    );
}

#[cfg(windows)]
fn dialogs(s: &mut Smoke) {
    use alloy_ui_platform::dialog::{DialogKind, FileDialogOptions, FileFilter};
    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        System::Threading::GetCurrentProcessId,
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
            WM_CLOSE,
        },
    };
    // The common file dialog is a modal "#32770" window of this process. A helper thread
    // closes it (the same as pressing Cancel), so the real IFileDialog runs end to end.
    unsafe extern "system" fn close_dialog(hwnd: HWND, found: LPARAM) -> BOOL {
        let mut pid = 0;
        // SAFETY: plain queries on a window handle EnumWindows just gave us.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut class = [0u16; 32];
            let n = GetClassNameW(hwnd, class.as_mut_ptr(), 32);
            if pid == GetCurrentProcessId()
                && IsWindowVisible(hwnd) != 0
                && String::from_utf16_lossy(&class[..n.max(0) as usize]) == "#32770"
            {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
                *(found as *mut bool) = true;
            }
        }
        1
    }
    let helper = std::thread::spawn(|| {
        for _ in 0..200 {
            std::thread::sleep(Duration::from_millis(100));
            let mut found = false;
            // SAFETY: `found` outlives the synchronous EnumWindows call.
            unsafe { EnumWindows(Some(close_dialog), &mut found as *mut bool as LPARAM) };
            if found {
                return true;
            }
        }
        false
    });
    let opts = FileDialogOptions {
        kind: DialogKind::OpenFile,
        title: Some("Smoke".into()),
        filters: vec![FileFilter {
            name: "Text".into(),
            extensions: vec!["txt".into()],
        }],
        ..Default::default()
    };
    let r = alloy_ui_platform::dialog::show(&opts);
    s.check(
        helper.join().unwrap_or(false),
        "the file dialog really appeared",
    );
    s.check(
        matches!(&r, Ok(v) if v.is_empty()),
        &format!("cancelled dialog returns an empty list ({r:?})"),
    );
}

#[cfg(not(any(target_os = "macos", windows)))]
fn dialogs(_s: &mut Smoke) {
    // Linux dialogs shell out to zenity/kdialog; tests/linux_dialogs.rs drives them with a fake.
    println!("  info covered by tests/linux_dialogs.rs");
}

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        println!("desktop_smoke: skipped (no display)");
        return;
    }
    // A hung OS call must fail the run, not stall CI.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(90));
        eprintln!("desktop_smoke: timed out");
        std::process::exit(2);
    });
    println!("desktop_smoke on {}", std::env::consts::OS);
    let result = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let out = result.clone();
    struct Report(Smoke, std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    impl ShellHandler for Report {
        fn started(&mut self, c: &mut ShellContext) {
            self.0.started(c);
            if NATIVE_MENU_AND_TRAY
                && self.0.phase == Phase::WaitMenu
                && !self.0.failures.is_empty()
            {
                c.exit(); // a failed set-up has nothing left to wait for
            }
        }
        fn event(&mut self, c: &mut ShellContext, e: PlatformEvent) {
            self.0.event(c, e);
        }
        fn redraw(&mut self, _c: &mut ShellContext) {}
    }
    impl Drop for Report {
        fn drop(&mut self) {
            *self.1.lock().unwrap() = self.0.failures.clone();
            if NATIVE_MENU_AND_TRAY && self.0.phase != Phase::Done {
                self.1
                    .lock()
                    .unwrap()
                    .push("the event round trips did not finish".into());
            }
        }
    }
    let run = alloy_ui_platform::run(
        WindowSettings {
            title: "Alloy smoke".into(),
            size: Size::new(480.0, 320.0),
        },
        move |_proxy| {
            Report(
                Smoke {
                    phase: Phase::WaitMenu,
                    failures: Vec::new(),
                },
                out,
            )
        },
    );
    let failures = result.lock().unwrap().clone();
    if let Err(e) = run {
        eprintln!("desktop_smoke: run failed: {e}");
        std::process::exit(1);
    }
    if failures.is_empty() {
        println!("desktop_smoke: all checks passed");
    } else {
        eprintln!("desktop_smoke: {} check(s) failed:", failures.len());
        for f in failures {
            eprintln!("  - {f}");
        }
        std::process::exit(1);
    }
}
