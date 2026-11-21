//! Workbench widgets: split panes, context menus, markdown, huge lists, tab bars, popovers.
//!
//! ```ignore
//! split(file_tree, editor).ratio(0.25)                       // drag the divider
//! context_menu(row, [menu_item("Rename").act(|s: &mut S| s.rename()), menu_rule(),
//!                    menu_item("Delete").danger().act(|s: &mut S| s.delete())])
//! markdown(include_str!("notes.md"))
//! vlist(100_000, |i| text(format!("Row {i}"))).act(|s: &mut S, i| s.open = i)
//! tab_bar(&s.names, at!(s.tab)).close(|s: &mut S, i| s.names.remove(i));
//! popover(button("Filters"), col![check("Only mine", at!(s.mine))])
//! ```

use crate::{
    bind::Val,
    cx::{self, auto_id},
    local,
    view::Node,
    Act, Cell,
};
use alloy_ui_core::Element;
use alloy_ui_widgets as w;
use std::{rc::Rc, sync::Arc};

// ---------------------------------------------------------------- split

/// A resizable two-pane split under construction. See [`split`].
pub struct Split<M> {
    a: Element<M>,
    b: Element<M>,
    vertical: bool,
    ratio: f32,
    min: f32,
    id: u64,
}

/// Two panes with a draggable divider (also keyboard-resizable). Side by side by default.
#[track_caller]
pub fn split<M: 'static>(first: impl Into<Node<M>>, second: impl Into<Node<M>>) -> Split<M> {
    Split {
        a: first.into().into_element(),
        b: second.into().into_element(),
        vertical: false,
        ratio: 0.5,
        min: 120.0,
        id: auto_id(51),
    }
}

impl<M: 'static> Split<M> {
    /// Stack the panes (first on top).
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }
    /// Where the divider starts, as the first pane's share (0.0 to 1.0).
    pub fn ratio(mut self, r: f32) -> Self {
        self.ratio = r;
        self
    }
    /// Smallest size in pixels of either pane.
    pub fn min(mut self, px: f32) -> Self {
        self.min = px;
        self
    }
}
impl<M: 'static> From<Split<M>> for Node<M> {
    fn from(s: Split<M>) -> Self {
        let mut e = w::SplitPane::new(&cx::ui(), s.id, s.a, s.b)
            .ratio(s.ratio)
            .min(s.min);
        if s.vertical {
            e = e.vertical();
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- context menu

type Run<S> = Arc<dyn Fn(&mut S) + Send + Sync>;

/// One command of a [`context_menu`]. See [`menu_item`].
pub struct MenuEntry<S> {
    item: w::MenuItem,
    run: Option<Run<S>>,
}

/// A command: `menu_item("Copy").key("Ctrl+C").act(|s: &mut App| s.copy())`.
pub fn menu_item<S>(label: impl Into<String>) -> MenuEntry<S> {
    MenuEntry {
        item: w::MenuItem::new(label),
        run: None,
    }
}
/// A divider line between groups of commands.
pub fn menu_rule<S>() -> MenuEntry<S> {
    MenuEntry {
        item: w::MenuItem::separator(),
        run: None,
    }
}

impl<S: 'static> MenuEntry<S> {
    /// Shortcut hint shown dim on the right (display only; bind the key with `Shortcuts`).
    pub fn key(mut self, hint: impl Into<String>) -> Self {
        self.item = self.item.shortcut(hint);
        self
    }
    /// Grey it out when `yes`.
    pub fn disabled(mut self, yes: bool) -> Self {
        self.item = self.item.disabled(yes);
        self
    }
    /// Draw it as destructive.
    pub fn danger(mut self) -> Self {
        self.item = self.item.danger();
        self
    }
    /// What it does.
    pub fn act(mut self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.run = Some(Arc::new(f));
        self
    }
}

/// A view with a right-click menu. See [`context_menu`].
pub struct Menu<S> {
    child: Element<Act<S>>,
    entries: Vec<MenuEntry<S>>,
    id: u64,
}

/// Open a menu of commands on right click (or the menu key / Shift+F10 over `child`). Up, Down
/// and Enter work inside it, Escape closes.
#[track_caller]
pub fn context_menu<S: 'static>(
    child: impl Into<Node<Act<S>>>,
    entries: impl IntoIterator<Item = MenuEntry<S>>,
) -> Menu<S> {
    Menu {
        child: child.into().into_element(),
        entries: entries.into_iter().collect(),
        id: auto_id(52),
    }
}
impl<S: 'static> From<Menu<S>> for Node<Act<S>> {
    fn from(m: Menu<S>) -> Self {
        let items: Vec<_> = m.entries.iter().map(|e| e.item.clone()).collect();
        let runs: Vec<Option<Run<S>>> = m.entries.into_iter().map(|e| e.run).collect();
        let e = w::ContextMenu::new(&cx::ui(), m.id, m.child, items).on_select(move |i| {
            let run = runs.get(i).cloned().flatten();
            Act::new(move |s: &mut S| {
                if let Some(f) = &run {
                    f(s);
                }
            })
        });
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- markdown

type LinkFn<S> = Arc<dyn Fn(&mut S, String) + Send + Sync>;

/// A Markdown view under construction. See [`markdown`].
pub struct Markdown<S> {
    src: String,
    width: f32,
    on: Option<LinkFn<S>>,
}

/// Rendered Markdown: headings, paragraphs, **bold**, `code`, links, lists, quotes, code blocks.
/// Wraps at 640 px unless `.width(px)` says otherwise. Not supported: tables, images, HTML.
pub fn markdown<S>(src: impl Into<String>) -> Markdown<S> {
    Markdown {
        src: src.into(),
        width: 640.0,
        on: None,
    }
}
impl<S: 'static> Markdown<S> {
    /// Wrap width in pixels.
    pub fn width(mut self, px: f32) -> Self {
        self.width = px;
        self
    }
    /// Called with the target of a clicked link.
    pub fn link(mut self, f: impl Fn(&mut S, String) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}
impl<S: 'static> From<Markdown<S>> for Node<Act<S>> {
    fn from(m: Markdown<S>) -> Self {
        let mut e = w::Markdown::new(&cx::ui(), &m.src).width(m.width);
        if let Some(f) = m.on {
            e = e.on_link(move |url| {
                let f = f.clone();
                Act::new(move |s: &mut S| f(s, url.clone()))
            });
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- virtual list

/// A virtualised list under construction. See [`vlist`].
pub struct VList<S> {
    count: usize,
    build: Rc<dyn Fn(usize) -> Element<Act<S>>>,
    row_h: Option<f32>,
    ruled: bool,
    selected: Option<Option<usize>>,
    on: Option<Run2<S>>,
    id: u64,
}
type Run2<S> = Arc<dyn Fn(&mut S, usize) + Send + Sync>;

/// A list of `count` rows that only builds what is on screen, so 100 000 rows scroll as
/// smoothly as 20. `row(i)` builds row `i`. It remembers the selected row itself; react with
/// `.act(|s, i| ..)` or pin it with `.selected(..)`. Click, Up/Down, PageUp/PageDown, Home/End,
/// wheel and a draggable scrollbar all work.
#[track_caller]
pub fn vlist<S: 'static, N: Into<Node<Act<S>>>>(
    count: usize,
    row: impl Fn(usize) -> N + 'static,
) -> VList<S> {
    // Rows are built at draw time, after `view` returned, so each gets its own context.
    let ui = cx::ui();
    VList {
        count,
        build: Rc::new(move |i| cx::scoped(&ui, i as u64, || row(i).into().into_element())),
        row_h: None,
        ruled: false,
        selected: None,
        on: None,
        id: auto_id(53),
    }
}
impl<S: 'static> VList<S> {
    /// Row height in pixels.
    pub fn row_height(mut self, px: f32) -> Self {
        self.row_h = Some(px);
        self
    }
    /// A hairline between rows.
    pub fn ruled(mut self) -> Self {
        self.ruled = true;
        self
    }
    /// Control the selection yourself.
    pub fn selected(mut self, i: Option<usize>) -> Self {
        self.selected = Some(i);
        self
    }
    /// Called with the index of a picked row.
    pub fn act(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}
impl<S: 'static> From<VList<S>> for Node<Act<S>> {
    fn from(v: VList<S>) -> Self {
        let picked = local(None::<usize>);
        let sel = v.selected.unwrap_or_else(|| picked.get());
        let on = v.on;
        let build = v.build;
        let mut e = w::VirtualList::new(&cx::ui(), v.id, v.count, move |i| build(i))
            .selected(sel)
            .on_select(move |i| {
                let on = on.clone();
                Act::new(move |s: &mut S| {
                    picked.set(Some(i));
                    if let Some(f) = &on {
                        f(s, i);
                    }
                })
            });
        if let Some(h) = v.row_h {
            e = e.row_height(h);
        }
        if v.ruled {
            e = e.ruled();
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- tab bar

type Setter<T, M> = Box<dyn Fn(T) -> M>;

/// A tab strip under construction. See [`tab_bar`].
pub struct TabBar<M> {
    labels: Vec<String>,
    selected: usize,
    on: Option<Setter<usize, M>>,
    close: Option<Setter<usize, M>>,
    id: u64,
}

/// Tabs with overflow: labels that do not fit collapse into a `+N` menu, and the selected tab
/// stays visible. `selected` is a plain index (read-only until `.on(..)`) or `at!(s.tab)`.
/// It draws only the strip; show the selected tab's content yourself. (`tabs(..)` is the
/// compact segmented control.)
#[track_caller]
pub fn tab_bar<M, L: ToString>(
    labels: impl IntoIterator<Item = L>,
    selected: impl Val<usize, M>,
) -> TabBar<M> {
    let (selected, on) = selected.split();
    TabBar {
        labels: labels.into_iter().map(|l| l.to_string()).collect(),
        selected,
        on,
        close: None,
        id: auto_id(54),
    }
}
impl<M: 'static> TabBar<M> {
    /// Called with the picked index.
    pub fn on(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on = Some(Box::new(f));
        self
    }
    /// Give each tab a close button (middle click too); called with the index to close.
    pub fn on_close(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.close = Some(Box::new(f));
        self
    }
}
impl<S: 'static> TabBar<Act<S>> {
    /// Closure style: `.act(|state, index| ..)`.
    pub fn act(self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        let f = Arc::new(f);
        self.on(move |i| {
            let f = f.clone();
            Act::new(move |s: &mut S| f(s, i))
        })
    }
    /// Closure style close button: `.close(|state, index| state.docs.remove(index))`.
    pub fn close(self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        let f = Arc::new(f);
        self.on_close(move |i| {
            let f = f.clone();
            Act::new(move |s: &mut S| f(s, i))
        })
    }
    /// Write the picked index into a view-local cell.
    pub fn local(self, cell: &impl Cell<usize>) -> Self {
        let cell = *cell;
        self.on(move |i| Act::new(move |_| cell.write(i)))
    }
}
impl<M: 'static> From<TabBar<M>> for Node<M> {
    fn from(t: TabBar<M>) -> Self {
        let mut e = w::Tabs::new(&cx::ui(), t.id, t.labels, t.selected);
        if let Some(f) = t.on {
            e = e.on_select(f);
        }
        if let Some(f) = t.close {
            e = e.on_close(f);
        }
        Node(e.into_element())
    }
}

// ---------------------------------------------------------------- popover

/// A popover under construction. See [`popover`].
pub struct Popover<S> {
    anchor: Element<Act<S>>,
    content: Element<Act<S>>,
    open: Option<bool>,
    id: u64,
}

/// A floating panel with any content, anchored to `anchor`: click the anchor to open, Escape or
/// a click outside to close. It keeps its own open state; pin it with `.open(..)`.
#[track_caller]
pub fn popover<S: 'static>(
    anchor: impl Into<Node<Act<S>>>,
    content: impl Into<Node<Act<S>>>,
) -> Popover<S> {
    Popover {
        anchor: anchor.into().into_element(),
        content: content.into().into_element(),
        open: None,
        id: auto_id(55),
    }
}
impl<S: 'static> Popover<S> {
    /// Control whether it is open yourself.
    pub fn open(mut self, yes: bool) -> Self {
        self.open = Some(yes);
        self
    }
}
impl<S: 'static> From<Popover<S>> for Node<Act<S>> {
    fn from(p: Popover<S>) -> Self {
        let state = local(false);
        let open = p.open.unwrap_or_else(|| state.get());
        let e = w::Popover::new(&cx::ui(), p.id, p.anchor, p.content)
            .open(open)
            .on_toggle(move |v| Act::new(move |_: &mut S| state.set(v)));
        Node(e.into_element())
    }
}
