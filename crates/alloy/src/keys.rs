//! Shortcuts, toasts and undo: the per-thread registry that views and handlers write to and the
//! shell reads from.

use crate::{desktop::DesktopSpec, dsl::Act, view::View};
use std::{any::Any, cell::RefCell};

/// A keyboard shortcut and the message it sends. See [`App::shortcuts`](crate::App::shortcuts).
pub struct Hotkey<M> {
    pub(crate) keys: String,
    pub(crate) label: Option<String>,
    /// Name the desktop uses to ask for this command (menu click, tray, global hotkey).
    /// Defaults to `cmd:<label>` for a labelled command.
    pub(crate) id: Option<String>,
    pub(crate) msg: M,
    /// Fire even while a text field has focus (plain keys otherwise stay with the field).
    pub(crate) typing: bool,
}

impl<M> Hotkey<M> {
    /// `keys` is like `"mod+s"`, `"mod+shift+z"`, `"ctrl+enter"` or `"f5"`; `mod` is Cmd on macOS
    /// and Ctrl elsewhere.
    pub fn new(keys: &str, msg: M) -> Self {
        Self {
            keys: keys.into(),
            label: None,
            id: None,
            msg,
            typing: false,
        }
    }
    /// A command only the desktop can call (a tray item, a global hotkey): no key, not in the palette.
    pub(crate) fn hidden(id: &str, msg: M) -> Self {
        Self {
            keys: String::new(),
            label: None,
            id: Some(id.into()),
            msg,
            typing: false,
        }
    }
    /// A command with no key: it only appears in the command palette (`mod+k`).
    pub fn command(label: &str, msg: M) -> Self {
        Self {
            keys: String::new(),
            label: Some(label.into()),
            id: None,
            msg,
            typing: false,
        }
    }
    /// Fire this shortcut even while a text field has focus. By default a shortcut made of a
    /// plain key (`"n"`, `"?"`, `"shift+a"`) stays quiet then, so typing is never stolen;
    /// shortcuts with Ctrl, Cmd or Alt, and keys like arrows, Enter and Escape, always fire.
    pub fn while_typing(mut self) -> Self {
        self.typing = true;
        self
    }
    /// Also list the shortcut under this name in the native menu bar and the palette.
    pub fn labelled(mut self, label: &str) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// What the history shortcuts ask the shell to do after the current message.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Meta {
    /// Step back one change.
    Undo,
    /// Step forward again.
    Redo,
}

/// A copy of the app's state, for undo. Made by [`Snap::new`].
pub struct Snap(Box<dyn Dyn>);

trait Dyn {
    fn same(&self, other: &dyn Any) -> bool;
    fn any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}
struct Held<T>(T);
impl<T: PartialEq + 'static> Dyn for Held<T> {
    fn same(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| *o == self.0)
    }
    fn any(&self) -> &dyn Any {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        Box::new(self.0)
    }
}

impl Snap {
    /// Wrap a copy of the state. Equal snapshots are not recorded as separate undo steps.
    pub fn new<T: PartialEq + 'static>(v: T) -> Self {
        Snap(Box::new(Held(v)))
    }
    /// The state back out, if it is a `T`.
    pub fn take<T: 'static>(self) -> Option<T> {
        self.0.into_any().downcast::<T>().ok().map(|b| *b)
    }
    pub(crate) fn same(&self, o: &Snap) -> bool {
        self.0.same(o.0.any())
    }
}

pub(crate) struct ToastItem {
    pub id: u64,
    pub text: String,
    pub undo: bool,
    pub error: bool,
    /// 0 plain, 1 success, 2 warning, 3 info: a coloured dot before the text.
    pub tone: u8,
    pub secs: f32,
    /// Tick time when it first showed; `None` until the next tick.
    pub born: Option<u64>,
}

/// A shortcut registered from a view: keys, label, desktop id and the boxed message.
type Registered = (String, Option<String>, Option<String>, Box<dyn Any>, bool);

#[derive(Default)]
struct Reg {
    keys: Vec<Registered>,
    meta: Option<Meta>,
    toasts: Vec<ToastItem>,
    /// Pixels under the stack and how many show at once; set by the app each view build.
    toast_inset: Option<f32>,
    toast_max: Option<usize>,
    next_toast: u64,
    now: u64,
    menu: Vec<(String, String)>,
    menu_sent: Vec<(String, String)>,
    undoable: bool,
    /// Desktop commands waiting for the page's shortcut layer to run them.
    invoked: Vec<String>,
    desktop: DesktopSpec,
    desktop_sent: Option<DesktopSpec>,
    quit: bool,
    /// The window is hidden (by `hide_window`, until `show_window`).
    window_hidden: bool,
    /// A hide (`true`) or show (`false`) the shell has yet to apply.
    window_hide_req: Option<bool>,
}

thread_local! {
    static REG: RefCell<Reg> = RefCell::new(Reg::default());
}

fn reg<R>(f: impl FnOnce(&mut Reg) -> R) -> R {
    REG.with(|r| f(&mut r.borrow_mut()))
}

/// Remember a shortcut registered while building the view.
pub(crate) fn register<M: 'static>(h: Hotkey<M>) {
    reg(|r| {
        r.keys
            .push((h.keys, h.label, h.id, Box::new(h.msg), h.typing))
    });
}

/// The shortcuts registered since the last call, of message type `M`.
pub(crate) fn take_registered<M: 'static>() -> Vec<Hotkey<M>> {
    reg(|r| std::mem::take(&mut r.keys))
        .into_iter()
        .filter_map(|(keys, label, id, m, typing)| {
            m.downcast::<M>().ok().map(|m| Hotkey {
                keys,
                label,
                id,
                msg: *m,
                typing,
            })
        })
        .collect()
}

pub(crate) fn publish_menu(items: Vec<(String, String)>, undoable: bool) {
    reg(|r| {
        r.menu = items;
        r.undoable = undoable;
    });
}

/// Ask the page's shortcut layer to run the command named `id` on its next event.
pub(crate) fn invoke(id: String) {
    reg(|r| r.invoked.push(id));
}

/// The commands the desktop asked for since the last call.
pub(crate) fn take_invoked() -> Vec<String> {
    reg(|r| std::mem::take(&mut r.invoked))
}

pub(crate) fn publish_desktop(spec: DesktopSpec) {
    reg(|r| r.desktop = spec);
}

/// The desktop spec the last view build published.
pub(crate) fn current_desktop() -> DesktopSpec {
    reg(|r| r.desktop.clone())
}

/// Close every window and exit the app. Call it from a handler, such as a menu's Quit item:
/// `.menu("File", |m| m.item("Quit", "mod+q", |_| quit()))`.
pub fn quit() {
    reg(|r| r.quit = true);
}

/// Hide the window without closing it; the app keeps running (in the tray, say). Call it from
/// any handler or from [`Closing::hide`](crate::Closing::hide). Bring the window back with
/// [`show_window`].
pub fn hide_window() {
    reg(|r| {
        r.window_hidden = true;
        r.window_hide_req = Some(true);
    });
}

/// Show the window again and focus it. Callable from tray, menu and global-hotkey handlers:
/// `.tray("App", |t| t.click(|_| show_window()))`.
pub fn show_window() {
    reg(|r| {
        r.window_hidden = false;
        r.window_hide_req = Some(false);
    });
}

/// True after [`hide_window`] until [`show_window`].
pub fn window_hidden() -> bool {
    reg(|r| r.window_hidden)
}

/// The hide/show the shell has yet to apply: `Some(true)` visible wanted.
pub(crate) fn take_window_visibility() -> Option<bool> {
    reg(|r| r.window_hide_req.take().map(|hide| !hide))
}

pub(crate) fn take_quit() -> bool {
    reg(|r| std::mem::take(&mut r.quit))
}

/// Whether the running app has undo, so a block can skip its "are you sure?" and offer a toast.
pub(crate) fn undoable() -> bool {
    reg(|r| r.undoable)
}

/// The labelled shortcuts and the desktop spec if either changed since the last call.
pub(crate) fn shell_changed() -> Option<(Vec<(String, String)>, DesktopSpec)> {
    reg(|r| {
        let same = r.menu == r.menu_sent && r.desktop_sent.as_ref() == Some(&r.desktop);
        (!same).then(|| {
            r.menu_sent = r.menu.clone();
            r.desktop_sent = Some(r.desktop.clone());
            (r.menu.clone(), r.desktop.clone())
        })
    })
}

/// Undo the last change to the app's state (what `mod+z` does). Call it from a handler:
/// `button("Undo").act(|_| undo())`. Needs [`Sketch::undoable`](crate::Sketch::undoable) or
/// [`Pages::undoable`](crate::Pages::undoable).
pub fn undo() {
    reg(|r| r.meta = Some(Meta::Undo));
}

/// Redo what [`undo`] undid (`mod+shift+z`).
pub fn redo() {
    reg(|r| r.meta = Some(Meta::Redo));
}

pub(crate) fn take_meta() -> Option<Meta> {
    reg(|r| r.meta.take())
}

pub(crate) fn meta_act<S: 'static>(m: Meta) -> Act<S> {
    Act::new(move |_: &mut S| reg(|r| r.meta = Some(m)))
}

// ---------------------------------------------------------------- toasts

/// A short message that fades in at the bottom of the window and goes away by itself:
/// `toast("Saved")`, or `toast("Note deleted").undo()` with an Undo button that steps the
/// app back (see [`Sketch::undoable`](crate::Sketch::undoable)). Call it from any handler.
pub fn toast(text: impl Into<String>) -> Toast {
    reg(|r| {
        let text = text.into();
        // The same message again replaces itself instead of stacking.
        r.toasts.retain(|t| t.text != text);
        r.next_toast += 1;
        let id = r.next_toast;
        r.toasts.push(ToastItem {
            id,
            text,
            undo: false,
            error: false,
            tone: 0,
            secs: 3.0,
            born: None,
        });
        // Keep a burst of toasts from piling up the screen.
        let max = r.toast_max.unwrap_or(4);
        while r.toasts.len() > max {
            r.toasts.remove(0);
        }
        Toast(id)
    })
}

/// The handle [`toast`] returns, to adjust the toast just shown.
#[derive(Clone, Copy)]
pub struct Toast(u64);

impl Toast {
    fn edit(self, f: impl FnOnce(&mut ToastItem)) -> Self {
        reg(|r| {
            if let Some(t) = r.toasts.iter_mut().find(|t| t.id == self.0) {
                f(t)
            }
        });
        self
    }
    /// Add an Undo button (stays 6 s unless [`secs`](Self::secs) says otherwise).
    pub fn undo(self) -> Self {
        self.edit(|t| {
            t.undo = true;
            t.secs = t.secs.max(6.0)
        })
    }
    /// Show for this long.
    pub fn secs(self, secs: f32) -> Self {
        self.edit(|t| t.secs = secs)
    }
    /// A green dot before the text: something worked.
    pub fn success(self) -> Self {
        self.edit(|t| t.tone = 1)
    }
    /// An amber dot: it worked, but look at this.
    pub fn warning(self) -> Self {
        self.edit(|t| t.tone = 2)
    }
    /// An accent dot: neutral news.
    pub fn info(self) -> Self {
        self.edit(|t| t.tone = 3)
    }
    /// Style as a failure.
    pub fn error(self) -> Self {
        self.edit(|t| t.error = true)
    }
}

pub(crate) fn toasts<R>(f: impl FnOnce(&mut Vec<ToastItem>, u64) -> R) -> R {
    reg(|r| f(&mut r.toasts, r.now))
}

pub(crate) fn publish_toast_layout((inset, max): (f32, usize)) {
    reg(|r| {
        r.toast_inset = Some(inset);
        r.toast_max = Some(max);
        while r.toasts.len() > max {
            r.toasts.remove(0);
        }
    });
}

/// Pixels kept free under the toast stack.
pub(crate) fn toast_inset() -> f32 {
    reg(|r| r.toast_inset.unwrap_or(24.0))
}

pub(crate) fn set_now(ms: u64) {
    reg(|r| r.now = ms);
}

/// Forget the toasts and the clock of an earlier window or `Harness` on this thread.
pub(crate) fn reset_toasts() {
    reg(|r| {
        r.toasts.clear();
        r.now = 0;
    });
}

// -------------------------------------------------------- view extension

/// Register a shortcut from inside a view: `page.shortcut("mod+n", |s| s.new_note())`.
pub trait Shortcuts<S: 'static>: View<Act<S>> + Sized {
    /// Run `f` on the state when `keys` is pressed anywhere in the window (while this view is
    /// shown). `mod` is Cmd on macOS and Ctrl elsewhere.
    fn shortcut(self, keys: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        register(Hotkey::new(keys, Act::new(f)));
        self
    }
    /// Like [`shortcut`](Self::shortcut), and it fires even while a text field has focus (see
    /// [`Hotkey::while_typing`]).
    fn shortcut_typing(self, keys: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        register(Hotkey::new(keys, Act::new(f)).while_typing());
        self
    }
    /// Add `label` to the command palette (`mod+k`) while this view is shown:
    /// `page.command("Export CSV", |s| s.export())`.
    fn command(self, label: &str, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        register(Hotkey::command(label, Act::new(f)));
        self
    }
    /// Like [`shortcut`](Self::shortcut), and listed in the menu bar as `label`.
    fn shortcut_as(
        self,
        keys: &str,
        label: &str,
        f: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        register(Hotkey::new(keys, Act::new(f)).labelled(label));
        self
    }
}
impl<S: 'static, V: View<Act<S>>> Shortcuts<S> for V {}
