//! macOS accessibility: NSAccessibility through `objc2`.
//!
//! A transparent, mouse-transparent `NSView` ([`AccessContainer`]) is added on
//! top of winit's content view. It is an accessibility group named after the
//! window and its `accessibilityChildren` are one [`AccessElement`] per
//! node (a subclass of `NSAccessibilityElement` that answers role, label,
//! value, frame and enabled/focused state, and performs press, increment,
//! decrement, focus and set-value). Each frame the elements are updated in
//! place and VoiceOver is told what changed with
//! `NSAccessibilityPostNotification`.
//!
//! Compiled for `aarch64-apple-darwin` by CI; not run on a Mac by the author.

use crate::access::{
    AccessAction, AccessAdapter, AccessId, AccessNode, AccessRequest, AccessRole, AccessTree,
    ActionSink,
};
use objc2::Message;
use objc2::{
    define_class, msg_send,
    rc::{Retained, Weak},
    runtime::AnyObject,
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{NSAccessibilityElement, NSAutoresizingMaskOptions, NSView};
use objc2_foundation::{NSArray, NSNumber, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};
use std::cell::RefCell;

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    fn NSAccessibilityPostNotification(element: &AnyObject, notification: &NSString);
}

fn post(element: &AnyObject, name: &str) {
    // SAFETY: `element` is a live NSObject that adopts the accessibility
    // protocol (our element or container) and `name` is an NSString.
    unsafe { NSAccessibilityPostNotification(element, &NSString::from_str(name)) }
}

/// AppKit role string for a node role (`NSAccessibilityRole` values).
fn ax_role(r: AccessRole) -> &'static str {
    match r {
        AccessRole::Label => "AXStaticText",
        AccessRole::Button => "AXButton",
        AccessRole::Checkbox | AccessRole::Switch => "AXCheckBox",
        AccessRole::Radio | AccessRole::Tab => "AXRadioButton",
        AccessRole::Slider => "AXSlider",
        AccessRole::SpinButton => "AXIncrementor",
        AccessRole::TextInput | AccessRole::DatePicker => "AXTextField",
        AccessRole::TextEditor => "AXTextArea",
        AccessRole::ComboBox => "AXComboBox",
        AccessRole::ProgressBar => "AXProgressIndicator",
        AccessRole::Group => "AXGroup",
    }
}

fn number(n: &AccessNode) -> f64 {
    n.value
        .as_deref()
        .and_then(|v| v.trim().trim_end_matches('%').parse().ok())
        .unwrap_or(0.0)
}

// ----- element ---------------------------------------------------------------

struct ElementIvars {
    node: RefCell<AccessNode>,
    sink: ActionSink,
    parent: Weak<AccessContainer>,
}

define_class!(
    // SAFETY: NSAccessibilityElement may be subclassed (Apple documents it for
    // custom elements). The overrides only read `ivars` and forward requests to
    // a thread-safe sink; all calls come from AppKit on the main thread.
    #[unsafe(super(NSAccessibilityElement))]
    #[thread_kind = MainThreadOnly]
    #[name = "AlloyAccessElement"]
    #[ivars = ElementIvars]
    struct AccessElement;

    impl AccessElement {
        #[unsafe(method(isAccessibilityElement))]
        fn is_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn role(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(ax_role(self.ivars().node.borrow().role)))
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn label(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(&self.ivars().node.borrow().label))
        }

        #[unsafe(method_id(accessibilityValue))]
        fn value(&self) -> Option<Retained<AnyObject>> {
            let n = self.ivars().node.borrow();
            // SAFETY: NSNumber and NSString are NSObjects; `AnyObject` is the
            // return type AppKit expects for an `id` value.
            unsafe {
                match (n.checked, n.role) {
                    (Some(c), _) => Some(Retained::cast_unchecked(NSNumber::new_i32(c as i32))),
                    (
                        None,
                        AccessRole::Slider | AccessRole::SpinButton | AccessRole::ProgressBar,
                    ) => Some(Retained::cast_unchecked(NSNumber::new_f64(number(&n)))),
                    (None, AccessRole::Label | AccessRole::Button | AccessRole::Tab) => None,
                    (None, _) => n
                        .value
                        .as_deref()
                        .map(|v| Retained::cast_unchecked(NSString::from_str(v))),
                }
            }
        }

        #[unsafe(method(setAccessibilityValue:))]
        fn set_value(&self, v: &AnyObject) {
            let n = self.ivars().node.borrow();
            let id = n.id;
            match n.role {
                AccessRole::Slider | AccessRole::SpinButton => {
                    // SAFETY: AppKit passes the NSNumber a client set; `doubleValue`
                    // is only sent after checking the class.
                    let target: Option<f64> = unsafe {
                        let is_num: bool = msg_send![v, isKindOfClass: objc2::class!(NSNumber)];
                        is_num.then(|| msg_send![v, doubleValue])
                    };
                    if let Some(t) = target {
                        let cur = number(&n);
                        if t > cur {
                            self.send(id, AccessAction::Increment);
                        } else if t < cur {
                            self.send(id, AccessAction::Decrement);
                        }
                    }
                }
                AccessRole::TextInput | AccessRole::TextEditor | AccessRole::DatePicker => {
                    // SAFETY: only sent after checking the class.
                    let s: Option<String> = unsafe {
                        let is_str: bool = msg_send![v, isKindOfClass: objc2::class!(NSString)];
                        is_str.then(|| {
                            let s: &NSString = &*(v as *const AnyObject as *const NSString);
                            s.to_string()
                        })
                    };
                    if let Some(s) = s {
                        self.send(id, AccessAction::SetValue(s));
                    }
                }
                _ => {}
            }
        }

        #[unsafe(method_id(accessibilityMinValue))]
        fn min_value(&self) -> Option<Retained<AnyObject>> {
            self.range_value(|r| r.0)
        }

        #[unsafe(method_id(accessibilityMaxValue))]
        fn max_value(&self) -> Option<Retained<AnyObject>> {
            self.range_value(|r| r.1)
        }

        #[unsafe(method(accessibilityFrame))]
        fn frame(&self) -> NSRect {
            let b = self.ivars().node.borrow().bounds;
            let local = NSRect::new(
                NSPoint::new(b.origin.x as f64, b.origin.y as f64),
                NSSize::new(b.size.width as f64, b.size.height as f64),
            );
            let Some(c) = self.ivars().parent.load() else {
                return local;
            };
            let in_window = c.convertRect_toView(local, None);
            match c.window() {
                Some(w) => w.convertRectToScreen(in_window),
                None => in_window,
            }
        }

        #[unsafe(method_id(accessibilityParent))]
        fn parent(&self) -> Option<Retained<AnyObject>> {
            // SAFETY: the container is an NSView, an NSObject.
            self.ivars()
                .parent
                .load()
                .map(|c| unsafe { Retained::cast_unchecked(c) })
        }

        #[unsafe(method(isAccessibilityEnabled))]
        fn enabled(&self) -> bool {
            self.ivars().node.borrow().enabled
        }

        #[unsafe(method(isAccessibilityFocused))]
        fn focused(&self) -> bool {
            self.ivars().node.borrow().focused
        }

        #[unsafe(method(setAccessibilityFocused:))]
        fn set_focused(&self, on: bool) {
            if on {
                let id = self.ivars().node.borrow().id;
                self.send(id, AccessAction::Focus);
            }
        }

        #[unsafe(method(accessibilityPerformPress))]
        fn perform_press(&self) -> bool {
            let n = self.ivars().node.borrow();
            let id = n.id;
            let ok = n.enabled;
            drop(n);
            if ok {
                self.send(id, AccessAction::Press);
            }
            ok
        }

        #[unsafe(method(accessibilityPerformIncrement))]
        fn perform_increment(&self) -> bool {
            let id = self.ivars().node.borrow().id;
            self.send(id, AccessAction::Increment);
            true
        }

        #[unsafe(method(accessibilityPerformDecrement))]
        fn perform_decrement(&self) -> bool {
            let id = self.ivars().node.borrow().id;
            self.send(id, AccessAction::Decrement);
            true
        }

        #[unsafe(method_id(accessibilityWindow))]
        fn window(&self) -> Option<Retained<AnyObject>> {
            // SAFETY: NSWindow is an NSObject.
            self.ivars()
                .parent
                .load()
                .and_then(|c| c.window())
                .map(|w| unsafe { Retained::cast_unchecked(w) })
        }

        #[unsafe(method_id(accessibilityTopLevelUIElement))]
        fn top_level(&self) -> Option<Retained<AnyObject>> {
            // SAFETY: NSWindow is an NSObject.
            self.ivars()
                .parent
                .load()
                .and_then(|c| c.window())
                .map(|w| unsafe { Retained::cast_unchecked(w) })
        }
    }

    unsafe impl NSObjectProtocol for AccessElement {}
);

impl AccessElement {
    fn new(
        mtm: MainThreadMarker,
        node: AccessNode,
        sink: ActionSink,
        parent: &AccessContainer,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ElementIvars {
            node: RefCell::new(node),
            sink,
            parent: Weak::from_retained(&parent.retain()),
        });
        // SAFETY: plain `init` of the freshly allocated superclass instance.
        unsafe { msg_send![super(this), init] }
    }

    fn send(&self, target: AccessId, action: AccessAction) {
        (self.ivars().sink)(AccessRequest { target, action });
    }

    fn range_value(&self, pick: impl Fn((f64, f64)) -> f64) -> Option<Retained<AnyObject>> {
        let n = self.ivars().node.borrow();
        if !matches!(
            n.role,
            AccessRole::Slider | AccessRole::SpinButton | AccessRole::ProgressBar
        ) {
            return None;
        }
        let (lo, hi) = n.range.map_or((0.0, 100.0), |r| (r.min, r.max));
        // SAFETY: NSNumber is an NSObject.
        Some(unsafe { Retained::cast_unchecked(NSNumber::new_f64(pick((lo, hi)))) })
    }
}

// ----- container -------------------------------------------------------------

struct ContainerIvars {
    title: RefCell<String>,
    children: RefCell<Vec<Retained<AccessElement>>>,
}

define_class!(
    // SAFETY: NSView may be subclassed. The overrides only read `ivars`; the
    // view ignores mouse hits so it never changes how the app behaves.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "AlloyAccessContainer"]
    #[ivars = ContainerIvars]
    struct AccessContainer;

    impl AccessContainer {
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, _p: NSPoint) -> Option<Retained<NSView>> {
            None
        }

        // Node bounds are top-left based, like the app's coordinates.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(isAccessibilityElement))]
        fn is_element(&self) -> bool {
            true
        }

        #[unsafe(method_id(accessibilityRole))]
        fn role(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str("AXGroup"))
        }

        #[unsafe(method_id(accessibilityLabel))]
        fn label(&self) -> Option<Retained<NSString>> {
            Some(NSString::from_str(&self.ivars().title.borrow()))
        }

        #[unsafe(method_id(accessibilityChildren))]
        fn children(&self) -> Option<Retained<NSArray<AccessElement>>> {
            Some(NSArray::from_retained_slice(&self.ivars().children.borrow()))
        }

        #[unsafe(method_id(accessibilityFocusedUIElement))]
        fn focused_element(&self) -> Option<Retained<AnyObject>> {
            let kids = self.ivars().children.borrow();
            // SAFETY: AccessElement is an NSObject.
            kids.iter()
                .find(|e| e.ivars().node.borrow().focused)
                .map(|hit| unsafe { Retained::cast_unchecked(hit.clone()) })
        }
    }

    unsafe impl NSObjectProtocol for AccessContainer {}
);

impl AccessContainer {
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ContainerIvars {
            title: RefCell::new(String::new()),
            children: RefCell::new(Vec::new()),
        });
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

// ----- adapter -----------------------------------------------------------------

/// One window's NSAccessibility tree.
pub struct MacAdapter {
    mtm: MainThreadMarker,
    container: Retained<AccessContainer>,
    sink: ActionSink,
    last: AccessTree,
}

impl MacAdapter {
    /// Attach an accessibility container to the window's content view.
    /// Must run on the main thread. Returns `None` for a non-AppKit handle.
    pub fn new(window: &winit::window::Window, sink: ActionSink) -> Option<Self> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let mtm = MainThreadMarker::new()?;
        let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: winit guarantees `ns_view` is the window's live content view
        // for as long as the window exists, and we are on the main thread.
        let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
        let container = AccessContainer::new(mtm, view.bounds());
        container.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        view.addSubview(&container);
        Some(Self {
            mtm,
            container,
            sink,
            last: AccessTree::default(),
        })
    }
}

impl Drop for MacAdapter {
    fn drop(&mut self) {
        self.container.removeFromSuperview();
    }
}

impl AccessAdapter for MacAdapter {
    fn update(&mut self, tree: &AccessTree) {
        let diff = self.last.diff(tree);
        *self.container.ivars().title.borrow_mut() = tree.title.clone();
        let old: Vec<Retained<AccessElement>> = self.container.ivars().children.take();
        let mut next = Vec::with_capacity(tree.nodes.len());
        for n in &tree.nodes {
            let reuse = old.iter().find(|e| e.ivars().node.borrow().id == n.id);
            let el = match reuse {
                Some(e) => {
                    *e.ivars().node.borrow_mut() = n.clone();
                    e.clone()
                }
                None => AccessElement::new(self.mtm, n.clone(), self.sink.clone(), &self.container),
            };
            next.push(el);
        }
        *self.container.ivars().children.borrow_mut() = next.clone();

        // Tell VoiceOver what changed.
        let as_obj = |e: &Retained<AccessElement>| -> Retained<AnyObject> {
            // SAFETY: AccessElement is an NSObject.
            unsafe { Retained::cast_unchecked(e.clone()) }
        };
        let container: Retained<AnyObject> =
            unsafe { Retained::cast_unchecked(self.container.clone()) };
        if !diff.added.is_empty() || !diff.removed.is_empty() || diff.reordered {
            post(&container, "AXLayoutChanged");
        }
        for id in &diff.changed {
            let (Some(o), Some(n)) = (self.last.get(*id), tree.get(*id)) else {
                continue;
            };
            let Some(el) = next.iter().find(|e| e.ivars().node.borrow().id == *id) else {
                continue;
            };
            let obj = as_obj(el);
            if o.label != n.label {
                post(&obj, "AXTitleChanged");
            }
            if o.value != n.value || o.checked != n.checked {
                post(&obj, "AXValueChanged");
            }
            if !o.focused && n.focused {
                post(&obj, "AXFocusedUIElementChanged");
            }
        }
        self.last = tree.clone();
    }
}
