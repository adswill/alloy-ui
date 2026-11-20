//! Conformance harness for Alloy's text fields.
//!
//! A [`Field`] hosts a real `TextInput` or `TextEditor` inside the real
//! `Runtime` (view rebuilds, layout, `Root`, focus), and is driven only with
//! synthetic platform [`Event`]s, exactly what the shell would deliver. Caret
//! and selection are observed black-box ([`Field::caret_marked`],
//! [`Field::selection`]) so the suite keeps working when the widgets' internals
//! change.
//!
//! Tests for behaviour that is known to be wrong are marked
//! `#[ignore = "BUG(owner): ..."]`; CI runs them in a separate, non-blocking
//! step so the failing list stays visible.
#![allow(missing_docs)]

use alloy_ui_core::{
    Application, Command, Element, Event, Key, Modifiers, MouseButton, Point, Runtime, Size,
};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::{layout, Clipboard, MemoryClipboard, Root, TextEditor, TextInput, Ui};
use std::rc::Rc;

pub use unicode_segmentation::UnicodeSegmentation;

/// Marker used to locate the caret (a character no test text contains).
pub const CARET: &str = "\u{2038}"; // ‸

/// Which widget to test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Input,
    Editor,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Input, Kind::Editor];
}

#[derive(Debug)]
pub enum Msg {
    Input(String),
    Submit,
}

struct App {
    ui: Ui,
    kind: Kind,
    editable: bool,
    text: String,
    submits: u32,
    edits: u32,
}

impl Application for App {
    type Message = Msg;
    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Input(s) => {
                self.text = s;
                self.edits += 1;
            }
            Msg::Submit => self.submits += 1,
        }
        Command::None
    }
    fn view(&self) -> Element<Msg> {
        let child = match self.kind {
            Kind::Input => {
                let mut t =
                    TextInput::new(&self.ui, 1, self.text.clone()).on_submit(|| Msg::Submit);
                if self.editable {
                    t = t.on_input(Msg::Input);
                }
                t.into_element()
            }
            Kind::Editor => {
                let mut t = TextEditor::new(&self.ui, 1, self.text.clone());
                if self.editable {
                    t = t.on_input(Msg::Input);
                }
                t.into_element()
            }
        };
        Root::new(&self.ui, child).into_element()
    }
}

/// Platform-correct shortcut modifiers.
#[derive(Debug, Clone, Copy)]
pub struct Plat {
    pub mac: bool,
    /// Copy/paste/undo/select-all modifier: Cmd on macOS, Ctrl elsewhere.
    pub primary: Modifiers,
    /// Word-wise movement and deletion: Option on macOS, Ctrl elsewhere.
    pub word: Modifiers,
}

pub fn mods(shift: bool, ctrl: bool, alt: bool, logo: bool) -> Modifiers {
    Modifiers {
        shift,
        ctrl,
        alt,
        logo,
    }
}
pub const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
    logo: false,
};
pub fn shift(m: Modifiers) -> Modifiers {
    Modifiers { shift: true, ..m }
}
pub const CTRL: Modifiers = Modifiers { ctrl: true, ..NONE };
pub const ALT: Modifiers = Modifiers { alt: true, ..NONE };
pub const CMD: Modifiers = Modifiers { logo: true, ..NONE };
pub const SHIFT: Modifiers = Modifiers {
    shift: true,
    ..NONE
};

impl Plat {
    pub const MAC: Plat = Plat {
        mac: true,
        primary: CMD,
        word: ALT,
    };
    pub const OTHER: Plat = Plat {
        mac: false,
        primary: CTRL,
        word: CTRL,
    };
    /// The convention of the OS this test binary runs on.
    pub fn native() -> Plat {
        if cfg!(target_os = "macos") {
            Plat::MAC
        } else {
            Plat::OTHER
        }
    }
}

pub fn ch(c: &str) -> Key {
    Key::Character(c.into())
}
/// A non-character key by name; the shell reports Tab/Enter/Escape/Backspace
/// as dedicated variants and everything else (arrows, Home, Delete...) as `Named`.
pub fn named(n: &str) -> Key {
    match n {
        "Tab" => Key::Tab,
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Backspace" => Key::Backspace,
        _ => Key::Named(n.into()),
    }
}

/// A text field running inside a real `Runtime`.
pub struct Field {
    rt: Runtime<App>,
    ui: Ui,
    clip: Rc<MemoryClipboard>,
    pub kind: Kind,
    now: u64,
}

const WINDOW: Size = Size {
    width: 400.0,
    height: 200.0,
};

impl Field {
    pub fn new(kind: Kind) -> Self {
        Self::build(kind, true)
    }
    pub fn read_only(kind: Kind) -> Self {
        Self::build(kind, false)
    }
    fn build(kind: Kind, editable: bool) -> Self {
        let ui = Ui::new(Theme::default());
        let clip = Rc::new(MemoryClipboard::default());
        ui.set_clipboard(clip.clone());
        let app = App {
            ui: ui.clone(),
            kind,
            editable,
            text: String::new(),
            submits: 0,
            edits: 0,
        };
        let rt = Runtime::new(app, WINDOW, layout::<Msg>);
        Self {
            rt,
            ui,
            clip,
            kind,
            now: 0,
        }
    }

    /// Start with `text` already in the field and the caret at its end, focused.
    pub fn with_text(mut self, text: &str) -> Self {
        self.set_value(text);
        self.focus_end();
        self
    }
    /// Replace the application's text without any widget event (no undo entry).
    pub fn set_value(&mut self, text: &str) {
        self.rt.dispatch(Msg::Input(text.to_owned()));
    }

    pub fn value(&self) -> &str {
        &self.rt.app().text
    }
    pub fn submits(&self) -> u32 {
        self.rt.app().submits
    }
    /// How many times the widget reported an edit.
    pub fn edits(&self) -> u32 {
        self.rt.app().edits
    }
    pub fn focused(&self) -> bool {
        self.ui.focused().is_some()
    }
    pub fn clipboard(&self) -> Option<String> {
        self.clip.get()
    }
    pub fn set_clipboard(&self, s: &str) {
        self.clip.set(s.to_owned());
    }

    // --- raw event injection ------------------------------------------------

    pub fn send(&mut self, e: Event) {
        self.rt.handle_event(&e);
    }
    pub fn click(&mut self, x: f32, y: f32) {
        self.send(Event::CursorMoved(Point::new(x, y)));
        self.send(Event::MousePressed(MouseButton::Left));
        self.send(Event::MouseReleased(MouseButton::Left));
    }
    pub fn drag(&mut self, from: (f32, f32), to: (f32, f32)) {
        self.send(Event::CursorMoved(Point::new(from.0, from.1)));
        self.send(Event::MousePressed(MouseButton::Left));
        self.send(Event::CursorMoved(Point::new(to.0, to.1)));
        self.send(Event::MouseReleased(MouseButton::Left));
    }
    /// Click far right and low inside the field: caret to the end of the text.
    pub fn focus_end(&mut self) {
        self.click(WINDOW.width - 4.0, 4.0);
        // The editor fills the window, so a click low in it lands on the last line.
        if self.kind == Kind::Editor {
            self.click(WINDOW.width - 4.0, WINDOW.height - 4.0);
        }
    }
    /// X coordinate of the boundary after `chars` characters of the first line
    /// (uses the widgets' own metrics, so it holds for any metrics installed).
    pub fn x_after(&self, prefix: &str) -> f32 {
        let t = self.ui.theme();
        let (pad, size) = match self.kind {
            Kind::Input => (t.spacing.md, t.typography.body.size),
            Kind::Editor => (t.spacing.md, t.typography.code.size),
        };
        pad + self.ui.metrics().width(prefix, size)
    }
    pub fn tick(&mut self, ms: u64) {
        self.now += ms;
        self.send(Event::Tick(self.now));
    }

    // --- keyboard -----------------------------------------------------------

    pub fn key(&mut self, k: Key, m: Modifiers) {
        self.send(Event::KeyPressed(k.clone(), m));
        self.send(Event::KeyReleased(k, m));
    }
    pub fn named(&mut self, name: &str, m: Modifiers) {
        self.key(named(name), m);
    }
    pub fn chord(&mut self, c: &str, m: Modifiers) {
        self.key(ch(c), m);
    }
    /// Type text the way a platform delivers it: per character, a key press
    /// followed by committed text.
    pub fn type_str(&mut self, s: &str) {
        for c in s.chars() {
            let t = c.to_string();
            self.send(Event::KeyPressed(Key::Character(t.clone()), NONE));
            self.send(Event::TextInput(t.clone()));
            self.send(Event::KeyReleased(Key::Character(t), NONE));
        }
    }
    /// An IME preedit update.
    pub fn preedit(&mut self, s: &str) {
        self.send(Event::ImePreedit(s.to_owned()));
    }
    /// An IME commit, in the order winit delivers it: preedit cleared, then text.
    pub fn commit(&mut self, s: &str) {
        self.send(Event::ImePreedit(String::new()));
        self.send(Event::TextInput(s.to_owned()));
    }

    // --- observation --------------------------------------------------------

    /// The text currently selected, or `None` when nothing is selected.
    /// Observed by copying: the clipboard is restored afterwards.
    pub fn selection(&mut self, plat: Plat) -> Option<String> {
        let saved = self.clip.get();
        let sentinel = "\u{0}no-selection\u{0}";
        self.clip.set(sentinel.to_owned());
        self.chord("c", plat.primary);
        let got = self.clip.get();
        match saved {
            Some(s) => self.clip.set(s),
            None => self.clip.set(String::new()),
        }
        got.filter(|s| s != sentinel)
    }
    /// The value with [`CARET`] inserted where the caret is. Only valid when
    /// nothing is selected (a selection would be replaced). The probe is
    /// removed again before returning.
    pub fn caret_marked(&mut self) -> String {
        let before = self.value().to_owned();
        self.type_str(CARET);
        let marked = self.value().to_owned();
        self.named("Backspace", NONE);
        assert_eq!(
            self.value(),
            before,
            "caret probe could not be removed again"
        );
        marked
    }
}

/// Run `f` for both widget kinds, naming the kind in any panic.
pub fn both(mut f: impl FnMut(Kind)) {
    for k in Kind::ALL {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(k)));
        if let Err(e) = r {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            panic!("[{k:?}] {msg}");
        }
    }
}

/// Collect named failures from a table of cases and fail once with all of them.
pub struct Cases(Vec<String>);
impl Cases {
    pub fn new() -> Self {
        Cases(Vec::new())
    }
    pub fn check<T: std::fmt::Debug + PartialEq>(&mut self, name: &str, got: T, want: T) {
        if got != want {
            self.0.push(format!("{name}: got {got:?}, want {want:?}"));
        }
    }
    pub fn finish(self) {
        assert!(self.0.is_empty(), "\n  {}", self.0.join("\n  "));
    }
}
impl Default for Cases {
    fn default() -> Self {
        Self::new()
    }
}

/// Text samples used across the suite.
pub mod samples {
    pub const FAMILY: &str = "👨\u{200d}👩\u{200d}👧";
    pub const FLAG: &str = "🇯🇵";
    pub const THUMB_TONE: &str = "👍🏽";
    pub const KEYCAP: &str = "1\u{fe0f}\u{20e3}";
    pub const E_ACUTE_COMBINING: &str = "e\u{301}";
    pub const A_TWO_MARKS: &str = "a\u{301}\u{323}";
    pub const HANGUL_JAMO: &str = "\u{1112}\u{1161}\u{11ab}"; // 한 decomposed
    pub const DEVANAGARI: &str = "क्षि";
    pub const SIMPLE_EMOJI: &str = "😀";
    pub const CLUSTERS: [(&str, &str); 8] = [
        ("family ZWJ", FAMILY),
        ("flag", FLAG),
        ("skin tone", THUMB_TONE),
        ("keycap", KEYCAP),
        ("e + combining acute", E_ACUTE_COMBINING),
        ("a + two marks", A_TWO_MARKS),
        ("hangul jamo", HANGUL_JAMO),
        ("devanagari conjunct", DEVANAGARI),
    ];
}
