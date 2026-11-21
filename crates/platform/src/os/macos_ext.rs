//! macOS extras: system settings, window effects, rich clipboard, URL
//! handling. Compiled by CI on macOS; never run on a Mac by the author.

use super::capture;
use crate::settings::{self, SystemSettings};

/// Dark mode, accent and reduced motion from the `defaults` database.
pub fn read_settings() -> SystemSettings {
    SystemSettings {
        dark: super::macos::is_dark().unwrap_or(false),
        reduced_motion: settings::parse_macos_flag(
            capture(
                "defaults",
                &["read", "com.apple.universalaccess", "reduceMotion"],
            )
            .as_deref(),
        ),
        high_contrast: settings::parse_macos_flag(
            capture(
                "defaults",
                &["read", "com.apple.universalaccess", "increaseContrast"],
            )
            .as_deref(),
        ),
        accent: settings::parse_macos_accent(
            capture("defaults", &["read", "-g", "AppleAccentColor"]).as_deref(),
        ),
    }
}

use crate::clipboard::{self, ClipContent};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeHTML, NSPasteboardTypePNG, NSPasteboardTypeString,
};
use objc2_foundation::{NSArray, NSData, NSString};

/// Pasteboard type name for a list of file paths (old but still honoured by Finder).
fn filenames_type() -> objc2::rc::Retained<NSString> {
    NSString::from_str("NSFilenamesPboardType")
}

/// Everything on the general pasteboard: text, HTML, a PNG image, copied files.
pub fn rich_read() -> Option<ClipContent> {
    let pb = NSPasteboard::generalPasteboard();
    // SAFETY: the type constants are valid AppKit exports; each call returns
    // an owned object or nil.
    let (text, html, png) = unsafe {
        (
            pb.stringForType(NSPasteboardTypeString)
                .map(|s| s.to_string()),
            pb.stringForType(NSPasteboardTypeHTML)
                .map(|s| s.to_string()),
            pb.dataForType(NSPasteboardTypePNG).map(|d| d.to_vec()),
        )
    };
    let files = pb
        .propertyListForType(&filenames_type())
        .and_then(|l| l.downcast::<NSArray>().ok())
        .map(|a| {
            a.iter()
                .filter_map(|o| o.downcast::<NSString>().ok())
                .map(|s| std::path::PathBuf::from(s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(ClipContent {
        text,
        html,
        image: png.and_then(|b| clipboard::decode_png(&b)),
        files,
    })
}

/// Replace the general pasteboard with every format of `c`.
pub fn rich_write(c: &ClipContent) -> bool {
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    let mut ok = true;
    // SAFETY: the type constants are valid AppKit exports; the arguments are
    // live Foundation objects for the duration of each call.
    unsafe {
        if let Some(t) = &c.text {
            ok &= pb.setString_forType(&NSString::from_str(t), NSPasteboardTypeString);
        }
        if let Some(h) = &c.html {
            ok &= pb.setString_forType(&NSString::from_str(h), NSPasteboardTypeHTML);
        }
        if let Some(png) = c.image.as_ref().and_then(clipboard::encode_png) {
            ok &= pb.setData_forType(Some(&NSData::with_bytes(&png)), NSPasteboardTypePNG);
        }
        if !c.files.is_empty() {
            let paths: Vec<_> = c
                .files
                .iter()
                .map(|p| NSString::from_str(&p.to_string_lossy()))
                .collect();
            let arr = NSArray::from_retained_slice(&paths);
            ok &= pb.setPropertyList_forType(&arr, &filenames_type());
        }
    }
    ok
}

use crate::EffectOutcome;
use alloy_ui_core::WindowEffect;
use objc2::{rc::Retained, MainThreadMarker};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;

thread_local! {
    /// The vibrancy view we added to the window frame, so it can be replaced.
    static FX: RefCell<Option<Retained<NSVisualEffectView>>> = const { RefCell::new(None) };
}

/// Vibrancy through an `NSVisualEffectView` placed *behind* the window's
/// content view (a sibling inside the frame view), so the Metal layer keeps
/// drawing on top of it. `Transparent` only clears the window background.
pub fn set_effect(window: &winit::window::Window, effect: WindowEffect) -> EffectOutcome {
    let Some(mtm) = MainThreadMarker::new() else {
        return EffectOutcome::Unsupported;
    };
    let Ok(RawWindowHandle::AppKit(h)) = window.window_handle().map(|h| h.as_raw()) else {
        return EffectOutcome::Unsupported;
    };
    // SAFETY: winit hands out the window's live NSView pointer, and we are on
    // the main thread (checked above), where AppKit objects may be used.
    let view: &NSView = unsafe { h.ns_view.cast().as_ref() };
    let Some(win) = view.window() else {
        return EffectOutcome::Unsupported;
    };
    FX.with(|fx| {
        if let Some(old) = fx.borrow_mut().take() {
            old.removeFromSuperview();
        }
    });
    if effect == WindowEffect::None {
        win.setOpaque(true);
        win.setBackgroundColor(None);
        return EffectOutcome::Applied;
    }
    win.setOpaque(false);
    win.setBackgroundColor(Some(&NSColor::clearColor()));
    if effect == WindowEffect::Transparent {
        return EffectOutcome::Applied;
    }
    let Some(frame_view) = (unsafe { view.superview() }) else {
        return EffectOutcome::SeeThrough;
    };
    let fx = NSVisualEffectView::new(mtm);
    fx.setMaterial(match effect {
        WindowEffect::Mica => NSVisualEffectMaterial::WindowBackground,
        WindowEffect::Acrylic => NSVisualEffectMaterial::HUDWindow,
        _ => NSVisualEffectMaterial::UnderWindowBackground,
    });
    fx.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    fx.setState(NSVisualEffectState::Active);
    fx.setFrame(view.frame());
    fx.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    frame_view.addSubview_positioned_relativeTo(&fx, NSWindowOrderingMode::Below, Some(view));
    FX.with(|slot| *slot.borrow_mut() = Some(fx));
    EffectOutcome::Applied
}

use crate::PlatformEvent;
use objc2::{define_class, sel, MainThreadOnly};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSObject, NSObjectProtocol};

/// `'----'`, the direct-object parameter of an Apple Event.
const KEY_DIRECT_OBJECT: u32 = 0x2D2D_2D2D;

/// The `'----'` parameter of an Apple Event.
fn direct_object(event: &NSAppleEventDescriptor) -> Option<Retained<NSAppleEventDescriptor>> {
    // SAFETY: `paramDescriptorForKeyword:` takes an AEKeyword (a u32) and
    // returns an autoreleased descriptor or nil.
    unsafe { objc2::msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT] }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the class only reads
    // Apple Event parameters and pushes an event, and is main-thread only.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AlloyOpenHandler"]
    struct OpenHandler;

    impl OpenHandler {
        /// `kInternetEventClass` / `kAEGetURL`: a `scheme://` link was opened.
        #[unsafe(method(handleURL:withReply:))]
        fn handle_url(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            let url = direct_object(event).and_then(|d| d.stringValue());
            log::debug!("open-URL Apple Event: {url:?}");
            if let Some(u) = url {
                super::push_pending(PlatformEvent::Open(vec![u.to_string()]));
            }
        }

        /// `kCoreEventClass` / `kAEOpenDocuments`: files were opened with the app.
        #[unsafe(method(handleOpenDocuments:withReply:))]
        fn handle_docs(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            log::debug!("open-documents Apple Event");
            let Some(list) = direct_object(event) else {
                return;
            };
            let files: Vec<String> = (1..=list.numberOfItems())
                .filter_map(|i| list.descriptorAtIndex(i))
                .filter_map(|d| d.fileURLValue())
                .filter_map(|u| u.path())
                .map(|p| p.to_string())
                .collect();
            if !files.is_empty() {
                super::push_pending(PlatformEvent::Open(files));
            }
        }
    }

    // SAFETY: `OpenHandler` is an `NSObject` subclass with no extra invariants.
    unsafe impl NSObjectProtocol for OpenHandler {}
);

thread_local! {
    static HANDLER: RefCell<Option<Retained<OpenHandler>>> = const { RefCell::new(None) };
}

/// Route "open this link" and "open these files" Apple Events to the shell as
/// [`PlatformEvent::Open`]. Main thread only; the shell calls it once, when
/// the app has launched.
pub fn install_open_handler() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    // SAFETY: `alloc` returns a fresh allocation and `init` is the NSObject initialiser.
    let handler: Retained<OpenHandler> = unsafe { objc2::msg_send![OpenHandler::alloc(mtm), init] };
    let mgr = NSAppleEventManager::sharedAppleEventManager();
    let register = |sel: objc2::runtime::Sel, class: u32, id: u32| {
        // SAFETY: `setEventHandler:andSelector:forEventClass:andEventID:` takes an
        // object, a selector it implements (defined on `OpenHandler` above) and two
        // four-character codes. The handler is kept alive in `HANDLER` (the manager
        // does not retain it).
        unsafe {
            let _: () = objc2::msg_send![
                &*mgr,
                setEventHandler: &*handler,
                andSelector: sel,
                forEventClass: class,
                andEventID: id
            ];
        }
    };
    register(sel!(handleURL:withReply:), 0x4755_524C, 0x4755_524C); // 'GURL', 'GURL'
    register(
        sel!(handleOpenDocuments:withReply:),
        0x6165_7674,
        0x6F64_6F63,
    ); // 'aevt', 'odoc'
    HANDLER.with(|h| *h.borrow_mut() = Some(handler));
}

/// A URL scheme cannot be registered while running on macOS; it must be in
/// the bundle's `Info.plist`.
pub fn register_scheme(
    _app_id: &str,
    _scheme: &str,
    _exe: &str,
) -> Result<(), crate::PlatformError> {
    Err(crate::PlatformError(
        "macOS reads URL schemes from the app bundle: list them under `url-schemes` in \
         [package.metadata.alloy] and build with `alloy bundle app`"
            .into(),
    ))
}
