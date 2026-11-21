//! Plugins: third-party widget crates, by convention and a small trait.
//!
//! A plugin is an ordinary Rust crate that depends on `alloy-ui`, never a dynamically loaded
//! library: it is compiled into the app, type-checked against it, and its widgets cost nothing
//! when unused. Three pieces make that pleasant:
//!
//! * **Widgets** implement [`Widget`](crate::Widget) and are written with the helpers in
//!   [`ext`] (Tab order, focus ring, press tracking, per-widget state). A plugin exports builder
//!   functions that return a [`Node`](crate::Node), exactly like `button` or `slider`.
//! * **[`Plugin`]** bundles what a crate brings beyond widgets, today its translations, and
//!   registers it with one call, [`install`].
//! * **Conventions** (naming, features, tests) are in `docs/PLUGINS.md`; the worked example is
//!   the `alloy-ui-rating` crate under `examples/plugin-rating`.
//!
//! ```
//! use alloy_ui::plugin::{self, Host, Plugin};
//!
//! struct Rating;
//! impl Plugin for Rating {
//!     fn name(&self) -> &str { "rating" }
//!     fn version(&self) -> &str { "1.0.0" }
//!     fn install(&self, host: &mut Host) {
//!         host.strings("en", "rating.label = Rated {n} of 5");
//!     }
//! }
//!
//! plugin::install(&Rating);
//! plugin::install(&Rating); // installing twice is a no-op
//! assert!(plugin::is_installed("rating"));
//! assert_eq!(plugin::installed(), vec![("rating".to_string(), "1.0.0".to_string())]);
//! assert_eq!(alloy_ui::i18n::translate("rating.label", &[("n", "4".into())]), "Rated 4 of 5");
//! ```

/// Helpers for writing widgets outside the framework: the [`alloy_ui_widgets::ext`] functions
/// plus the app's `ui` handle, `theme` and `auto_id`.
pub mod ext {
    pub use crate::cx::{theme, ui};
    pub use alloy_ui_widgets::ext::*;

    /// A stable focus id for the widget built at the caller's source location, the same one the
    /// built-in builders use. Call it in a builder function marked `#[track_caller]`; pass a
    /// distinct `salt` per widget kind.
    #[track_caller]
    pub fn auto_id(salt: u64) -> u64 {
        crate::cx::auto_id(salt)
    }
}
use std::sync::Mutex;

/// What a plugin crate registers with an app. See the [module docs](self).
pub trait Plugin {
    /// A short unique name, usually the crate name without the `alloy-ui-` prefix. Installing a
    /// second plugin with the same name does nothing.
    fn name(&self) -> &str;
    /// The plugin's version, shown by [`installed`] (for an About box or a bug report).
    fn version(&self) -> &str {
        ""
    }
    /// One sentence on what the plugin adds, for an About box or a plugin list.
    fn description(&self) -> &str {
        ""
    }
    /// Register everything the plugin contributes through `host`.
    fn install(&self, host: &mut Host);
}

/// The registration handle passed to [`Plugin::install`].
pub struct Host {
    plugin: String,
}

impl Host {
    /// Add translations to `locale` (a tag such as `"en"` or `"fr"`) from catalogue text in the
    /// [`i18n`](crate::i18n) format. Prefix keys with the plugin name (`rating.label`) so
    /// two plugins never clash; keys already defined by the app win nothing and lose nothing,
    /// the last `add` for a key is used.
    pub fn strings(&mut self, locale: &str, catalog: &str) {
        crate::i18n::add(locale, catalog);
    }
    /// The name of the plugin being installed.
    pub fn plugin(&self) -> &str {
        &self.plugin
    }
}

// Process-wide, like the i18n catalogues plugins add to.
static INSTALLED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn installed_list() -> std::sync::MutexGuard<'static, Vec<(String, String)>> {
    INSTALLED.lock().unwrap_or_else(|e| e.into_inner())
}

/// Install `plugin` into this app, once, before the first view.
/// Does nothing when a plugin of the same [`name`](Plugin::name) is already installed.
pub fn install(plugin: &dyn Plugin) {
    // A plugin must not be able to deadlock the registry by installing from `install`: the
    // lock is released before `install` runs (below).
    let name = plugin.name().to_string();
    let fresh = {
        let mut i = installed_list();
        if i.iter().any(|(n, _)| *n == name) {
            false
        } else {
            i.push((name.clone(), plugin.version().to_string()));
            true
        }
    };
    if fresh {
        plugin.install(&mut Host { plugin: name });
    }
}

/// Install several plugins in order (same as calling [`install`] for each).
pub fn install_all(plugins: &[&dyn Plugin]) {
    for p in plugins {
        install(*p);
    }
}

/// True when a plugin called `name` is installed: lets a plugin check that one it builds on
/// is present, or an app show a feature only when its plugin is.
pub fn is_installed(name: &str) -> bool {
    installed_list().iter().any(|(n, _)| n == name)
}

/// `(name, version)` of every installed plugin, in install order.
pub fn installed() -> Vec<(String, String)> {
    installed_list().clone()
}
