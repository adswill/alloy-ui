//! Runtime checks of the system-integration features against the real OS: rich
//! clipboard, accent and reduced-motion settings, URL-scheme registration, single
//! instance hand-off, deep-link dispatch, global hotkeys and window effects.
//!
//! This is a plain `main` (not libtest) because winit needs the main thread, and
//! because the same binary acts as the "second launch" in the hand-off checks.
//! CI runs it on macOS, Windows and Linux (under Xvfb). Each check says how it
//! was verified; a check that cannot run in the environment prints `SKIP` and
//! the reason, never a silent pass.

use alloy_ui_core::{Size, WindowEffect};
use alloy_ui_platform::{
    deeplink, default_clipboard,
    instance::{single_instance, Single},
    EffectOutcome, PlatformEvent, ShellContext, ShellHandler, WindowSettings,
};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

const APP_ID: &str = "dev.alloy.systemruntime";
const SCHEME: &str = "alloytest";
const URL: &str = "alloytest://handoff/42?x=1";

static FAILURES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn pass(name: &str, detail: &str) {
    println!("PASS  {name}: {detail}");
}
fn skip(name: &str, why: &str) {
    println!("SKIP  {name}: {why}");
}
fn fail(name: &str, why: String) {
    println!("FAIL  {name}: {why}");
    FAILURES.lock().unwrap().push(format!("{name}: {why}"));
}
fn check(name: &str, r: Result<String, String>) {
    match r {
        Ok(d) => pass(name, &d),
        Err(e) => fail(name, e),
    }
}

fn has_display() -> bool {
    cfg!(not(target_os = "linux"))
        || std::env::var_os("DISPLAY").is_some()
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[allow(dead_code)] // only the macOS and Windows checks shell out
fn run_out(prog: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(prog).args(args).output().ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // A second launch: hand the URL to the running instance and leave.
    if args.iter().any(|a| a.starts_with(SCHEME)) {
        let code = match single_instance(APP_ID) {
            Single::Forwarded => 0,
            Single::Primary(_) => 3,
        };
        std::process::exit(code);
    }
    // Started by Launch Services through a registered URL scheme (macOS).
    #[cfg(target_os = "macos")]
    if std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS")) {
        bundle_mode();
        return;
    }

    check("clipboard round trip", clipboard());
    check("settings (accent, reduced motion, dark)", settings());
    check("deep-link registration", registration());
    check("global hotkey register/release", hotkeys());
    #[cfg(target_os = "macos")]
    check("deep link through Launch Services", macos_bundle());
    windowed();

    let failures = FAILURES.lock().unwrap();
    if failures.is_empty() {
        println!("system_runtime: all checks passed");
    } else {
        println!("system_runtime: {} failed", failures.len());
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------- clipboard

fn pic(w: u32, h: u32) -> alloy_ui_platform::ClipContent {
    let rgba = (0..w * h)
        .flat_map(|i| [(i % 251) as u8, (i / 7 % 253) as u8, 90, 255])
        .collect();
    alloy_ui_platform::ClipContent::image(w, h, rgba).unwrap()
}

fn clipboard() -> Result<String, String> {
    if !has_display() {
        skip("clipboard round trip", "no display");
        return Ok("skipped".into());
    }
    let mut cb = default_clipboard();
    // A fresh clipboard object has no in-process fallback, so every read below
    // goes to the real system clipboard.
    let fresh = || default_clipboard();

    cb.write_rich(&alloy_ui_platform::ClipContent::html(
        "<b>bold</b> ü",
        "bold ü",
    ))
    .map_err(|e| e.to_string())?;
    let got = fresh().read_rich().map_err(|e| e.to_string())?;
    if got.html.as_deref() != Some("<b>bold</b> ü") || got.text.as_deref() != Some("bold ü") {
        return Err(format!("html round trip gave {got:?}"));
    }
    // A program that is not us must see both formats.
    foreign_view(true, false)?;

    let img = pic(64, 48);
    cb.write_rich(&img).map_err(|e| e.to_string())?;
    let got = fresh().read_rich().map_err(|e| e.to_string())?;
    if got.image != img.image {
        return Err("image changed on the way through the clipboard".into());
    }
    foreign_view(false, true)?;

    let dir = std::env::temp_dir();
    let file = dir.join("alloy-clip-test.txt");
    std::fs::write(&file, "x").map_err(|e| e.to_string())?;
    cb.write_rich(&alloy_ui_platform::ClipContent::files([file.clone()]))
        .map_err(|e| e.to_string())?;
    let got = fresh().read_rich().map_err(|e| e.to_string())?;
    let canon = |p: &std::path::PathBuf| std::fs::canonicalize(p).unwrap_or(p.clone());
    if got.files.iter().map(canon).collect::<Vec<_>>() != vec![canon(&file)] {
        return Err(format!("files round trip gave {:?}", got.files));
    }
    let _ = std::fs::remove_file(file);
    Ok("html + text, image, files read back through a fresh clipboard".into())
}

/// Ask the OS, not Alloy, what is on the clipboard.
fn foreign_view(html: bool, image: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let info = run_out("osascript", &["-e", "clipboard info"]).ok_or("osascript failed")?;
        if html && !info.contains("HTML") {
            return Err(format!("macOS clipboard has no HTML type: {info}"));
        }
        if image && !(info.contains("PNGf") || info.contains("TIFF")) {
            return Err(format!("macOS clipboard has no image type: {info}"));
        }
    }
    #[cfg(windows)]
    {
        let script = "Add-Type -AssemblyName System.Windows.Forms; \
            $c=[Windows.Forms.Clipboard]; \
            Write-Output ('IMG=' + $c::ContainsImage()); \
            Write-Output ('HTML=' + $c::ContainsData('HTML Format')); \
            Write-Output ('TEXT=' + $c::ContainsText())";
        let out = run_out("powershell", &["-NoProfile", "-STA", "-Command", script])
            .ok_or("powershell failed")?;
        if html && !out.contains("HTML=True") {
            return Err(format!("Windows clipboard has no HTML Format: {out}"));
        }
        if image && !out.contains("IMG=True") {
            return Err(format!("Windows clipboard has no image: {out}"));
        }
    }
    let _ = (html, image);
    Ok(())
}

// ----------------------------------------------------------------- settings

fn settings() -> Result<String, String> {
    use alloy_ui_platform::settings::system_settings;
    #[cfg(windows)]
    {
        let reg = |args: &[&str]| {
            Command::new("reg")
                .args(args)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        let dwm = r"HKCU\Software\Microsoft\Windows\DWM";
        // ABGR 0xFFB86800 is a clearly non-default accent (0x0068B8 = RGB(0,104,184) reversed).
        if !reg(&[
            "add",
            dwm,
            "/v",
            "AccentColor",
            "/t",
            "REG_DWORD",
            "/d",
            "0xFFB86800",
            "/f",
        ]) {
            return Err("cannot write the accent registry value".into());
        }
        let s = system_settings();
        if s.accent != Some([0x00, 0x68, 0xb8]) {
            return Err(format!("accent read back as {:?}", s.accent));
        }
        // Animations off must read as reduced motion.
        set_windows_animations(false)?;
        let off = system_settings();
        set_windows_animations(true)?;
        let on = system_settings();
        if !off.reduced_motion || on.reduced_motion {
            return Err(format!(
                "reduced_motion did not follow the setting: off={} on={}",
                off.reduced_motion, on.reduced_motion
            ));
        }
        let pers = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
        let write_light = |v: &str| {
            reg(&[
                "add",
                pers,
                "/v",
                "AppsUseLightTheme",
                "/t",
                "REG_DWORD",
                "/d",
                v,
                "/f",
            ])
        };
        write_light("0");
        let dark = system_settings().dark;
        write_light("1");
        let light = !system_settings().dark;
        if !(dark && light) {
            return Err(format!(
                "dark mode did not follow the registry: dark={dark} light={light}"
            ));
        }
        Ok(format!(
            "accent, animations and dark mode follow the OS; now {:?}",
            system_settings()
        ))
    }
    #[cfg(target_os = "macos")]
    {
        // The accent lives in the user's global domain, which any process may write.
        let w = |v: &str| {
            run_out("defaults", &["write", "-g", "AppleAccentColor", "-int", v]).is_some()
        };
        if !w("5") {
            return Err("cannot write AppleAccentColor".into());
        }
        let purple = system_settings().accent;
        w("0");
        let red = system_settings().accent;
        let _ = run_out("defaults", &["delete", "-g", "AppleAccentColor"]);
        if purple.is_none() || red.is_none() || purple == red {
            return Err(format!(
                "accent did not follow `defaults`: {purple:?} vs {red:?}"
            ));
        }
        // Reduced motion: whatever `defaults` says must be what we report. Writing the
        // accessibility domain can be refused by the OS, so only the agreement is checked.
        let _ = run_out(
            "defaults",
            &[
                "write",
                "com.apple.universalaccess",
                "reduceMotion",
                "-bool",
                "true",
            ],
        );
        let said = run_out(
            "defaults",
            &["read", "com.apple.universalaccess", "reduceMotion"],
        );
        let expect = said.as_deref().map(str::trim) == Some("1");
        let got = system_settings().reduced_motion;
        let _ = run_out(
            "defaults",
            &["delete", "com.apple.universalaccess", "reduceMotion"],
        );
        if got != expect {
            return Err(format!("reduced_motion={got} but `defaults` says {said:?}"));
        }
        Ok(format!(
            "accent follows `defaults` ({purple:?} then {red:?}); reduced_motion agrees with it ({expect})"
        ))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let s = system_settings();
        Ok(format!(
            "read without error: {s:?} (Linux parsing has its own tests)"
        ))
    }
}

#[cfg(windows)]
fn set_windows_animations(on: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETCLIENTAREAANIMATION,
    };
    // SAFETY: SPI_SETCLIENTAREAANIMATION takes the BOOL in pvParam by value.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETCLIENTAREAANIMATION,
            0,
            usize::from(on) as *mut std::ffi::c_void,
            SPIF_SENDCHANGE,
        )
    };
    (ok != 0)
        .then_some(())
        .ok_or_else(|| "SystemParametersInfoW refused".into())
}

// ------------------------------------------------------------ registration

#[allow(clippy::needless_return)] // the per-OS blocks each end in `return`
fn registration() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        // macOS reads schemes from the bundle; the run-time call must say so rather than lie.
        return match deeplink::register_scheme(APP_ID, SCHEME) {
            Err(e) if e.0.contains("Info.plist") || e.0.contains("bundle") => {
                Ok("run-time registration refused with the bundle hint (see the Launch Services check)".into())
            }
            other => Err(format!("unexpected result {other:?}")),
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        deeplink::register_scheme(APP_ID, SCHEME).map_err(|e| e.to_string())?;
        let exe = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        #[cfg(windows)]
        {
            let key = format!(r"HKCU\Software\Classes\{SCHEME}\shell\open\command");
            let out = run_out("reg", &["query", &key, "/ve"]).ok_or("scheme key missing")?;
            if !out.contains(&exe) || !out.contains("%1") {
                return Err(format!("command does not launch us with the URL: {out}"));
            }
            let proto = run_out(
                "reg",
                &[
                    "query",
                    &format!(r"HKCU\Software\Classes\{SCHEME}"),
                    "/v",
                    "URL Protocol",
                ],
            );
            if proto.is_none() {
                return Err("`URL Protocol` value missing".into());
            }
            let r = Ok(format!(
                "HKCU\\Software\\Classes\\{SCHEME} launches {exe} \"%1\""
            ));
            // Left registered: the windowed stage launches the URL through the Windows shell.
            return r;
        }
        #[cfg(not(windows))]
        {
            let home = std::env::var("XDG_DATA_HOME")
                .ok()
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| std::path::PathBuf::from(h).join(".local/share"))
                })
                .ok_or("no home")?;
            let f = home.join("applications").join(format!("{APP_ID}.desktop"));
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let _ = std::fs::remove_file(&f);
            if !text.contains(&format!("x-scheme-handler/{SCHEME}")) || !text.contains(&exe) {
                return Err(format!("desktop entry is wrong:\n{text}"));
            }
            Ok("desktop entry names this executable and the scheme".into())
        }
    }
}

// ------------------------------------------------------------------ hotkeys

fn hotkeys() -> Result<String, String> {
    use alloy_ui_platform::hotkey;
    if !has_display() {
        skip("global hotkey register/release", "no display");
        return Ok("skipped".into());
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Ok("Wayland: needs the portal, see the unit tests with a fake portal".into());
    }
    hotkey::register("ctrl+alt+k").map_err(|e| format!("first registration: {e}"))?;
    // The same key cannot be taken twice.
    let again = hotkey::register("ctrl+alt+k");
    hotkey::unregister_all();
    if cfg!(not(target_os = "macos")) && again.is_ok() {
        return Err("a second registration of the same key was accepted".into());
    }
    hotkey::register("ctrl+alt+k").map_err(|e| format!("not free after release: {e}"))?;
    hotkey::unregister_all();
    if hotkey::register("ctrl+alt+nonsense").is_ok() {
        return Err("an unparsable key was accepted".into());
    }
    Ok("register, refuse duplicate, release, re-register".into())
}

// -------------------------------------------------------- macOS Launch Services

#[cfg(target_os = "macos")]
fn app_bundle() -> std::path::PathBuf {
    // Launch Services ignores bundles under the per-user temp directory.
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Applications/AlloySystemRuntime.app")
}
#[cfg(target_os = "macos")]
fn bundle_log(line: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("alloy-bundle-log.txt"))
    {
        let _ = writeln!(f, "{line}");
    }
}
#[cfg(target_os = "macos")]
fn bundle_result() -> std::path::PathBuf {
    std::env::temp_dir().join("alloy-bundle-result.txt")
}

#[cfg(target_os = "macos")]
fn macos_bundle() -> Result<String, String> {
    let app = app_bundle();
    let _ = std::fs::remove_dir_all(&app);
    let _ = std::fs::remove_file(bundle_result());
    let _ = std::fs::remove_file(std::env::temp_dir().join("alloy-bundle-log.txt"));
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).map_err(|e| e.to_string())?;
    std::fs::copy(
        std::env::current_exe().unwrap(),
        macos.join("system_runtime"),
    )
    .map_err(|e| e.to_string())?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{APP_ID}</string>
<key>CFBundleName</key><string>AlloySystemRuntime</string>
<key>CFBundleExecutable</key><string>system_runtime</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleURLTypes</key><array><dict>
<key>CFBundleURLName</key><string>{APP_ID}</string>
<key>CFBundleURLSchemes</key><array><string>{SCHEME}</string></array>
</dict></array>
</dict></plist>"#
    );
    std::fs::write(app.join("Contents/Info.plist"), plist).map_err(|e| e.to_string())?;
    let _ = Command::new("codesign")
        .args(["--force", "--sign", "-"])
        .arg(&app)
        .status();
    let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
    let _ = Command::new(lsregister)
        .args(["-f", "-R", "-trusted"])
        .arg(&app)
        .status();
    std::thread::sleep(Duration::from_secs(2));
    let st = Command::new("open")
        .arg(URL)
        .status()
        .map_err(|e| e.to_string())?;
    if !st.success() {
        return Err("`open` could not resolve the scheme to the bundle".into());
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(25) {
        if let Ok(got) = std::fs::read_to_string(bundle_result()) {
            let _ = std::fs::remove_dir_all(&app);
            return if got.trim() == URL {
                Ok(
                    "`open alloytest://` started the bundle and the Apple Event arrived as Open"
                        .into(),
                )
            } else {
                Err(format!("the bundle received {got:?}"))
            };
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = std::fs::remove_dir_all(&app);
    let log = std::fs::read_to_string(std::env::temp_dir().join("alloy-bundle-log.txt"))
        .unwrap_or_else(|_| "(the bundle never started: no log)".into());
    Err(format!(
        "the bundle never reported the URL; its log:\n{log}"
    ))
}

/// Inside the bundle: wait for the Apple Event, report the URL, exit.
#[cfg(target_os = "macos")]
fn bundle_mode() {
    struct L;
    impl log::Log for L {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, r: &log::Record) {
            bundle_log(&format!("log {}: {}", r.target(), r.args()));
        }
        fn flush(&self) {}
    }
    let _ = log::set_logger(&L);
    log::set_max_level(log::LevelFilter::Debug);
    bundle_log(&format!(
        "bundle started, args {:?}",
        std::env::args().collect::<Vec<_>>()
    ));
    struct Wait;
    impl ShellHandler for Wait {
        fn event(&mut self, ctx: &mut ShellContext, e: PlatformEvent) {
            bundle_log(&format!("event {e:?}"));
            if let PlatformEvent::Open(a) = e {
                if let Some(u) = a.iter().find(|a| a.starts_with(SCHEME)) {
                    let _ = std::fs::write(bundle_result(), u);
                    ctx.exit();
                }
            }
        }
        fn redraw(&mut self, _: &mut ShellContext) {}
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(40));
        std::process::exit(1);
    });
    let r = alloy_ui_platform::run(window("bundle"), |_| Wait);
    bundle_log(&format!("run returned {r:?}"));
}

// ------------------------------------------------------------------ windowed

fn window(title: &str) -> WindowSettings {
    WindowSettings {
        title: title.into(),
        size: Size {
            width: 320.0,
            height: 240.0,
        },
    }
}

/// Single-instance hand-off, deep-link dispatch, a real hotkey press (Windows and
/// X11) and every window effect, all inside one real event loop.
fn windowed() {
    if !has_display() {
        skip("windowed checks", "no display (run under Xvfb)");
        return;
    }
    let guard = match single_instance(APP_ID) {
        Single::Primary(g) => g,
        Single::Forwarded => {
            fail(
                "single instance",
                "a stale instance answered the first launch".into(),
            );
            return;
        }
    };
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(60));
        println!("FAIL  windowed: timed out waiting for events");
        std::process::exit(1);
    });

    struct Probe {
        proxy: alloy_ui_platform::shell::ShellProxy,
        opened: Option<Vec<String>>,
        hotkey: bool,
        expects_hotkey: bool,
        done: bool,
    }
    impl ShellHandler for Probe {
        fn started(&mut self, _: &mut ShellContext) {
            let (proxy, exe) = (self.proxy.clone(), std::env::current_exe().unwrap());
            // The second launch: a separate process, exactly as the OS would start it.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                // On Windows the launch goes through the shell and the registry entry written by
                // `register_scheme`, as clicking a link would; elsewhere the process is started directly.
                let mut cmd = if cfg!(windows) {
                    let mut c = Command::new("cmd");
                    c.args(["/C", "start", "", URL]);
                    c
                } else {
                    let mut c = Command::new(&exe);
                    c.arg(URL);
                    c
                };
                match cmd.status() {
                    Ok(s) if s.success() => {}
                    other => fail(
                        "single instance",
                        format!("second launch ended with {other:?}"),
                    ),
                }
                proxy.wake();
            });
            self.expects_hotkey = start_hotkey_press(self.proxy.clone());
        }
        fn event(&mut self, ctx: &mut ShellContext, e: PlatformEvent) {
            match e {
                PlatformEvent::Open(a) if a.iter().any(|a| a.starts_with(SCHEME)) => {
                    self.opened = Some(a)
                }
                PlatformEvent::GlobalHotkey(k) if k == "ctrl+alt+j" => self.hotkey = true,
                _ => {}
            }
            if self.done || self.opened.is_none() || (self.expects_hotkey && !self.hotkey) {
                return;
            }
            self.done = true;
            let args = self.opened.take().unwrap();
            let got = deeplink::classify(&args);
            check(
                "single-instance hand-off + deep-link dispatch",
                if got.urls == [URL] {
                    Ok(format!(
                        "second process forwarded {URL}; the first received it as Open"
                    ))
                } else {
                    Err(format!("first instance received {args:?}"))
                },
            );
            if self.expects_hotkey {
                pass(
                    "global hotkey press",
                    "a synthesised Ctrl+Alt+J reached the app through the global grab",
                );
            } else {
                skip(
                    "global hotkey press",
                    "no way to press keys on this OS/session",
                );
            }
            check("window effects", effects(ctx));
            ctx.exit();
        }
        fn redraw(&mut self, _: &mut ShellContext) {}
    }

    let result = alloy_ui_platform::run(window("system_runtime"), |proxy| Probe {
        proxy,
        opened: None,
        hotkey: false,
        expects_hotkey: false,
        done: false,
    });
    if let Err(e) = result {
        fail("windowed", format!("shell failed: {e}"));
    }
    drop(guard);
    #[cfg(windows)]
    {
        let _ = run_out(
            "reg",
            &["delete", &format!(r"HKCU\Software\Classes\{SCHEME}"), "/f"],
        );
    }
}

fn effects(ctx: &mut ShellContext) -> Result<String, String> {
    let mut report = Vec::new();
    for e in [
        WindowEffect::Mica,
        WindowEffect::Acrylic,
        WindowEffect::Blur,
        WindowEffect::Transparent,
        WindowEffect::None,
    ] {
        let o = ctx.set_effect(e);
        report.push(format!("{e:?}={o:?}"));
        #[cfg(target_os = "macos")]
        if o != EffectOutcome::Applied {
            return Err(format!("macOS must apply every effect, {e:?} gave {o:?}"));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = EffectOutcome::Applied;
    }
    Ok(report.join(" "))
}

/// Register ctrl+alt+j and press it for real. Returns true when a press was sent.
fn start_hotkey_press(proxy: alloy_ui_platform::shell::ShellProxy) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_MENU,
        };
        if let Err(e) = alloy_ui_platform::hotkey::register("ctrl+alt+j") {
            fail("global hotkey press", e.to_string());
            return false;
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            for (vk, up) in [
                (VK_CONTROL, 0),
                (VK_MENU, 0),
                (0x4a, 0),
                (0x4a, KEYEVENTF_KEYUP),
                (VK_MENU, KEYEVENTF_KEYUP),
                (VK_CONTROL, KEYEVENTF_KEYUP),
            ] {
                // SAFETY: keybd_event takes plain integers and synthesises one key transition.
                unsafe { keybd_event(vk as u8, 0, up, 0) };
                std::thread::sleep(Duration::from_millis(30));
            }
            proxy.wake();
        });
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        let have = Command::new("xdotool").arg("version").output().is_ok();
        if std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var_os("DISPLAY").is_none()
            || !have
        {
            return false;
        }
        if let Err(e) = alloy_ui_platform::hotkey::register("ctrl+alt+j") {
            fail("global hotkey press", e.to_string());
            return false;
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            let _ = Command::new("xdotool").args(["key", "ctrl+alt+j"]).status();
            proxy.wake();
        });
        return true;
    }
    #[allow(unreachable_code)]
    {
        let _ = proxy;
        false
    }
}
