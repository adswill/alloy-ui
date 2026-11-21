//! The ambient view context: the window's [`Ui`] and automatic widget ids.
//!
//! While `view` (or `update`) runs, the facade makes the window's `Ui`
//! available through [`ui`], so widget helpers need no `&ui` argument. Widgets
//! that need a stable identity (inputs, sliders, scroll areas) get one from
//! their call site plus an occurrence counter, so `input(..)` just works and
//! `.id(n)` stays available as an override.

use alloy_ui_core::Color;
use alloy_ui_theme::{Palette, Theme};
use alloy_ui_widgets::Ui;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    hash::Hasher,
    rc::Rc,
};

struct Cx {
    ui: Ui,
    seen: HashMap<u64, u32>,
    /// Mixed into every generated id, so the same call site gets a different identity per key.
    salt: u64,
}

thread_local! {
    /// The `Ui` of every open window, so the shell host and the app adapter agree on them.
    static WINDOW_UIS: RefCell<HashMap<alloy_ui_core::WindowId, Ui>> = RefCell::new(HashMap::new());
    static CX: RefCell<Option<Cx>> = const { RefCell::new(None) };
}

/// Run `f` with `ui` as the ambient context and the id counters reset.
/// Run `f` with `ui` as the ambient context, ids salted by `key`, without touching the durable
/// counters: for building a piece of the tree later than `view` (a virtualised row, at draw time).
pub(crate) fn scoped<R>(ui: &Ui, key: u64, f: impl FnOnce() -> R) -> R {
    let prev = CX.with(|c| {
        c.borrow_mut().replace(Cx {
            ui: ui.clone(),
            seen: HashMap::new(),
            salt: key.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x7157_0001,
        })
    });
    let r = f();
    CX.with(|c| *c.borrow_mut() = prev);
    r
}

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

/// Switch the window between the light and dark theme (an accent set with [`set_accent`] stays).
pub fn set_dark(dark: bool) {
    let mut t = if dark { Theme::dark() } else { Theme::light() };
    if let Some(c) = ACCENT.with(|a| a.get()) {
        t = t.with_accent(c);
    }
    ui().set_theme(t);
}

thread_local! {
    static ACCENT: std::cell::Cell<Option<Color>> = const { std::cell::Cell::new(None) };
}

/// Recolour the whole window with a brand accent, keeping light or dark. The accent is nudged
/// until it passes WCAG AA on the theme's surfaces. Cheap to call every frame: nothing happens
/// if the window already has that accent.
pub fn set_accent(brand: alloy_ui_core::Color) {
    ACCENT.with(|a| a.set(Some(brand)));
    let t = theme().as_ref().clone().with_accent(brand);
    if t.palette.accent != theme().palette.accent {
        ui().set_theme(t);
    }
}

/// A complete look: appearance, optional brand accent, contrast and density. Build one and
/// apply it with [`set_look`]; every part is optional.
///
/// ```ignore
/// set_look(Look::dark().accent(Color::hex(0x6D4AFF)).density(Density::Compact));
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Look {
    dark: bool,
    accent: Option<alloy_ui_core::Color>,
    high_contrast: bool,
    density: alloy_ui_theme::Density,
}

impl Look {
    /// The light theme.
    pub fn light() -> Self {
        Self::default()
    }
    /// The dark theme.
    pub fn dark() -> Self {
        Self {
            dark: true,
            ..Self::default()
        }
    }
    /// Pick light or dark from a flag.
    pub fn is_dark(self, dark: bool) -> Self {
        Self { dark, ..self }
    }
    /// Use a brand accent.
    pub fn accent(self, c: alloy_ui_core::Color) -> Self {
        Self {
            accent: Some(c),
            ..self
        }
    }
    /// 7:1 text, solid borders.
    pub fn high_contrast(self, on: bool) -> Self {
        Self {
            high_contrast: on,
            ..self
        }
    }
    /// Spacing preset.
    pub fn density(self, d: alloy_ui_theme::Density) -> Self {
        Self { density: d, ..self }
    }
    /// The theme this look describes.
    pub fn theme(self) -> Theme {
        let mut t = if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        };
        if let Some(c) = self.accent {
            t = t.with_accent(c);
        }
        if self.high_contrast {
            t = t.high_contrast();
        }
        t.density(self.density)
    }
}

/// Apply a [`Look`] to the window.
pub fn set_look(look: Look) {
    ACCENT.with(|a| a.set(look.accent));
    ui().set_theme(look.theme());
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
    put: std::collections::BTreeMap<String, Box<dyn Fn(serde_json::Value)>>,
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

/// The page data worth undoing: every `store` except the theme and the open page.
pub(crate) fn durable_data() -> serde_json::Value {
    match durable_dump() {
        serde_json::Value::Object(m) => serde_json::Value::Object(
            m.into_iter()
                .filter(|(k, _)| k != "=dark" && !k.starts_with('#'))
                .collect(),
        ),
        v => v,
    }
}

/// Put back what [`durable_data`] returned.
pub(crate) fn durable_restore(v: serde_json::Value) {
    if let serde_json::Value::Object(m) = v {
        DURABLE.with(|d| {
            let d = d.borrow();
            for (k, v) in m {
                if let Some(put) = d.put.get(&k) {
                    put(v);
                }
            }
        });
    }
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
        let mut d = d.borrow_mut();
        let s = slot.clone();
        d.put.entry(key.clone()).or_insert_with(|| {
            Box::new(move |v| {
                if let Ok(v) = serde_json::from_value(v) {
                    *s.lock_ok() = v;
                }
            })
        });
        d.live.entry(key).or_insert_with(|| {
            let s = slot.clone();
            Box::new(move || serde_json::to_value(&*s.lock_ok()).unwrap_or_default())
        });
    });
    id
}

/// The name of the page being built (empty outside [`Pages`](crate::Pages)).
pub(crate) fn scope_name() -> String {
    DURABLE.with(|d| d.borrow().scope.clone())
}

/// Remember `ui` as window `id`'s widget state.
pub(crate) fn register_window_ui(id: alloy_ui_core::WindowId, ui: &Ui) {
    WINDOW_UIS.with(|m| m.borrow_mut().insert(id, ui.clone()));
}

/// The `Ui` of window `id`, if it has one.
pub(crate) fn window_ui(id: alloy_ui_core::WindowId) -> Option<Ui> {
    WINDOW_UIS.with(|m| m.borrow().get(&id).cloned())
}

/// Drop a closed window's widget state.
pub(crate) fn forget_window_ui(id: alloy_ui_core::WindowId) {
    WINDOW_UIS.with(|m| m.borrow_mut().remove(&id));
}

/// The `Ui` of window `id`, created on first use as a fresh copy of `template`:
/// same theme, text metrics and clipboard, but its own focus, scroll and caret state.
pub(crate) fn window_ui_or_fork(id: alloy_ui_core::WindowId, template: &Ui) -> Ui {
    if let Some(ui) = window_ui(id) {
        return ui;
    }
    let ui = Ui::new((*template.theme()).clone());
    ui.set_metrics(template.metrics());
    ui.set_clipboard(template.clipboard());
    ui.set_reduced_motion(template.reduced_motion());
    register_window_ui(id, &ui);
    ui
}

thread_local! {
    static PAGE_ALPHA: Cell<f32> = const { Cell::new(1.0) };
    static SYSTEM: Cell<alloy_ui_platform::settings::SystemSettings> =
        const { Cell::new(alloy_ui_platform::settings::SystemSettings {
            dark: false,
            reduced_motion: false,
            high_contrast: false,
            accent: None,
        }) };
    static HOVERING_FILES: Cell<bool> = const { Cell::new(false) };
}

/// Opacity of the page background: below 1 while a window effect shows through.
pub(crate) fn page_alpha() -> f32 {
    PAGE_ALPHA.with(Cell::get)
}

pub(crate) fn set_page_alpha(a: f32) {
    PAGE_ALPHA.with(|c| c.set(a));
}

/// What the OS reported about dark mode, reduced motion, contrast and accent colour.
/// Until the OS has answered (a few milliseconds after start-up) every flag is off.
pub fn system() -> alloy_ui_platform::settings::SystemSettings {
    SYSTEM.with(Cell::get)
}

pub(crate) fn set_system(s: alloy_ui_platform::settings::SystemSettings) {
    SYSTEM.with(|c| c.set(s));
}

/// True while files from another app are being dragged over the window: show a "drop here" hint.
/// The view is rebuilt when it changes.
pub fn files_hovering() -> bool {
    HOVERING_FILES.with(Cell::get)
}

pub(crate) fn set_files_hovering(on: bool) -> bool {
    HOVERING_FILES.with(|c| c.replace(on)) != on
}

/// Recolour every window with the accent `c`, keeping light or dark.
pub(crate) fn apply_accent_everywhere(c: Color) {
    ACCENT.with(|a| a.set(Some(c)));
    let all: Vec<Ui> = WINDOW_UIS.with(|m| m.borrow().values().cloned().collect());
    for ui in all {
        let t = ui.theme().as_ref().clone().with_accent(c);
        if t.palette.accent != ui.theme().palette.accent {
            ui.set_theme(t);
        }
    }
}

/// Set reduced motion on every window.
pub(crate) fn set_reduced_motion_everywhere(on: bool) {
    let all: Vec<Ui> = WINDOW_UIS.with(|m| m.borrow().values().cloned().collect());
    for ui in all {
        ui.set_reduced_motion(on);
    }
}

/// `Mutex::lock` that survives poisoning: a panicking worker must not take the UI thread
/// down with it, and the guarded state is plain data that stays valid.
pub(crate) trait LockExt<T> {
    fn lock_ok(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockExt<T> for std::sync::Mutex<T> {
    fn lock_ok(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
