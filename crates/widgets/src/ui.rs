//! Persistent UI state shared by widgets, text metrics and the clipboard seam.

use crate::{text_edit::Edit, util};
use alloy_ui_core::{DrawList, Element, Event, Key, Layout, Point, Rectangle, Size, Widget};
use alloy_ui_theme::Theme;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Measures text. The renderer will provide real metrics; until then
/// [`ApproxMetrics`] gives deterministic estimates so layout and tests work
/// without a font.
pub trait TextMetrics {
    /// Width in logical pixels of `text` (single line) at `size`.
    fn width(&self, text: &str, size: f32) -> f32;
}

/// Deterministic per-character width estimates. Not exact; never used for
/// final pixels once a renderer-backed implementation is installed.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApproxMetrics;

impl TextMetrics for ApproxMetrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| {
                let em = match c {
                    ' ' => 0.28,
                    'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '\'' | '!' | '|' => 0.28,
                    'm' | 'w' | 'M' | 'W' => 0.85,
                    c if (c as u32) >= 0x2E80 => 1.0,
                    c if c.is_uppercase() => 0.64,
                    _ => 0.54,
                };
                em * size
            })
            .sum()
    }
}

/// Clipboard seam. The platform crate supplies the real one.
pub trait Clipboard {
    /// Current text contents.
    fn get(&self) -> Option<String>;
    /// Replace contents.
    fn set(&self, text: String);
}

/// In-process clipboard used until the platform provides one (and in tests).
#[derive(Debug, Default)]
pub struct MemoryClipboard(RefCell<Option<String>>);

impl Clipboard for MemoryClipboard {
    fn get(&self) -> Option<String> {
        self.0.borrow().clone()
    }
    fn set(&self, text: String) {
        *self.0.borrow_mut() = Some(text);
    }
}

struct Inner {
    theme: Rc<Theme>,
    metrics: Rc<dyn TextMetrics>,
    clipboard: Rc<dyn Clipboard>,
    focused: Option<u64>,
    order: Vec<u64>,
    cursor: Point,
    pressed: Option<(i32, i32)>,
    scroll: HashMap<u64, f32>,
    edits: HashMap<u64, Edit>,
}

/// Cheaply clonable handle to per-window widget state.
#[derive(Clone)]
pub struct Ui(Rc<RefCell<Inner>>);

impl Ui {
    /// New state using `theme`, approximate text metrics and an in-memory clipboard.
    pub fn new(theme: Theme) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            theme: Rc::new(theme),
            metrics: Rc::new(ApproxMetrics),
            clipboard: Rc::new(MemoryClipboard::default()),
            focused: None,
            order: Vec::new(),
            cursor: Point::default(),
            pressed: None,
            scroll: HashMap::new(),
            edits: HashMap::new(),
        })))
    }

    /// The current theme.
    pub fn theme(&self) -> Rc<Theme> {
        self.0.borrow().theme.clone()
    }
    /// Swap the theme (e.g. when the system appearance changes).
    pub fn set_theme(&self, theme: Theme) {
        self.0.borrow_mut().theme = Rc::new(theme);
    }
    /// Text metrics in use.
    pub fn metrics(&self) -> Rc<dyn TextMetrics> {
        self.0.borrow().metrics.clone()
    }
    /// Install renderer-backed text metrics.
    pub fn set_metrics(&self, m: Rc<dyn TextMetrics>) {
        self.0.borrow_mut().metrics = m;
    }
    /// Clipboard in use.
    pub fn clipboard(&self) -> Rc<dyn Clipboard> {
        self.0.borrow().clipboard.clone()
    }
    /// Install the platform clipboard.
    pub fn set_clipboard(&self, c: Rc<dyn Clipboard>) {
        self.0.borrow_mut().clipboard = c;
    }

    /// Id of the focused widget, if any.
    pub fn focused(&self) -> Option<u64> {
        self.0.borrow().focused
    }
    /// Whether `id` has keyboard focus.
    pub fn is_focused(&self, id: Option<u64>) -> bool {
        id.is_some() && self.focused() == id
    }
    /// Move keyboard focus (or clear it with `None`).
    pub fn set_focus(&self, id: Option<u64>) {
        self.0.borrow_mut().focused = id;
    }
    /// Focus the next widget in draw order, wrapping around.
    pub fn focus_next(&self) {
        self.step_focus(1);
    }
    /// Focus the previous widget in draw order, wrapping around.
    pub fn focus_prev(&self) {
        self.step_focus(-1);
    }
    fn step_focus(&self, dir: i32) {
        let mut s = self.0.borrow_mut();
        let n = s.order.len() as i32;
        if n == 0 {
            return;
        }
        let cur = s
            .focused
            .and_then(|f| s.order.iter().position(|&x| x == f))
            .map(|i| i as i32);
        let next = match cur {
            Some(i) => (i + dir).rem_euclid(n),
            None if dir > 0 => 0,
            None => n - 1,
        };
        s.focused = Some(s.order[next as usize]);
    }
    /// Focusable widgets register themselves while drawing, in tab order.
    pub(crate) fn register(&self, id: u64) {
        let mut s = self.0.borrow_mut();
        if !s.order.contains(&id) {
            s.order.push(id);
        }
    }
    pub(crate) fn begin_frame(&self) {
        self.0.borrow_mut().order.clear();
    }

    /// Last known cursor position (used for hover styling).
    pub fn cursor(&self) -> Point {
        self.0.borrow().cursor
    }
    pub(crate) fn set_cursor(&self, p: Point) {
        self.0.borrow_mut().cursor = p;
    }
    pub(crate) fn pressed(&self) -> Option<(i32, i32)> {
        self.0.borrow().pressed
    }
    pub(crate) fn set_pressed(&self, key: Option<(i32, i32)>) {
        self.0.borrow_mut().pressed = key;
    }
    pub(crate) fn hovered(&self, bounds: &Rectangle) -> bool {
        bounds.contains(self.cursor())
    }
    pub(crate) fn is_pressed(&self, bounds: &Rectangle) -> bool {
        self.pressed() == Some(util::key_of(bounds))
    }

    /// Scroll offset of the scrollable `id`.
    pub fn scroll_offset(&self, id: u64) -> f32 {
        self.0.borrow().scroll.get(&id).copied().unwrap_or(0.0)
    }
    /// Set the scroll offset of `id` (clamped when next used).
    pub fn set_scroll_offset(&self, id: u64, offset: f32) {
        self.0.borrow_mut().scroll.insert(id, offset.max(0.0));
    }

    pub(crate) fn edit(&self, id: u64) -> Edit {
        self.0.borrow().edits.get(&id).cloned().unwrap_or_default()
    }
    pub(crate) fn set_edit(&self, id: u64, e: Edit) {
        self.0.borrow_mut().edits.insert(id, e);
    }
}

/// Wraps the root element: tracks the cursor, resets focus bookkeeping each
/// frame, clears focus on clicks that hit nothing focusable, and handles
/// Tab / Shift+Tab navigation.
pub struct Root<M> {
    ui: Ui,
    child: Vec<Element<M>>,
}

impl<M: 'static> Root<M> {
    /// Wrap `child` as the root of a window's tree.
    pub fn new(ui: &Ui, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            child: vec![child],
        }
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Root<M> {
    fn style(&self) -> alloy_ui_core::style::Style {
        self.child[0].0.style()
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        util::arrange_widget(self, bounds)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        match event {
            Event::CursorMoved(p) => self.ui.set_cursor(*p),
            Event::CursorLeft => self.ui.set_cursor(Point::new(-1.0e6, -1.0e6)),
            Event::MousePressed(_) => {
                self.ui.set_cursor(cursor);
                self.ui.set_focus(None);
            }
            _ => {}
        }
        let consumed = util::dispatch(&mut self.child, event, layout, cursor, out);
        match event {
            Event::MouseReleased(_) => self.ui.set_pressed(None),
            Event::KeyPressed(Key::Tab, m) if !consumed => {
                if m.shift {
                    self.ui.focus_prev();
                } else {
                    self.ui.focus_next();
                }
                return true;
            }
            _ => {}
        }
        consumed
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        self.ui.begin_frame();
        util::draw_children(&self.child, layout, out);
    }
}
