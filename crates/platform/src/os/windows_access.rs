//! Windows accessibility: UI Automation providers, through the `windows` crate
//! (needed for the COM interfaces; see DECISIONS.md 13).
//!
//! The window's HWND is subclassed (`SetWindowSubclass`) to answer
//! `WM_GETOBJECT` with a root fragment provider. Its children are one
//! provider per node, created on demand from the latest [`AccessTree`] and
//! identified by runtime id, so navigation never holds stale objects. Pattern
//! providers (Invoke, Toggle, Value, RangeValue) forward requests to the
//! action sink. UI Automation calls providers on its own threads, so all state
//! is behind a mutex and the sink is thread-safe.
//!
//! Type-checked from Linux with `--target x86_64-pc-windows-gnu`; not run on
//! Windows by the author.

// The Windows API names its constants in mixed case (`UIA_NamePropertyId`).
#![allow(non_upper_case_globals)]

use crate::access::{
    AccessAction, AccessAdapter, AccessId, AccessNode, AccessRequest, AccessRole, AccessTree,
    ActionSink,
};
use std::sync::{Arc, Mutex};
use windows::{
    core::{implement, IUnknown, Interface, Result as WinResult, BSTR, HRESULT, VARIANT},
    Win32::{
        Foundation::{BOOL, E_FAIL, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::ClientToScreen,
        System::{
            Com::{SAFEARRAY, SAFEARRAYBOUND},
            Ole::{SafeArrayCreate, SafeArrayPutElement},
            Variant::VT_I4,
        },
        UI::{
            Accessibility::*,
            HiDpi::GetDpiForWindow,
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{WM_GETOBJECT, WM_NCDESTROY},
        },
    },
};

const SUBCLASS_ID: usize = 0xA110_7001;

struct Shared {
    hwnd: isize,
    tree: AccessTree,
    sink: ActionSink,
}

type State = Arc<Mutex<Shared>>;

fn lock(s: &State) -> std::sync::MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(|p| p.into_inner())
}

fn scale(hwnd: isize) -> f64 {
    // SAFETY: GetDpiForWindow accepts any HWND value and returns 0 if invalid.
    let dpi = unsafe { GetDpiForWindow(HWND(hwnd as *mut _)) };
    if dpi == 0 {
        1.0
    } else {
        dpi as f64 / 96.0
    }
}

fn control_type(r: AccessRole) -> UIA_CONTROLTYPE_ID {
    match r {
        AccessRole::Label => UIA_TextControlTypeId,
        AccessRole::Button => UIA_ButtonControlTypeId,
        AccessRole::Checkbox | AccessRole::Switch => UIA_CheckBoxControlTypeId,
        AccessRole::Radio => UIA_RadioButtonControlTypeId,
        AccessRole::Slider => UIA_SliderControlTypeId,
        AccessRole::SpinButton => UIA_SpinnerControlTypeId,
        AccessRole::TextInput | AccessRole::DatePicker => UIA_EditControlTypeId,
        AccessRole::TextEditor => UIA_DocumentControlTypeId,
        AccessRole::ComboBox => UIA_ComboBoxControlTypeId,
        AccessRole::Tab => UIA_TabItemControlTypeId,
        AccessRole::ProgressBar => UIA_ProgressBarControlTypeId,
        AccessRole::Group => UIA_GroupControlTypeId,
    }
}

fn number(n: &AccessNode) -> f64 {
    n.value
        .as_deref()
        .and_then(|v| v.trim().trim_end_matches('%').parse().ok())
        .unwrap_or(0.0)
}

fn range(n: &AccessNode) -> (f64, f64, f64) {
    n.range
        .map_or((0.0, 100.0, 1.0), |r| (r.min, r.max, r.step))
}

fn is_range(r: AccessRole) -> bool {
    matches!(
        r,
        AccessRole::Slider | AccessRole::SpinButton | AccessRole::ProgressBar
    )
}

fn is_text(r: AccessRole) -> bool {
    matches!(
        r,
        AccessRole::TextInput | AccessRole::TextEditor | AccessRole::DatePicker
    )
}

fn can_invoke(r: AccessRole) -> bool {
    matches!(
        r,
        AccessRole::Button | AccessRole::Radio | AccessRole::Tab | AccessRole::ComboBox
    )
}

fn can_toggle(r: AccessRole) -> bool {
    matches!(r, AccessRole::Checkbox | AccessRole::Switch)
}

fn variant_str(s: &str) -> VARIANT {
    VARIANT::from(BSTR::from(s))
}

fn safearray_i4(vals: &[i32]) -> WinResult<*mut SAFEARRAY> {
    let bound = SAFEARRAYBOUND {
        cElements: vals.len() as u32,
        lLbound: 0,
    };
    // SAFETY: the array is created with the bounds above and each element
    // index written is within them; `SafeArrayPutElement` copies the i32.
    unsafe {
        let sa = SafeArrayCreate(VT_I4, 1, &bound);
        if sa.is_null() {
            return Err(E_FAIL.into());
        }
        for (i, v) in vals.iter().enumerate() {
            let idx = i as i32;
            SafeArrayPutElement(sa, &idx, v as *const i32 as *const _)?;
        }
        Ok(sa)
    }
}

// ----- element provider -------------------------------------------------------------

/// `None` is the window root; `Some(id)` is a node.
#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct Element {
    state: State,
    id: Option<AccessId>,
}

impl Element {
    fn new(state: &State, id: Option<AccessId>) -> Self {
        Self {
            state: state.clone(),
            id,
        }
    }

    fn node(&self) -> Option<AccessNode> {
        let s = lock(&self.state);
        s.tree.get(self.id?).cloned()
    }

    fn fragment(&self, id: Option<AccessId>) -> IRawElementProviderFragment {
        Element::new(&self.state, id).into()
    }
}

#[allow(non_snake_case)]
impl IRawElementProviderSimple_Impl for Element_Impl {
    fn ProviderOptions(&self) -> WinResult<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, pattern: UIA_PATTERN_ID) -> WinResult<IUnknown> {
        let Some(n) = self.node() else {
            return Err(E_FAIL.into());
        };
        let id = n.id;
        let state = self.state.clone();
        let unk: IUnknown = if pattern == UIA_InvokePatternId && can_invoke(n.role) {
            IInvokeProvider::from(Pattern { state, id }).into()
        } else if pattern == UIA_TogglePatternId && can_toggle(n.role) {
            IToggleProvider::from(Pattern { state, id }).into()
        } else if pattern == UIA_ValuePatternId && is_text(n.role) {
            IValueProvider::from(Pattern { state, id }).into()
        } else if pattern == UIA_RangeValuePatternId && is_range(n.role) {
            IRangeValueProvider::from(Pattern { state, id }).into()
        } else {
            return Err(E_FAIL.into());
        };
        Ok(unk)
    }

    fn GetPropertyValue(&self, property: UIA_PROPERTY_ID) -> WinResult<VARIANT> {
        let s = lock(&self.state);
        let node = self.id.and_then(|i| s.tree.get(i));
        Ok(match property {
            UIA_NamePropertyId => variant_str(match node {
                Some(n) => &n.label,
                None => &s.tree.title,
            }),
            UIA_ControlTypePropertyId => VARIANT::from(
                node.map_or(UIA_WindowControlTypeId, |n| control_type(n.role))
                    .0,
            ),
            UIA_IsEnabledPropertyId => VARIANT::from(node.map_or(true, |n| n.enabled)),
            UIA_HasKeyboardFocusPropertyId => VARIANT::from(node.is_some_and(|n| n.focused)),
            UIA_IsKeyboardFocusablePropertyId => VARIANT::from(node.is_some_and(|n| {
                n.enabled && !matches!(n.role, AccessRole::Label | AccessRole::ProgressBar)
            })),
            UIA_IsControlElementPropertyId | UIA_IsContentElementPropertyId => VARIANT::from(true),
            UIA_AutomationIdPropertyId => variant_str(
                &self
                    .id
                    .map_or_else(|| "window".to_string(), |i| format!("n{}", i.0)),
            ),
            UIA_FrameworkIdPropertyId => variant_str("Alloy"),
            _ => VARIANT::default(), // empty: "not supported", UIA uses its default
        })
    }

    fn HostRawElementProvider(&self) -> WinResult<IRawElementProviderSimple> {
        if self.id.is_some() {
            return Err(E_FAIL.into());
        }
        let hwnd = lock(&self.state).hwnd;
        // SAFETY: the HWND is the live window this provider was created for.
        unsafe { UiaHostProviderFromHwnd(HWND(hwnd as *mut _)) }
    }
}

#[allow(non_snake_case)]
impl IRawElementProviderFragment_Impl for Element_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> WinResult<IRawElementProviderFragment> {
        let s = lock(&self.state);
        let nodes = &s.tree.nodes;
        let pick = |i: Option<usize>| -> WinResult<IRawElementProviderFragment> {
            i.and_then(|i| nodes.get(i))
                .map(|n| self.fragment(Some(n.id)))
                .ok_or_else(|| E_FAIL.into())
        };
        match (self.id, direction) {
            (None, NavigateDirection_FirstChild) => pick(Some(0)),
            (None, NavigateDirection_LastChild) => pick(nodes.len().checked_sub(1)),
            (Some(_), NavigateDirection_Parent) => Ok(self.fragment(None)),
            (Some(id), NavigateDirection_NextSibling) => {
                pick(nodes.iter().position(|n| n.id == id).map(|i| i + 1))
            }
            (Some(id), NavigateDirection_PreviousSibling) => pick(
                nodes
                    .iter()
                    .position(|n| n.id == id)
                    .and_then(|i| i.checked_sub(1)),
            ),
            _ => Err(E_FAIL.into()), // the root's parent is the HWND host
        }
    }

    fn GetRuntimeId(&self) -> WinResult<*mut SAFEARRAY> {
        match self.id {
            // The root is identified by its host HWND.
            None => Ok(std::ptr::null_mut()),
            Some(id) => {
                // Two u32 halves of the id, prefixed by UiaAppendRuntimeId.
                safearray_i4(&[
                    UiaAppendRuntimeId as i32,
                    id.0 as u32 as i32,
                    (id.0 >> 32) as u32 as i32,
                ])
            }
        }
    }

    fn BoundingRectangle(&self) -> WinResult<UiaRect> {
        let s = lock(&self.state);
        let mut origin = POINT { x: 0, y: 0 };
        // SAFETY: plain query of the client-area origin of our own window.
        unsafe {
            let _ = ClientToScreen(HWND(s.hwnd as *mut _), &mut origin);
        }
        let k = scale(s.hwnd);
        let b = match self.id.and_then(|i| s.tree.get(i)) {
            Some(n) => n.bounds,
            None => {
                let (mut w, mut h) = (0f32, 0f32);
                for n in &s.tree.nodes {
                    w = w.max(n.bounds.origin.x + n.bounds.size.width);
                    h = h.max(n.bounds.origin.y + n.bounds.size.height);
                }
                alloy_ui_core::Rectangle {
                    origin: alloy_ui_core::Point { x: 0.0, y: 0.0 },
                    size: alloy_ui_core::Size {
                        width: w,
                        height: h,
                    },
                }
            }
        };
        Ok(UiaRect {
            left: origin.x as f64 + b.origin.x as f64 * k,
            top: origin.y as f64 + b.origin.y as f64 * k,
            width: b.size.width as f64 * k,
            height: b.size.height as f64 * k,
        })
    }

    fn GetEmbeddedFragmentRoots(&self) -> WinResult<*mut SAFEARRAY> {
        Ok(std::ptr::null_mut())
    }

    fn SetFocus(&self) -> WinResult<()> {
        if let Some(id) = self.id {
            let s = lock(&self.state);
            (s.sink)(AccessRequest {
                target: id,
                action: AccessAction::Focus,
            });
        }
        Ok(())
    }

    fn FragmentRoot(&self) -> WinResult<IRawElementProviderFragmentRoot> {
        Ok(Element::new(&self.state, None).into())
    }
}

#[allow(non_snake_case)]
impl IRawElementProviderFragmentRoot_Impl for Element_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> WinResult<IRawElementProviderFragment> {
        let s = lock(&self.state);
        let mut origin = POINT { x: 0, y: 0 };
        // SAFETY: plain query of the client-area origin of our own window.
        unsafe {
            let _ = ClientToScreen(HWND(s.hwnd as *mut _), &mut origin);
        }
        let k = scale(s.hwnd);
        let (lx, ly) = (
            ((x - origin.x as f64) / k) as f32,
            ((y - origin.y as f64) / k) as f32,
        );
        // Later nodes are drawn on top.
        let hit = s.tree.nodes.iter().rev().find(|n| {
            let b = n.bounds;
            lx >= b.origin.x
                && ly >= b.origin.y
                && lx < b.origin.x + b.size.width
                && ly < b.origin.y + b.size.height
        });
        Ok(self.fragment(hit.map(|n| n.id)))
    }

    fn GetFocus(&self) -> WinResult<IRawElementProviderFragment> {
        let s = lock(&self.state);
        s.tree
            .focused()
            .map(|n| self.fragment(Some(n.id)))
            .ok_or_else(|| E_FAIL.into())
    }
}

// ----- patterns ---------------------------------------------------------------------

#[implement(IInvokeProvider, IToggleProvider, IValueProvider, IRangeValueProvider)]
struct Pattern {
    state: State,
    id: AccessId,
}

impl Pattern {
    fn node(&self) -> WinResult<AccessNode> {
        lock(&self.state)
            .tree
            .get(self.id)
            .cloned()
            .ok_or_else(|| E_FAIL.into())
    }
    fn send(&self, action: AccessAction) {
        let s = lock(&self.state);
        (s.sink)(AccessRequest {
            target: self.id,
            action,
        });
    }
}

#[allow(non_snake_case)]
impl IInvokeProvider_Impl for Pattern_Impl {
    fn Invoke(&self) -> WinResult<()> {
        if !self.node()?.enabled {
            return Err(HRESULT(0x8004_0200_u32 as i32).into()); // UIA_E_ELEMENTNOTENABLED
        }
        self.send(AccessAction::Press);
        Ok(())
    }
}

#[allow(non_snake_case)]
impl IToggleProvider_Impl for Pattern_Impl {
    fn Toggle(&self) -> WinResult<()> {
        self.send(AccessAction::Press);
        Ok(())
    }

    fn ToggleState(&self) -> WinResult<ToggleState> {
        Ok(if self.node()?.checked == Some(true) {
            ToggleState_On
        } else {
            ToggleState_Off
        })
    }
}

#[allow(non_snake_case)]
impl IValueProvider_Impl for Pattern_Impl {
    fn SetValue(&self, val: &windows::core::PCWSTR) -> WinResult<()> {
        // SAFETY: UI Automation passes a NUL-terminated UTF-16 string.
        let text = unsafe { val.to_string() }.map_err(|_| windows::core::Error::from(E_FAIL))?;
        self.send(AccessAction::SetValue(text));
        Ok(())
    }

    fn Value(&self) -> WinResult<BSTR> {
        Ok(BSTR::from(self.node()?.value.unwrap_or_default().as_str()))
    }

    fn IsReadOnly(&self) -> WinResult<BOOL> {
        Ok(BOOL::from(!self.node()?.enabled))
    }
}

#[allow(non_snake_case)]
impl IRangeValueProvider_Impl for Pattern_Impl {
    fn SetValue(&self, val: f64) -> WinResult<()> {
        let cur = number(&self.node()?);
        if val > cur {
            self.send(AccessAction::Increment);
        } else if val < cur {
            self.send(AccessAction::Decrement);
        }
        Ok(())
    }

    fn Value(&self) -> WinResult<f64> {
        Ok(number(&self.node()?))
    }

    fn IsReadOnly(&self) -> WinResult<BOOL> {
        Ok(BOOL::from(!self.node()?.enabled))
    }

    fn Maximum(&self) -> WinResult<f64> {
        Ok(range(&self.node()?).1)
    }

    fn Minimum(&self) -> WinResult<f64> {
        Ok(range(&self.node()?).0)
    }

    fn LargeChange(&self) -> WinResult<f64> {
        Ok(range(&self.node()?).2 * 10.0)
    }

    fn SmallChange(&self) -> WinResult<f64> {
        Ok(range(&self.node()?).2)
    }
}

// ----- window hook ----------------------------------------------------------------------

/// Answer `WM_GETOBJECT` for UI Automation.
///
/// # Safety
/// Only installed as a `SetWindowSubclass` callback, so Windows calls it on the window's
/// thread with a valid `hwnd` and message parameters.
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    refdata: usize,
) -> LRESULT {
    // SAFETY: `refdata` is the pointer to the `State` leaked by `WinAdapter::new`
    // and stays valid until WM_NCDESTROY or `Drop` removes this subclass.
    let state: &State = unsafe { &*(refdata as *const State) };
    match msg {
        WM_GETOBJECT if lparam.0 as i32 == UiaRootObjectId => {
            let root: IRawElementProviderSimple =
                IRawElementProviderFragment::from(Element::new(state, None))
                    .cast()
                    .expect("root implements IRawElementProviderSimple");
            // SAFETY: arguments come straight from the window procedure.
            unsafe { UiaReturnRawElementProvider(hwnd, wparam, lparam, &root) }
        }
        WM_NCDESTROY => {
            // SAFETY: standard teardown; the leaked state is freed by `Drop`.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
        }
        // SAFETY: forwarding the original message unchanged.
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

/// One window's UI Automation tree.
pub struct WinAdapter {
    state: State,
    /// The leaked `Box<State>` handed to the subclass procedure.
    refdata: *mut State,
    hwnd: isize,
    last: AccessTree,
}

// SAFETY: the raw pointer is only dereferenced by the subclass procedure on the
// window's thread and freed in `Drop`; the adapter itself is used from one thread.
unsafe impl Send for WinAdapter {}

impl WinAdapter {
    /// Subclass the window so UI Automation can find our provider.
    pub fn new(window: &winit::window::Window, sink: ActionSink) -> Option<Self> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let RawWindowHandle::Win32(h) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        let hwnd = h.hwnd.get();
        let state: State = Arc::new(Mutex::new(Shared {
            hwnd,
            tree: AccessTree::default(),
            sink,
        }));
        let refdata = Box::into_raw(Box::new(state.clone()));
        // SAFETY: `refdata` outlives the subclass (freed in Drop after removal).
        let ok = unsafe {
            SetWindowSubclass(
                HWND(hwnd as *mut _),
                Some(subclass_proc),
                SUBCLASS_ID,
                refdata as usize,
            )
        };
        if !ok.as_bool() {
            // SAFETY: reclaiming the box we just leaked; nothing else holds it.
            drop(unsafe { Box::from_raw(refdata) });
            return None;
        }
        Some(Self {
            state,
            refdata,
            hwnd,
            last: AccessTree::default(),
        })
    }
}

impl Drop for WinAdapter {
    fn drop(&mut self) {
        // SAFETY: remove the subclass first so the procedure can no longer see
        // `refdata`, then free it. If the window is already destroyed the call
        // fails harmlessly (WM_NCDESTROY already removed it).
        unsafe {
            let _ =
                RemoveWindowSubclass(HWND(self.hwnd as *mut _), Some(subclass_proc), SUBCLASS_ID);
            drop(Box::from_raw(self.refdata));
        }
    }
}

impl AccessAdapter for WinAdapter {
    fn update(&mut self, tree: &AccessTree) {
        let diff = self.last.diff(tree);
        lock(&self.state).tree = tree.clone();
        // SAFETY: querying whether any client is listening is always allowed.
        let listening = unsafe { UiaClientsAreListening() }.as_bool();
        if listening && !diff.is_empty() {
            let root: IRawElementProviderSimple =
                IRawElementProviderFragment::from(Element::new(&self.state, None))
                    .cast()
                    .expect("root implements IRawElementProviderSimple");
            if !diff.added.is_empty() || !diff.removed.is_empty() || diff.reordered {
                // SAFETY: `root` is a live provider for this window.
                unsafe {
                    let _ = UiaRaiseStructureChangedEvent(
                        &root,
                        StructureChangeType_ChildrenInvalidated,
                        std::ptr::null_mut(),
                        0,
                    );
                }
            }
            for id in &diff.changed {
                let (Some(o), Some(n)) = (self.last.get(*id), tree.get(*id)) else {
                    continue;
                };
                let el: IRawElementProviderSimple =
                    IRawElementProviderFragment::from(Element::new(&self.state, Some(*id)))
                        .cast()
                        .expect("element implements IRawElementProviderSimple");
                // SAFETY: `el` is a live provider; VARIANTs are owned values.
                unsafe {
                    if o.label != n.label {
                        let _ = UiaRaiseAutomationPropertyChangedEvent(
                            &el,
                            UIA_NamePropertyId,
                            &variant_str(&o.label),
                            &variant_str(&n.label),
                        );
                    }
                    if o.checked != n.checked {
                        let st = |c: Option<bool>| {
                            VARIANT::from(if c == Some(true) {
                                ToggleState_On.0
                            } else {
                                ToggleState_Off.0
                            })
                        };
                        let _ = UiaRaiseAutomationPropertyChangedEvent(
                            &el,
                            UIA_ToggleToggleStatePropertyId,
                            &st(o.checked),
                            &st(n.checked),
                        );
                    }
                    if o.value != n.value {
                        if is_range(n.role) {
                            let _ = UiaRaiseAutomationPropertyChangedEvent(
                                &el,
                                UIA_RangeValueValuePropertyId,
                                &VARIANT::from(number(o)),
                                &VARIANT::from(number(n)),
                            );
                        } else {
                            let _ = UiaRaiseAutomationPropertyChangedEvent(
                                &el,
                                UIA_ValueValuePropertyId,
                                &variant_str(o.value.as_deref().unwrap_or("")),
                                &variant_str(n.value.as_deref().unwrap_or("")),
                            );
                        }
                    }
                    if o.enabled != n.enabled {
                        let _ = UiaRaiseAutomationPropertyChangedEvent(
                            &el,
                            UIA_IsEnabledPropertyId,
                            &VARIANT::from(o.enabled),
                            &VARIANT::from(n.enabled),
                        );
                    }
                    if !o.focused && n.focused {
                        let _ = UiaRaiseAutomationEvent(&el, UIA_AutomationFocusChangedEventId);
                    }
                }
            }
        }
        self.last = tree.clone();
    }
}
