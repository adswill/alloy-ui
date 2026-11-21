//! The `app` shell: a sidebar of pages.

use super::*;

type PageFn = Box<dyn Fn() -> Node<Act<()>>>;

/// A multi-page app with a sidebar and no state struct: every page owns its state
/// in [`store`]s. Made by [`app`].
pub struct Pages {
    title: String,
    pages: Vec<(String, PageFn)>,
    size: (f32, f32),
    save: Option<std::path::PathBuf>,
    keys: Vec<(String, Option<String>, Act<()>)>,
    undo: bool,
    desktop: crate::Desktop<()>,
}

/// Start a multi-page app: `app("Admin").page("People", || ..).page("Settings", || ..).run()`.
pub fn app(title: &str) -> Pages {
    Pages {
        title: title.into(),
        pages: Vec::new(),
        size: (1280.0, 720.0),
        save: None,
        keys: Vec::new(),
        undo: false,
        desktop: crate::Desktop::default(),
    }
}

impl Pages {
    /// A "Settings" page: your controls (a [`section`](crate::section) each, say) under the
    /// built-in heading, above the Light / Dark switch every app wants.
    pub fn settings<N: Into<Node<Act<()>>>>(self, body: impl Fn() -> N + 'static) -> Self {
        self.page("Settings", move || {
            col([
                Node::from(crate::heading("Settings")),
                body().into(),
                crate::section("Theme", theme_switch()).into(),
            ])
            .gap(16.0)
            .pad(24.0)
        })
    }
    /// A "Settings" page with only the Light / Dark switch.
    pub fn theme_page(self) -> Self {
        self.settings(|| col::<Act<()>, Node<Act<()>>>([]))
    }
    /// Add a sidebar entry. `view` builds the page while it is showing.
    pub fn page<N: Into<Node<Act<()>>>>(
        mut self,
        name: &str,
        view: impl Fn() -> N + 'static,
    ) -> Self {
        self.pages
            .push((name.into(), Box::new(move || view().into())));
        self
    }
    /// Run `f` when `keys` is pressed (`"mod+n"`: Cmd on macOS, Ctrl elsewhere). Blocks such as
    /// [`browse`](crate::browse) add their own (`mod+n` is New); to react to a key inside a page
    /// use [`Shortcuts::shortcut`](crate::Shortcuts::shortcut).
    pub fn shortcut<R>(self, keys: &str, f: impl Fn() -> R + Send + Sync + 'static) -> Self {
        self.key(keys, None, f)
    }
    /// Add `label` to the command palette (`mod+k`): `.command("Export CSV", || export())`.
    /// Every shortcut is listed there too.
    pub fn command<R>(self, label: &str, f: impl Fn() -> R + Send + Sync + 'static) -> Self {
        self.key("", Some(label), f)
    }
    /// Like [`shortcut`](Self::shortcut), and listed in the menu bar as `label`.
    pub fn shortcut_as<R>(
        self,
        keys: &str,
        label: &str,
        f: impl Fn() -> R + Send + Sync + 'static,
    ) -> Self {
        self.key(keys, Some(label), f)
    }
    fn key<R>(
        mut self,
        keys: &str,
        label: Option<&str>,
        f: impl Fn() -> R + Send + Sync + 'static,
    ) -> Self {
        let act = Act::new(move |_: &mut ()| {
            f();
        });
        self.keys.push((keys.into(), label.map(Into::into), act));
        self
    }
    /// Add a menu to the menu bar: `.menu("File", |m| m.item("Export", "mod+e", || export()))`.
    /// Items run a plain closure, as [`shortcut`](Self::shortcut) does.
    pub fn menu(
        mut self,
        title: &str,
        build: impl FnOnce(crate::MenuBuilder<()>) -> crate::MenuBuilder<()>,
    ) -> Self {
        self.desktop.menu(title, build);
        self
    }
    /// Put an icon in the system tray with a context menu, as [`Sketch::tray`](crate::Sketch::tray).
    pub fn tray(
        mut self,
        tooltip: &str,
        build: impl FnOnce(crate::TrayBuilder<()>) -> crate::TrayBuilder<()>,
    ) -> Self {
        self.desktop.tray(tooltip, build);
        self
    }
    /// Run `f` when `keys` is pressed anywhere on the desktop, as
    /// [`Sketch::global_hotkey`](crate::Sketch::global_hotkey).
    pub fn global_hotkey(mut self, keys: &str, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.desktop.global_hotkey(keys, move |_: &mut ()| f());
        self
    }
    /// Undo and redo for every change to a [`store`] (`mod+z`, `mod+shift+z`, and the Undo
    /// button of [`toast(..).undo()`](crate::toast)). The theme and the open page are not undone.
    pub fn undoable(mut self) -> Self {
        self.undo = true;
        self
    }
    /// Window size in logical pixels.
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.size = (w, h);
        self
    }
    /// Keep every [`store`] (and the open page) between runs, under the app's title.
    pub fn persist(mut self) -> Self {
        let path = crate::persist::path(&self.title);
        if let Some(saved) = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            cx::durable_load(saved);
        }
        self.save = Some(path);
        self
    }
    /// Open the window and run until it closes.
    pub fn run(self) {
        let (t, (w, h)) = (self.title.clone(), self.size);
        App::run_sized(self, &t, w, h)
    }
}

impl App for Pages {
    type Msg = Act<()>;

    fn update(&mut self, m: Act<()>) {
        (m.0)(&mut ());
        if let Some(path) = &self.save {
            crate::persist::write(path, &cx::durable_dump());
        }
    }

    fn subscriptions(&self) -> Vec<crate::Sub<Act<()>>> {
        crate::resource::pump_sub()
            .into_iter()
            .chain(crate::live::poll_sub())
            .collect()
    }

    fn shortcuts(&self) -> Vec<crate::keys::Hotkey<Act<()>>> {
        self.keys
            .iter()
            .map(|(k, l, a)| {
                let h = crate::keys::Hotkey::new(k, a.clone());
                match l {
                    Some(l) => h.labelled(l),
                    None => h,
                }
            })
            .chain(self.desktop.hotkeys())
            .collect()
    }

    fn desktop(&self) -> crate::DesktopSpec {
        self.desktop.spec()
    }

    fn snapshot(&self) -> Option<crate::keys::Snap> {
        self.undo
            .then(|| crate::keys::Snap::new(cx::durable_data()))
    }

    fn restore(&mut self, snap: crate::keys::Snap) {
        if let Some(v) = snap.take::<serde_json::Value>() {
            cx::durable_restore(v);
            if let Some(path) = &self.save {
                crate::persist::write(path, &cx::durable_dump());
            }
        }
    }

    fn meta(&self, what: crate::keys::Meta) -> Option<Act<()>> {
        self.undo.then(|| crate::keys::meta_act(what))
    }

    fn view(&self) -> impl View<Act<()>> {
        let dark = store_as("dark", || false).get();
        if (alloy_ui_theme::relative_luminance(cx::palette().background) < 0.5) != dark {
            cx::set_dark(dark);
        }
        let open = store(|| 0usize);
        let at = open.get().min(self.pages.len().saturating_sub(1));
        let names: Vec<&str> = self.pages.iter().map(|(n, _)| n.as_str()).collect();
        let side: Node<Act<()>> = crate::dsl::Nav::of(
            names
                .iter()
                .enumerate()
                .map(|(i, n)| (n.to_string(), i))
                .collect(),
            at,
        )
        .on(move |i| {
            Act::new({
                let open = open;
                move |_: &mut ()| open.set(i)
            })
        })
        .into();
        // Every app gets these in its command palette (mod+k).
        let theme = store_as("dark", || false);
        for (i, n) in names.iter().enumerate().filter(|(i, _)| *i != at) {
            crate::keys::register(crate::keys::Hotkey::command(
                &format!("Go to {n}"),
                Act::new(move |_: &mut ()| open.set(i)),
            ));
        }
        crate::keys::register(crate::keys::Hotkey::command(
            if dark {
                "Use light theme"
            } else {
                "Use dark theme"
            },
            Act::new(move |_: &mut ()| theme.set(!dark)),
        ));
        let body = cx::scope(names[at], || (self.pages[at].1)());
        crate::view::row([
            side.surface().w(170.0).fill_h().into(),
            sidebar_edge(),
            body,
        ])
        .gap(0.0)
        .fill()
    }
}
