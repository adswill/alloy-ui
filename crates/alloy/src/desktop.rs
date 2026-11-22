//! Desktop integration in one line each: menu bar, tray icon, global hotkeys, notifications
//! and file dialogs.
//!
//! ```no_run
//! use alloy_ui::prelude::*;
//! sketch(0, |n: &i64| col![n.to_string(), button("+").act(|n: &mut i64| *n += 1)])
//!     .menu("File", |m| m.item("Reset", "mod+r", |n| *n = 0).separator().item("Quit", "mod+q", |_| quit()))
//!     .tray("Counter", |t| t.item("Reset", |n| *n = 0).item("Quit", |_| quit()))
//!     .global_hotkey("ctrl+alt+c", |n| *n += 1)
//!     .run("Counter");
//! ```

use crate::{dsl::Act, keys::Hotkey};
use alloy_ui_platform::{
    dialog::{DialogKind, FileDialogOptions, FileFilter},
    menu::{Menu, MenuItem},
    notify::Notification,
    tray::TraySettings,
};
use std::path::PathBuf;

/// One line in a menu being built.
enum Entry<S> {
    Item {
        label: String,
        keys: String,
        act: Act<S>,
        enabled: bool,
    },
    /// A standard item the platform knows (`std:hide`, `std:copy`, ...).
    Standard {
        id: &'static str,
        label: &'static str,
        keys: &'static str,
    },
    Separator,
    Sub(String, Vec<Entry<S>>),
}

/// The items of one menu, built inside [`Sketch::menu`](crate::Sketch::menu).
pub struct MenuBuilder<S> {
    entries: Vec<Entry<S>>,
}

impl<S: 'static> MenuBuilder<S> {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// An item that runs `f` on the state. `keys` is its shortcut (`"mod+o"`: Cmd on macOS,
    /// Ctrl elsewhere), or `""` for none. Items also appear in the command palette.
    pub fn item(self, label: &str, keys: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.item_enabled(label, keys, true, f)
    }

    /// Like [`item`](Self::item) but greyed out and inert when `enabled` is false. The menu is
    /// rebuilt after every change by an [`App`](crate::App) that returns a fresh
    /// [`Desktop`](crate::Desktop), so the flag can follow the state there.
    pub fn item_enabled(
        mut self,
        label: &str,
        keys: &str,
        enabled: bool,
        f: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        self.entries.push(Entry::Item {
            label: label.into(),
            keys: keys.into(),
            act: Act::new(f),
            enabled,
        });
        self
    }

    fn standard(mut self, id: &'static str, label: &'static str, keys: &'static str) -> Self {
        self.entries.push(Entry::Standard { id, label, keys });
        self
    }

    /// Standard Cut (`mod+x`). Replayed as that key press to the focused widget, so a text
    /// field cuts its own selection and no app shortcut is stolen.
    pub fn cut(self) -> Self {
        self.standard("std:cut", "Cut", "mod+x")
    }

    /// Standard Copy (`mod+c`), see [`cut`](Self::cut).
    pub fn copy(self) -> Self {
        self.standard("std:copy", "Copy", "mod+c")
    }

    /// Standard Paste (`mod+v`), see [`cut`](Self::cut).
    pub fn paste(self) -> Self {
        self.standard("std:paste", "Paste", "mod+v")
    }

    /// Standard Select All (`mod+a`), see [`cut`](Self::cut).
    pub fn select_all(self) -> Self {
        self.standard("std:select_all", "Select All", "mod+a")
    }

    /// A thin line between groups of items.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// A nested menu: `.sub("Export", |m| m.item("CSV", "", |s| s.csv()))`.
    pub fn sub(
        mut self,
        title: &str,
        build: impl FnOnce(MenuBuilder<S>) -> MenuBuilder<S>,
    ) -> Self {
        self.entries
            .push(Entry::Sub(title.into(), build(MenuBuilder::new()).entries));
        self
    }
}

/// The application menu (the one named after the app, first in the macOS menu bar), built
/// inside [`Sketch::app_menu`](crate::Sketch::app_menu). Only macOS has such a menu; elsewhere
/// the custom items' shortcuts still work and the rest is ignored.
pub struct AppMenuBuilder<S>(MenuBuilder<S>);

impl<S: 'static> AppMenuBuilder<S> {
    /// The system About panel.
    pub fn about(self) -> Self {
        Self(self.0.standard("std:about", "About", ""))
    }

    /// A custom item, e.g. `.item("Settings...", "mod+,", |s| s.open_settings())`.
    pub fn item(self, label: &str, keys: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        Self(self.0.item(label, keys, f))
    }

    /// Like [`item`](Self::item) with an enabled flag.
    pub fn item_enabled(
        self,
        label: &str,
        keys: &str,
        enabled: bool,
        f: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        Self(self.0.item_enabled(label, keys, enabled, f))
    }

    /// A thin line between groups of items.
    pub fn separator(self) -> Self {
        Self(self.0.separator())
    }

    /// Hide the app (`mod+h`).
    pub fn hide(self) -> Self {
        Self(self.0.standard("std:hide", "Hide", "mod+h"))
    }

    /// Hide every other app (`mod+alt+h`).
    pub fn hide_others(self) -> Self {
        Self(
            self.0
                .standard("std:hide_others", "Hide Others", "mod+alt+h"),
        )
    }

    /// Show every hidden app.
    pub fn show_all(self) -> Self {
        Self(self.0.standard("std:show_all", "Show All", ""))
    }

    /// Quit the app (`mod+q`).
    pub fn quit(self) -> Self {
        Self(self.0.standard("alloy:quit", "Quit", "mod+q"))
    }
}

/// The tray icon being built inside [`Sketch::tray`](crate::Sketch::tray).
pub struct TrayBuilder<S> {
    icon: Option<(Vec<u8>, u32, u32)>,
    click: Option<Act<S>>,
    entries: Vec<Entry<S>>,
}

impl<S: 'static> TrayBuilder<S> {
    /// A context-menu item that runs `f` on the state.
    pub fn item(mut self, label: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.entries.push(Entry::Item {
            label: label.into(),
            keys: String::new(),
            act: Act::new(f),
            enabled: true,
        });
        self
    }

    /// A thin line between groups of items.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// Run `f` when the icon itself is clicked.
    pub fn click(mut self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.click = Some(Act::new(f));
        self
    }

    /// Use your own icon: `width * height * 4` bytes of RGBA, 32x32 is a good size. The default
    /// is a rounded square with an A.
    pub fn icon(mut self, rgba: Vec<u8>, width: u32, height: u32) -> Self {
        self.icon = Some((rgba, width, height));
        self
    }
}

/// Everything the app asks of the desktop beyond its window, as plain data the shell applies.
/// Made by [`App::desktop`](crate::App::desktop).
#[derive(Clone, Default, PartialEq, Debug)]
pub struct DesktopSpec {
    /// Top-level menus, in order.
    pub menus: Vec<Menu>,
    /// The application menu (first in the macOS menu bar), if the app asked for one with
    /// [`Sketch::app_menu`](crate::Sketch::app_menu). Ignored off macOS.
    pub app_menu: Option<Menu>,
    /// The tray icon, if any.
    pub tray: Option<TraySettings>,
    /// Global hotkeys, as registered.
    pub hotkeys: Vec<String>,
}

/// The menu bar, tray and global hotkeys of an app over state `S`. Held by
/// [`Sketch`](crate::Sketch) and [`Pages`](crate::Pages). An [`App`](crate::App) written by hand can hold one and return its `spec()` and `hotkeys()`.
pub struct Desktop<S> {
    menus: Vec<(String, Vec<Entry<S>>)>,
    app_menu: Option<Vec<Entry<S>>>,
    tray: Option<(String, TrayBuilder<S>)>,
    globals: Vec<(String, Act<S>)>,
}

impl<S> Default for Desktop<S> {
    fn default() -> Self {
        Self {
            menus: Vec::new(),
            app_menu: None,
            tray: None,
            globals: Vec::new(),
        }
    }
}

/// The name the desktop uses for a menu item: its keys when it has some (the shell replays
/// them as a key press), otherwise `cmd:<label>`.
fn item_id(keys: &str, label: &str) -> String {
    if keys.is_empty() {
        format!("cmd:{label}")
    } else {
        keys.to_string()
    }
}

fn to_menu<S>(title: &str, entries: &[Entry<S>], tray: bool) -> Menu {
    let items = entries
        .iter()
        .map(|e| match e {
            Entry::Separator => MenuItem::Separator,
            Entry::Sub(t, es) => MenuItem::Submenu(to_menu(t, es, tray)),
            Entry::Standard { id, label, keys } => MenuItem::Action {
                id: (*id).into(),
                label: (*label).into(),
                shortcut: (!keys.is_empty()).then(|| (*keys).into()),
                enabled: true,
            },
            Entry::Item {
                label,
                keys,
                enabled,
                ..
            } => MenuItem::Action {
                id: if tray {
                    format!("tray:{label}")
                } else {
                    item_id(keys, label)
                },
                label: label.clone(),
                shortcut: (!keys.is_empty()).then(|| keys.clone()),
                enabled: *enabled,
            },
        })
        .collect();
    Menu {
        title: title.into(),
        items,
    }
}

fn hotkeys<S: 'static>(entries: &[Entry<S>], tray: bool, out: &mut Vec<Hotkey<Act<S>>>) {
    for e in entries {
        match e {
            Entry::Separator | Entry::Standard { .. } => {}
            Entry::Sub(_, es) => hotkeys(es, tray, out),
            // A disabled item neither fires from its shortcut nor shows in the palette.
            Entry::Item { enabled: false, .. } => {}
            Entry::Item { label, act, .. } if tray => {
                out.push(Hotkey::hidden(&format!("tray:{label}"), act.clone()))
            }
            Entry::Item {
                label, keys, act, ..
            } if keys.is_empty() => out.push(Hotkey::command(label, act.clone())),
            Entry::Item {
                label, keys, act, ..
            } => out.push(Hotkey::new(keys, act.clone()).labelled(label)),
        }
    }
}

impl<S: 'static> Desktop<S> {
    /// Add a top-level menu.
    pub fn menu(&mut self, title: &str, build: impl FnOnce(MenuBuilder<S>) -> MenuBuilder<S>) {
        self.menus
            .push((title.into(), build(MenuBuilder::new()).entries));
    }

    /// Set the application menu (macOS): `.app_menu(|m| m.about().separator().hide().quit())`.
    pub fn app_menu(&mut self, build: impl FnOnce(AppMenuBuilder<S>) -> AppMenuBuilder<S>) {
        self.app_menu = Some(build(AppMenuBuilder(MenuBuilder::new())).0.entries);
    }

    /// Show a tray icon with `tooltip`.
    pub fn tray(&mut self, tooltip: &str, build: impl FnOnce(TrayBuilder<S>) -> TrayBuilder<S>) {
        let t = TrayBuilder {
            icon: None,
            click: None,
            entries: Vec::new(),
        };
        self.tray = Some((tooltip.into(), build(t)));
    }

    /// Run `f` whenever `keys` is pressed anywhere on the desktop.
    pub fn global_hotkey(&mut self, keys: &str, f: impl Fn(&mut S) + Send + Sync + 'static) {
        self.globals.push((keys.into(), Act::new(f)));
    }

    /// The commands behind every menu item, tray item and global hotkey.
    pub fn hotkeys(&self) -> Vec<Hotkey<Act<S>>> {
        let mut out = Vec::new();
        for (_, es) in &self.menus {
            hotkeys(es, false, &mut out);
        }
        if let Some(es) = &self.app_menu {
            hotkeys(es, false, &mut out);
        }
        if let Some((_, t)) = &self.tray {
            hotkeys(&t.entries, true, &mut out);
            if let Some(c) = &t.click {
                out.push(Hotkey::hidden("tray:click", c.clone()));
            }
        }
        for (k, a) in &self.globals {
            out.push(Hotkey::hidden(&format!("global:{k}"), a.clone()));
        }
        out
    }

    /// What to hand the shell.
    pub fn spec(&self) -> DesktopSpec {
        DesktopSpec {
            menus: self
                .menus
                .iter()
                .map(|(t, es)| to_menu(t, es, false))
                .collect(),
            app_menu: self.app_menu.as_ref().map(|es| to_menu("", es, false)),
            tray: self.tray.as_ref().map(|(tip, t)| {
                let (icon_rgba, icon_width, icon_height) =
                    t.icon.clone().unwrap_or_else(default_icon);
                TraySettings {
                    tooltip: tip.clone(),
                    icon_rgba,
                    icon_width,
                    icon_height,
                    menu: (!t.entries.is_empty()).then(|| to_menu(tip, &t.entries, true)),
                }
            }),
            hotkeys: self.globals.iter().map(|(k, _)| k.clone()).collect(),
        }
    }
}

/// A 32x32 rounded indigo square with a white A, drawn with distance fields so its edges are smooth.
fn default_icon() -> (Vec<u8>, u32, u32) {
    const N: u32 = 32;
    fn seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
        let (px, py, ax, ay) = (p.0 - a.0, p.1 - a.1, b.0 - a.0, b.1 - a.1);
        let t = ((px * ax + py * ay) / (ax * ax + ay * ay)).clamp(0.0, 1.0);
        ((px - ax * t).powi(2) + (py - ay * t).powi(2)).sqrt()
    }
    let mut px = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            // Rounded square, radius 7, inset 1.
            let (dx, dy) = ((p.0 - 16.0).abs() - 9.0, (p.1 - 16.0).abs() - 9.0);
            let sq = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() + dx.max(dy).min(0.0) - 6.0;
            let cover = (0.5 - sq).clamp(0.0, 1.0);
            let d = seg(p, (9.0, 24.0), (16.0, 8.0))
                .min(seg(p, (16.0, 8.0), (23.0, 24.0)))
                .min(seg(p, (11.8, 19.0), (20.2, 19.0)));
            let ink = (1.5 + 0.5 - d).clamp(0.0, 1.0);
            let mix = |bg: f32| bg + (255.0 - bg) * ink;
            px.extend([
                mix(79.0) as u8,
                mix(107.0) as u8,
                mix(237.0) as u8,
                (cover * 255.0) as u8,
            ]);
        }
    }
    (px, N, N)
}

// ------------------------------------------------------------------ notifications

/// Show a desktop notification: `notify("Export finished", "report.csv is ready")`.
/// Never blocks the window and never fails loudly: a desktop with no notification service
/// is logged and ignored.
pub fn notify(title: &str, body: &str) {
    let n = Notification {
        title: title.into(),
        body: body.into(),
        timeout_ms: None,
    };
    let sent = std::thread::Builder::new()
        .name("alloy-notify".into())
        .spawn(move || {
            if let Err(e) = alloy_ui_platform::notify::send(&n) {
                log::warn!("{e}");
            }
        });
    if let Err(e) = sent {
        log::warn!("notification thread: {e}");
    }
}

// ------------------------------------------------------------------ file dialogs

/// A native file dialog being set up. Start one with [`files::open`](crate::files::open),
/// [`save`](crate::files::save), [`open_many`](crate::files::open_many) or
/// [`folder`](crate::files::folder), then call [`pick`](Self::pick).
pub struct FileDialog(FileDialogOptions);

impl FileDialog {
    pub(crate) fn new(kind: DialogKind, name: &str) -> Self {
        Self(FileDialogOptions {
            kind,
            name: name.into(),
            ..Default::default()
        })
    }

    /// Window title.
    pub fn title(mut self, title: &str) -> Self {
        self.0.title = Some(title.into());
        self
    }

    /// Folder to start in.
    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.0.dir = Some(dir.into());
        self
    }

    /// Offer a kind of file: `.filter("Images", &["png", "jpg"])`. Call it again for more kinds.
    pub fn filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.0.filters.push(FileFilter {
            name: name.into(),
            extensions: extensions.iter().map(|e| e.to_string()).collect(),
        });
        self
    }

    /// Show the dialog and wait. `None` when the user cancels, or when the system has no dialog
    /// (the reason is logged).
    pub fn pick(self) -> Option<PathBuf> {
        self.pick_all().into_iter().next()
    }

    /// Like [`pick`](Self::pick), for every chosen path (several with [`open_many`](crate::files::open_many)).
    pub fn pick_all(self) -> Vec<PathBuf> {
        match alloy_ui_platform::dialog::show(&self.0) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("{e}");
                Vec::new()
            }
        }
    }
}

/// Native file dialogs: `files::open().filter("Images", &["png"]).pick()`.
pub mod files {
    use super::{DialogKind, FileDialog};

    /// Choose one existing file.
    pub fn open() -> FileDialog {
        FileDialog::new(DialogKind::OpenFile, "")
    }

    /// Choose several existing files; read them with [`pick_all`](FileDialog::pick_all).
    pub fn open_many() -> FileDialog {
        FileDialog::new(DialogKind::OpenFiles, "")
    }

    /// Choose where to save, suggesting `name`. The system asks before overwriting.
    pub fn save(name: &str) -> FileDialog {
        FileDialog::new(DialogKind::Save, name)
    }

    /// Choose a folder.
    pub fn folder() -> FileDialog {
        FileDialog::new(DialogKind::Folder, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Desktop<i32> {
        let mut d = Desktop::default();
        d.menu("File", |m| {
            m.item("Open", "mod+o", |n: &mut i32| *n += 1)
                .separator()
                .item("About", "", |_| {})
                .sub("Export", |s| s.item("CSV", "", |_| {}))
        });
        d.tray("T", |t| t.item("Show", |_| {}).click(|_| {}));
        d.global_hotkey("ctrl+alt+x", |_| {});
        d
    }

    #[test]
    fn menu_ids_follow_keys_or_label() {
        let spec = sample().spec();
        let file = &spec.menus[0];
        assert_eq!(file.title, "File");
        assert_eq!(file.find("mod+o"), Some("Open"));
        assert_eq!(file.find("cmd:About"), Some("About"));
        assert_eq!(file.find("cmd:CSV"), Some("CSV"));
        assert_eq!(spec.hotkeys, vec!["ctrl+alt+x".to_string()]);
        let tray = spec.tray.unwrap();
        assert!(tray.icon_valid());
        assert_eq!(tray.menu.unwrap().find("tray:Show"), Some("Show"));
    }

    #[test]
    fn every_entry_is_a_command_the_page_can_run() {
        let ids: Vec<_> = sample()
            .hotkeys()
            .iter()
            .map(|h| (h.keys.clone(), h.label.clone(), h.id.clone()))
            .collect();
        // Labelled with keys, labelled without, nested, tray item, tray click, global.
        assert!(ids.contains(&("mod+o".into(), Some("Open".into()), None)));
        assert!(ids.contains(&("".into(), Some("About".into()), None)));
        assert!(ids.contains(&("".into(), Some("CSV".into()), None)));
        assert!(ids.contains(&("".into(), None, Some("tray:Show".into()))));
        assert!(ids.contains(&("".into(), None, Some("tray:click".into()))));
        assert!(ids.contains(&("".into(), None, Some("global:ctrl+alt+x".into()))));
    }

    #[test]
    fn default_icon_is_opaque_in_the_middle_and_clear_at_the_corner() {
        let (px, w, h) = default_icon();
        assert_eq!(px.len(), (w * h * 4) as usize);
        assert_eq!(px[3], 0, "corner is transparent");
        let mid = ((20 * w + 6) * 4 + 3) as usize;
        assert_eq!(px[mid], 255, "body is opaque");
    }
}
