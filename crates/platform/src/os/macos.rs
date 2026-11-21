//! macOS backend using the stock `pbcopy`/`pbpaste`/`osascript`/`defaults` tools.
//! Compiled by CI on macOS; not run on a Mac by the author.

use super::capture;
use crate::{
    dialog::{self, DialogKind, FileDialogOptions},
    menu::{Menu, MenuItem},
    notify::Notification,
    shortcut::Shortcut,
    theme,
    tray::TraySettings,
    PlatformError, PlatformEvent,
};
use objc2::runtime::AnyObject;
use objc2::{
    define_class, msg_send, rc::Retained, sel, AllocAnyThread, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSImage, NSMenu, NSMenuItem, NSPasteboard, NSPasteboardTypeString, NSStatusBar,
    NSStatusItem,
};
use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSString};
use std::path::PathBuf;
use std::{cell::RefCell, sync::Mutex};

pub fn clipboard_read() -> Option<String> {
    // SAFETY: NSPasteboardTypeString is a valid constant exported by AppKit.
    let pb = NSPasteboard::generalPasteboard();
    unsafe { pb.stringForType(NSPasteboardTypeString) }.map(|s| s.to_string())
}

pub fn clipboard_write(text: &str) -> bool {
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    // SAFETY: as above.
    unsafe { pb.setString_forType(&NSString::from_str(text), NSPasteboardTypeString) }
}

/// Escape a string for use inside an AppleScript double-quoted literal.
pub(crate) fn applescript_quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn open_file() -> Result<Option<PathBuf>, PlatformError> {
    Ok(open_panel(false)?.into_iter().next())
}

pub fn open_files() -> Result<Vec<PathBuf>, PlatformError> {
    open_panel(true)
}

fn open_panel(multiple: bool) -> Result<Vec<PathBuf>, PlatformError> {
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    let Some(mtm) = MainThreadMarker::new() else {
        // AppKit panels are main-thread only; fall back to osascript.
        let script = if multiple {
            "set fs to choose file with multiple selections allowed\nset out to \"\"\nrepeat with f in fs\nset out to out & POSIX path of f & linefeed\nend repeat\nreturn out"
        } else {
            "POSIX path of (choose file)"
        };
        return Ok(capture("osascript", &["-e", script])
            .map(|s| dialog::parse_lines(&s))
            .unwrap_or_default());
    };
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(multiple);
    if panel.runModal() != NSModalResponseOK {
        return Ok(Vec::new());
    }
    Ok(panel
        .URLs()
        .iter()
        .filter_map(|u| u.path())
        .map(|p| PathBuf::from(p.to_string()))
        .collect())
}

pub fn save_file(name: &str) -> Result<Option<PathBuf>, PlatformError> {
    use objc2_app_kit::{NSModalResponseOK, NSSavePanel};
    let Some(mtm) = MainThreadMarker::new() else {
        let script = format!(
            "POSIX path of (choose file name default name \"{}\")",
            applescript_quote(name)
        );
        return Ok(capture("osascript", &["-e", &script])
            .map(|s| PathBuf::from(s.trim()))
            .filter(|p| !p.as_os_str().is_empty()));
    };
    let panel = NSSavePanel::savePanel(mtm);
    panel.setNameFieldStringValue(&NSString::from_str(name));
    if panel.runModal() != NSModalResponseOK {
        return Ok(None);
    }
    Ok(panel
        .URL()
        .and_then(|u| u.path())
        .map(|p| PathBuf::from(p.to_string())))
}

/// Show an `NSOpenPanel`, `NSSavePanel` or folder chooser. AppKit panels only work on the main
/// thread; from any other thread the same dialog is shown through `osascript`.
pub fn file_dialog(o: &FileDialogOptions) -> Result<Vec<PathBuf>, PlatformError> {
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel, NSSavePanel};
    use objc2_foundation::{NSArray, NSURL};
    let Some(mtm) = MainThreadMarker::new() else {
        return Ok(capture("osascript", &["-e", &dialog::applescript(o)])
            .map(|s| dialog::parse_lines(&s))
            .unwrap_or_default());
    };
    let start = o
        .dir
        .as_ref()
        .map(|d| NSURL::fileURLWithPath(&NSString::from_str(&d.display().to_string())));
    let types: Vec<_> = o
        .filters
        .iter()
        .flat_map(|f| f.extensions.iter())
        .map(|e| NSString::from_str(e))
        .collect();
    let types = (!types.is_empty()).then(|| NSArray::from_retained_slice(&types));
    let title = o.title.as_deref().map(NSString::from_str);
    if o.kind == DialogKind::Save {
        let panel = NSSavePanel::savePanel(mtm);
        panel.setNameFieldStringValue(&NSString::from_str(&o.name));
        if let Some(t) = &title {
            panel.setMessage(Some(t));
        }
        if let Some(u) = &start {
            panel.setDirectoryURL(Some(u));
        }
        #[allow(deprecated)] // UTType needs another framework crate; extensions are enough here
        if let Some(t) = &types {
            panel.setAllowedFileTypes(Some(t));
        }
        if panel.runModal() != NSModalResponseOK {
            return Ok(Vec::new());
        }
        return Ok(panel
            .URL()
            .and_then(|u| u.path())
            .map(|p| PathBuf::from(p.to_string()))
            .into_iter()
            .collect());
    }
    let panel = NSOpenPanel::openPanel(mtm);
    let folder = o.kind == DialogKind::Folder;
    panel.setCanChooseFiles(!folder);
    panel.setCanChooseDirectories(folder);
    panel.setAllowsMultipleSelection(o.kind == DialogKind::OpenFiles);
    if let Some(t) = &title {
        panel.setMessage(Some(t));
    }
    if let Some(u) = &start {
        panel.setDirectoryURL(Some(u));
    }
    #[allow(deprecated)] // as above
    if let (Some(t), false) = (&types, folder) {
        panel.setAllowedFileTypes(Some(t));
    }
    if panel.runModal() != NSModalResponseOK {
        return Ok(Vec::new());
    }
    Ok(panel
        .URLs()
        .iter()
        .filter_map(|u| u.path())
        .map(|p| PathBuf::from(p.to_string()))
        .collect())
}

/// Post to Notification Center. `osascript` works from an unbundled binary and any
/// thread; the notification is attributed to Script Editor until the app is bundled
/// and signed (a `UNUserNotificationCenter` follow-up).
pub fn notify(n: &Notification) -> Result<(), PlatformError> {
    capture("osascript", &["-e", &crate::notify::applescript(n)])
        .map(|_| ())
        .ok_or_else(|| PlatformError("notification: osascript failed".into()))
}

pub fn message(title: &str, body: &str) -> Result<(), PlatformError> {
    use objc2_app_kit::NSAlert;
    if let Some(mtm) = MainThreadMarker::new() {
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str(title));
        alert.setInformativeText(&NSString::from_str(body));
        alert.runModal();
        return Ok(());
    }
    let script = format!(
        "display dialog \"{}\" with title \"{}\" buttons {{\"OK\"}} default button \"OK\"",
        applescript_quote(body),
        applescript_quote(title)
    );
    capture("osascript", &["-e", &script]);
    Ok(())
}

pub fn is_dark() -> Option<bool> {
    if let Some(mtm) = MainThreadMarker::new() {
        let name = NSApplication::sharedApplication(mtm)
            .effectiveAppearance()
            .name()
            .to_string();
        return Some(name.contains("Dark"));
    }
    // Off the main thread: the key is absent in light mode, "Dark" in dark mode.
    Some(theme::parse_macos(
        capture("defaults", &["read", "-g", "AppleInterfaceStyle"]).as_deref(),
    ))
}

/// Menu-item tag (index + 1) -> action id.
static IDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the class only reads
    // an item tag and pushes an event, and is main-thread only.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AlloyMenuTarget"]
    struct Target;

    impl Target {
        #[unsafe(method(menuAction:))]
        fn menu_action(&self, sender: &NSMenuItem) {
            let tag = sender.tag();
            if let Ok(ids) = IDS.lock() {
                if let Some(id) = usize::try_from(tag).ok().and_then(|t| t.checked_sub(1)).and_then(|i| ids.get(i)) {
                    super::push_pending(PlatformEvent::MenuAction(id.clone()));
                }
            }
        }

        #[unsafe(method(trayClicked:))]
        fn tray_clicked(&self, _sender: &AnyObject) {
            super::push_pending(PlatformEvent::TrayClicked);
        }
    }

    // SAFETY: `Target` is an `NSObject` subclass with no extra invariants.
    unsafe impl NSObjectProtocol for Target {}
);

type Keep = Option<(Retained<Target>, Option<Retained<NSStatusItem>>)>;

thread_local! {
    // Keep the action target and status item alive; AppKit holds targets weakly.
    static KEEP: RefCell<Keep> =
        const { RefCell::new(None) };
}

fn mtm() -> Result<MainThreadMarker, PlatformError> {
    MainThreadMarker::new()
        .ok_or_else(|| PlatformError("menus must be set on the main thread".into()))
}

fn target(mtm: MainThreadMarker) -> Retained<Target> {
    KEEP.with(|k| {
        if let Some((t, _)) = &*k.borrow() {
            return t.clone();
        }
        // SAFETY: `Target::alloc` returns a fresh allocation, and `init` is the NSObject initialiser.
        let t: Retained<Target> = unsafe { msg_send![Target::alloc(mtm), init] };
        *k.borrow_mut() = Some((t.clone(), None));
        t
    })
}

fn build(
    mtm: MainThreadMarker,
    title: &str,
    items: &[MenuItem],
    tgt: &Target,
    ids: &mut Vec<String>,
) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    menu.setAutoenablesItems(false);
    for item in items {
        match item {
            MenuItem::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            MenuItem::Submenu(sub) => {
                let mi = NSMenuItem::new(mtm);
                mi.setTitle(&NSString::from_str(&sub.title));
                mi.setSubmenu(Some(&build(mtm, &sub.title, &sub.items, tgt, ids)));
                menu.addItem(&mi);
            }
            MenuItem::Action {
                id,
                label,
                shortcut,
                enabled,
            } => {
                ids.push(id.clone());
                let key = shortcut.as_deref().map(key_equivalent).unwrap_or_default();
                // SAFETY: `menuAction:` is defined on Target above and takes one object argument.
                let mi = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        &NSString::from_str(label),
                        Some(sel!(menuAction:)),
                        &NSString::from_str(&key),
                    )
                };
                mi.setTag(ids.len() as isize);
                mi.setEnabled(*enabled);
                // SAFETY: `tgt` responds to `menuAction:` and is kept alive by `KEEP`.
                unsafe { mi.setTarget(Some(tgt)) };
                menu.addItem(&mi);
            }
        }
    }
    menu
}

/// The key character of a shortcut like `Mod+Shift+S` (modifiers use AppKit
/// defaults: Cmd). Only the final key is honoured; Cmd is implied.
pub(crate) fn key_equivalent(shortcut: &str) -> String {
    shortcut.rsplit('+').next().unwrap_or("").to_lowercase()
}

pub fn set_menu(_w: &winit::window::Window, menus: &[Menu]) -> Result<(), PlatformError> {
    let mtm = mtm()?;
    let tgt = target(mtm);
    let mut ids = Vec::new();
    let bar = NSMenu::new(mtm);
    for m in menus {
        let holder = NSMenuItem::new(mtm);
        holder.setSubmenu(Some(&build(mtm, &m.title, &m.items, &tgt, &mut ids)));
        bar.addItem(&holder);
    }
    *IDS.lock()
        .map_err(|_| PlatformError("state poisoned".into()))? = ids;
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&bar));
    Ok(())
}

pub fn set_tray(_w: &winit::window::Window, t: &TraySettings) -> Result<(), PlatformError> {
    if !t.icon_valid() || t.icon_width == 0 {
        return Err(PlatformError(
            "tray icon buffer does not match its size".into(),
        ));
    }
    let mtm = mtm()?;
    let tgt = target(mtm);
    clear_tray();
    let png = crate::tray::encode_png(&t.icon_rgba, t.icon_width, t.icon_height);
    let data = NSData::with_bytes(&png);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| PlatformError("could not decode tray icon".into()))?;
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(-1.0);
    let button = item
        .button(mtm)
        .ok_or_else(|| PlatformError("status item has no button".into()))?;
    button.setImage(Some(&image));
    button.setToolTip(Some(&NSString::from_str(&t.tooltip)));
    if let Some(menu) = &t.menu {
        let mut ids = IDS
            .lock()
            .map_err(|_| PlatformError("state poisoned".into()))?;
        item.setMenu(Some(&build(mtm, &menu.title, &menu.items, &tgt, &mut ids)));
    } else {
        // SAFETY: `trayClicked:` is defined on Target; button retains target weakly, KEEP holds it.
        unsafe {
            button.setTarget(Some(&tgt));
            button.setAction(Some(sel!(trayClicked:)));
        }
    }
    KEEP.with(|k| {
        if let Some((_, slot)) = &mut *k.borrow_mut() {
            *slot = Some(item);
        }
    });
    Ok(())
}

pub fn clear_tray() {
    KEEP.with(|k| {
        if let Some((_, slot)) = &mut *k.borrow_mut() {
            if let (Some(item), Some(_)) = (slot.take(), MainThreadMarker::new()) {
                NSStatusBar::systemStatusBar().removeStatusItem(&item);
            }
        }
    });
}

/// Connect `window` to NSAccessibility (see `macos_access`).
pub fn new_access_adapter(
    window: &std::sync::Arc<winit::window::Window>,
    _key: crate::shell::WindowKey,
    sink: crate::access::ActionSink,
) -> Option<Box<dyn crate::access::AccessAdapter>> {
    let a = super::macos_access::MacAdapter::new(window, sink)?;
    Some(Box::new(a))
}

// ---------------------------------------------------------------------------------------
// Global hotkeys: Carbon's RegisterEventHotKey. It needs no accessibility permission and its
// events arrive through the application's normal event loop, which winit runs.

mod carbon {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct EventTypeSpec {
        pub event_class: u32,
        pub event_kind: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct EventHotKeyId {
        pub signature: u32,
        pub id: u32,
    }

    pub type Handler = extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        pub fn GetApplicationEventTarget() -> *mut c_void;
        pub fn InstallEventHandler(
            target: *mut c_void,
            handler: Handler,
            count: u32,
            list: *const EventTypeSpec,
            user_data: *mut c_void,
            out: *mut *mut c_void,
        ) -> i32;
        pub fn RegisterEventHotKey(
            code: u32,
            modifiers: u32,
            id: EventHotKeyId,
            target: *mut c_void,
            options: u32,
            out: *mut *mut c_void,
        ) -> i32;
        pub fn UnregisterEventHotKey(hotkey: *mut c_void) -> i32;
        pub fn GetEventParameter(
            event: *mut c_void,
            name: u32,
            wanted: u32,
            actual: *mut u32,
            size: usize,
            out_size: *mut usize,
            data: *mut c_void,
        ) -> i32;
    }
}

/// Registered hotkeys: the string to report and the Carbon reference (as an address).
static HOTKEYS: Mutex<Vec<(String, usize)>> = Mutex::new(Vec::new());
static HANDLER: std::sync::Once = std::sync::Once::new();
const SIGNATURE: u32 = u32::from_be_bytes(*b"ally");

extern "C" fn on_hotkey(
    _next: *mut std::ffi::c_void,
    event: *mut std::ffi::c_void,
    _user: *mut std::ffi::c_void,
) -> i32 {
    let mut id = carbon::EventHotKeyId {
        signature: 0,
        id: 0,
    };
    // SAFETY: `event` is the live EventRef Carbon passed to this handler; the out pointer
    // is a properly sized, writable EventHotKeyID ('----' direct object, type 'hkid').
    let status = unsafe {
        carbon::GetEventParameter(
            event,
            u32::from_be_bytes(*b"----"),
            u32::from_be_bytes(*b"hkid"),
            std::ptr::null_mut(),
            std::mem::size_of::<carbon::EventHotKeyId>(),
            std::ptr::null_mut(),
            (&mut id as *mut carbon::EventHotKeyId).cast(),
        )
    };
    if status == 0 && id.signature == SIGNATURE {
        if let Ok(h) = HOTKEYS.lock() {
            if let Some((name, _)) = h.get((id.id as usize).wrapping_sub(1)) {
                super::push_pending(PlatformEvent::GlobalHotkey(name.clone()));
            }
        }
    }
    0
}

pub fn register_hotkey(keys: &str, sc: &Shortcut) -> Result<(), PlatformError> {
    let fail = |m: &str| PlatformError(format!("global hotkey {keys}: {m}"));
    let code = crate::hotkey::macos_keycode(&sc.key)
        .ok_or_else(|| fail(&format!("unknown key {:?}", sc.key)))?;
    let mut mods = 0u32;
    if sc.mods.logo {
        mods |= 0x100; // cmdKey
    }
    if sc.mods.shift {
        mods |= 0x200; // shiftKey
    }
    if sc.mods.alt {
        mods |= 0x800; // optionKey
    }
    if sc.mods.ctrl {
        mods |= 0x1000; // controlKey
    }
    let mut list = HOTKEYS.lock().map_err(|_| fail("state poisoned"))?;
    HANDLER.call_once(|| {
        let spec = carbon::EventTypeSpec {
            event_class: u32::from_be_bytes(*b"keyb"),
            event_kind: 5, // kEventHotKeyPressed
        };
        let mut out = std::ptr::null_mut();
        // SAFETY: the application event target is valid for the whole run; `spec` is read
        // during the call only; the handler is a plain function that never unwinds.
        unsafe {
            carbon::InstallEventHandler(
                carbon::GetApplicationEventTarget(),
                on_hotkey,
                1,
                &spec,
                std::ptr::null_mut(),
                &mut out,
            );
        }
    });
    let mut reference = std::ptr::null_mut();
    let id = carbon::EventHotKeyId {
        signature: SIGNATURE,
        id: list.len() as u32 + 1,
    };
    // SAFETY: plain call into Carbon with valid arguments; `reference` receives the handle.
    let status = unsafe {
        carbon::RegisterEventHotKey(
            code,
            mods,
            id,
            carbon::GetApplicationEventTarget(),
            0,
            &mut reference,
        )
    };
    if status != 0 || reference.is_null() {
        return Err(fail("already taken by another program"));
    }
    list.push((keys.to_string(), reference as usize));
    Ok(())
}

pub fn unregister_hotkeys() {
    let Ok(mut list) = HOTKEYS.lock() else { return };
    for (_, r) in list.drain(..) {
        // SAFETY: `r` came from RegisterEventHotKey and is released exactly once here.
        unsafe { carbon::UnregisterEventHotKey(r as *mut std::ffi::c_void) };
    }
}
