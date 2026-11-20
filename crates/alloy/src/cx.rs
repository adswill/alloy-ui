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
    /// Mixed into every generated id, so the same call site gets a different identity per key.
    salt: u64,
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
            salt: 0,
        })
    });
    DURABLE.with(|d| d.borrow_mut().counts.clear());
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

/// Run `f` with every generated id (and so every [`local`](crate::local)) keyed by `key` as
/// well as its call site: the same code for row 3 and row 4 gets separate cursors and state.
pub(crate) fn keyed<R>(key: u64, f: impl FnOnce() -> R) -> R {
    let set = |to: u64| {
        CX.with(|c| {
            c.borrow_mut()
                .as_mut()
                .map_or(0, |c| std::mem::replace(&mut c.salt, to))
        })
    };
    let prev = set(0);
    set(prev.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ key.wrapping_add(0x1234_5678_9ABC_DEF1));
    let r = f();
    set(prev);
    r
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
    h.write_u64(CX.with(|c| c.borrow().as_ref().map_or(0, |c| c.salt)));
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

/// The slot stored for `id` (created earlier by [`slot`]).
pub(crate) fn slot_by_id<T: Send + 'static>(id: u64) -> std::sync::Arc<std::sync::Mutex<T>> {
    SLOTS.with(|s| {
        s.borrow()
            .get(&id)
            .and_then(|b| b.downcast_ref::<std::sync::Arc<std::sync::Mutex<T>>>())
            .expect("a Local used on a different thread or type than it was made on")
            .clone()
    })
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

// ------------------------------------------------------ durable (persisted) slots

#[derive(Default)]
struct Durable {
    loaded: HashMap<String, serde_json::Value>,
    live: std::collections::BTreeMap<String, Box<dyn Fn() -> serde_json::Value>>,
    scope: String,
    counts: HashMap<String, u32>,
}

thread_local! {
    static DURABLE: RefCell<Durable> = RefCell::new(Durable::default());
}

/// Give the next `store`s values that were saved by an earlier run.
pub(crate) fn durable_load(saved: HashMap<String, serde_json::Value>) {
    DURABLE.with(|d| d.borrow_mut().loaded = saved);
}

/// Every live `store`, as one JSON object.
pub(crate) fn durable_dump() -> serde_json::Value {
    DURABLE.with(|d| {
        let d = d.borrow();
        serde_json::Value::Object(d.live.iter().map(|(k, f)| (k.clone(), f())).collect())
    })
}

/// Name the page being built: its `store`s are keyed by this plus their order.
pub(crate) fn scope<R>(name: &str, f: impl FnOnce() -> R) -> R {
    let prev = DURABLE.with(|d| std::mem::replace(&mut d.borrow_mut().scope, name.into()));
    DURABLE.with(|d| d.borrow_mut().counts.remove(name));
    let r = f();
    DURABLE.with(|d| d.borrow_mut().scope = prev);
    r
}

/// The persistent, saved-with-the-app value for the next `store` call in this scope.
pub(crate) fn durable<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
    init: impl FnOnce() -> T,
) -> u64 {
    let key = DURABLE.with(|d| {
        let mut d = d.borrow_mut();
        let scope = d.scope.clone();
        let n = d.counts.entry(scope.clone()).or_insert(0);
        *n += 1;
        format!("{scope}#{n}")
    });
    durable_keyed(key, init)
}

/// Like [`durable`] with an explicit key, shared by every page that uses it.
pub(crate) fn durable_keyed<T: serde::Serialize + serde::de::DeserializeOwned + Send + 'static>(
    key: String,
    init: impl FnOnce() -> T,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    h.write(key.as_bytes());
    let id = h.finish() & !(1 << 63);
    let slot = slot(id, || {
        DURABLE
            .with(|d| d.borrow().loaded.get(&key).cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_else(init)
    });
    DURABLE.with(|d| {
        d.borrow_mut().live.entry(key).or_insert_with(|| {
            let s = slot.clone();
            Box::new(move || serde_json::to_value(&*s.lock().unwrap()).unwrap_or_default())
        });
    });
    id
}

/// The name of the page being built (empty outside [`Pages`](crate::Pages)).
pub(crate) fn scope_name() -> String {
    DURABLE.with(|d| d.borrow().scope.clone())
}
