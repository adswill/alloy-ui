//! The ambient view context: the window's [`Ui`] and automatic widget ids.
//!
//! While `view` (or `update`) runs, the facade makes the window's `Ui`
//! available through [`ui`], so widget helpers need no `&ui` argument. Widgets
//! that need a stable identity (inputs, sliders, scroll areas) get one from
//! their call site plus an occurrence counter, so `input(..)` just works and
//! `.id(n)` stays available as an override.

use alloy_ui_theme::{Palette, Theme};
use alloy_ui_widgets::Ui;
use std::{cell::RefCell, collections::HashMap, hash::Hasher, rc::Rc};

struct Cx {
    ui: Ui,
    seen: HashMap<u64, u32>,
}

thread_local! {
    static CX: RefCell<Option<Cx>> = const { RefCell::new(None) };
}

/// Run `f` with `ui` as the ambient context and the id counters reset.
pub(crate) fn enter<R>(ui: &Ui, f: impl FnOnce() -> R) -> R {
    let prev = CX.with(|c| {
        c.borrow_mut().replace(Cx {
            ui: ui.clone(),
            seen: HashMap::new(),
        })
    });
    let r = f();
    CX.with(|c| *c.borrow_mut() = prev);
    r
}

/// The window's [`Ui`]. Only valid inside `view`/`update` (or a test harness).
///
/// # Panics
/// Outside of those, with a message saying so.
pub fn ui() -> Ui {
    CX.with(|c| {
        c.borrow()
            .as_ref()
            .map(|c| c.ui.clone())
            .expect("alloy_ui widget helpers can only be used inside `App::view`")
    })
}

/// The current theme.
pub fn theme() -> Rc<Theme> {
    ui().theme()
}

/// The current theme's colours, for the odd custom colour.
pub fn palette() -> Palette {
    theme().palette.clone()
}

/// Switch the window between the light and dark theme.
pub fn set_dark(dark: bool) {
    ui().set_theme(if dark { Theme::dark() } else { Theme::light() });
}

/// A stable id for the widget built at the caller's source location.
#[track_caller]
pub(crate) fn auto_id(salt: u64) -> u64 {
    let loc = std::panic::Location::caller();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write(loc.file().as_bytes());
    h.write_u32(loc.line());
    h.write_u32(loc.column());
    h.write_u64(salt);
    let site = h.finish();
    let n = CX.with(|c| {
        let mut c = c.borrow_mut();
        let n = c.as_mut().map_or(0, |c| {
            let n = c.seen.entry(site).or_insert(0);
            *n += 1;
            *n
        });
        n as u64
    });
    // The top bit keeps generated ids clear of the small integers people pick by hand.
    (site.wrapping_add(n.wrapping_mul(0x9E37_79B9_7F4A_7C15))) | (1 << 63)
}

thread_local! {
    static SLOTS: RefCell<HashMap<u64, Box<dyn std::any::Any>>> = RefCell::new(HashMap::new());
}

/// The persistent value stored for `id`, created with `init` the first time.
pub(crate) fn slot<T: Send + 'static>(
    id: u64,
    init: impl FnOnce() -> T,
) -> std::sync::Arc<std::sync::Mutex<T>> {
    SLOTS.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(v) = s
            .get(&id)
            .and_then(|b| b.downcast_ref::<std::sync::Arc<std::sync::Mutex<T>>>())
        {
            return v.clone();
        }
        let v = std::sync::Arc::new(std::sync::Mutex::new(init()));
        s.insert(id, Box::new(v.clone()));
        v
    })
}
