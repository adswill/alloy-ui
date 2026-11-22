//! The overlay layer: [`Portal`] and [`Modal`] draw above the whole window and take the pointer
//! while open, wherever in the tree they are written (inside a scroll area, a clipped card, a
//! list row).
//!
//! Both are leaves for layout. Their content is placed over the window (a portal) or in a
//! centred card (a modal) at draw time, appended to the overlay list that [`Root`](crate::Root)
//! paints last, so no clip or later sibling can cover it. While one is open it claims the whole
//! window as its click region: a press anywhere reaches it (even through a scroll area, which
//! forwards it) and never the widgets underneath. Popovers and menus opened inside its content
//! register their own regions later and keep working.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Border, Color, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton,
    Point, Rectangle, Size, Widget,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

thread_local! {
    /// The last card and body a dialog drew while open, replayed while it leaves when it keeps
    /// its content on exit.
    static LAST: RefCell<HashMap<u64, (Rectangle, Vec<DrawCommand>)>> = RefCell::new(HashMap::new());
}

/// Slot of the layer: set while open; the focus to hand back on close lives beside it.
const K_FOCUS_ID: u64 = 0x0000_00F0_0000_0000;
const K_PREV: u64 = 0x0E01_0000_0000;
/// Transition of the enter and exit animation.
/// Whether the content has a popover or menu open (it gets Escape first).
const K_INNER: u64 = 0x0E03_0000_0000;
/// Set once the enter animation has been seeded for this layer.
const K_SEED: u64 = 0x0E04_0000_0000;
/// Transition of the enter and exit animation.
const K_ANIM: u64 = 0x0E02_0000_0000;
/// Pixels the card travels while entering and leaving.
const RISE: f32 = 14.0;
/// Gap kept between a card and the window edge.
const MARGIN: f32 = 24.0;

/// How a layer presents its content.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Content fills the window, nothing is drawn behind it.
    Plain,
    /// Scrim, centred card, Escape, trapped focus, enter and exit animation.
    Dialog,
}

struct Layer<M> {
    ui: Ui,
    id: u64,
    mode: Mode,
    open: bool,
    width: f32,
    scrim_close: bool,
    on_close: Option<Rc<dyn Fn() -> M>>,
    max_height: Option<f32>,
    keep: bool,
    default_action: Option<Rc<dyn Fn() -> M>>,
    content: Element<M>,
}

impl<M: 'static> Layer<M> {
    fn new(ui: &Ui, id: u64, mode: Mode, content: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            mode,
            open: false,
            width: 420.0,
            scrim_close: true,
            on_close: None,
            max_height: None,
            keep: false,
            default_action: None,
            content,
        }
    }

    /// 0 to 1: how far the enter animation has got (reduced motion jumps).
    fn shown(&self) -> f32 {
        if self.mode == Mode::Plain {
            return if self.open { 1.0 } else { 0.0 };
        }
        let t = self.ui.theme();
        // A dialog built only while open has no animation yet: start it from closed.
        if self.open && self.ui.slot(self.id ^ K_SEED).is_none() {
            self.ui.set_slot(self.id ^ K_SEED, Some(1));
            self.ui.ease_with(
                self.id ^ K_ANIM,
                0.0,
                t.motion.normal_ms as u64,
                t.motion.easing,
            );
        }
        self.ui.ease_with(
            self.id ^ K_ANIM,
            self.open as u8 as f32,
            t.motion.normal_ms as u64,
            t.motion.easing,
        )
    }

    /// The area the content is laid out in: the window, or the card for a dialog.
    fn area(&self) -> Rectangle {
        let vp = self.ui.viewport();
        if self.mode == Mode::Plain {
            return vp;
        }
        let pad = self.ui.theme().spacing.lg;
        let c = util::preferred(self.content.0.as_ref());
        let w = self
            .width
            .min(vp.size.width - 2.0 * MARGIN)
            .max(c.width.min(vp.size.width))
            .min(vp.size.width);
        let w = w.max(1.0);
        let inner_w = (w - 2.0 * pad).max(1.0);
        let mut h = c.height.min((vp.size.height - 2.0 * MARGIN).max(1.0));
        if let Some(m) = self.max_height {
            h = h.min((m - 2.0 * pad).max(1.0));
        }
        let (cw, ch) = (w, h + 2.0 * pad);
        Rectangle::new(
            Point::new(
                vp.origin.x + (vp.size.width - cw) / 2.0 + pad,
                vp.origin.y + ((vp.size.height - ch) / 2.0).max(0.0) + pad,
            ),
            Size::new(inner_w, h),
        )
    }
    /// The visible card (the content area grown by its padding).
    fn card(&self) -> Rectangle {
        if !self.open && self.keep {
            if let Some((c, _)) = LAST.with(|l| l.borrow().get(&self.id).cloned()) {
                return c;
            }
        }
        let a = self.area();
        if self.mode == Mode::Plain {
            return a;
        }
        let pad = self.ui.theme().spacing.lg;
        Rectangle::new(
            Point::new(a.origin.x - pad, a.origin.y - pad),
            Size::new(a.size.width + 2.0 * pad, a.size.height + 2.0 * pad),
        )
    }

    fn close(&self, out: &mut Vec<M>) {
        if let Some(f) = &self.on_close {
            out.push(f());
        }
    }
    /// Offer an event to the content, laid out where it is drawn.
    fn forward(&mut self, event: &Event, cursor: Point, out: &mut Vec<M>) -> bool {
        let l = self.content.0.arrange(self.area());
        self.content.0.on_event(event, &l, cursor, out)
    }

    fn on_event(&mut self, event: &Event, cursor: Point, out: &mut Vec<M>) -> bool {
        if !self.open {
            return false;
        }
        let dialog = self.mode == Mode::Dialog;
        match event {
            Event::MousePressed(btn) => {
                if let Some(p) = self.ui.take_hit(self.id) {
                    if self.card().contains(p) || !dialog {
                        self.forward(event, p, out);
                    } else if self.scrim_close && *btn == MouseButton::Left {
                        self.close(out);
                    }
                    return true;
                }
                if self.ui.peek_hit().is_some() {
                    // Meant for a popover or menu inside the content.
                    self.forward(event, cursor, out);
                    return true;
                }
                // No overlay press in flight (nothing drawn yet): same thing by position.
                if self.card().contains(cursor) || !dialog {
                    self.forward(event, cursor, out);
                } else if self.scrim_close && *btn == MouseButton::Left {
                    self.close(out);
                }
                true
            }
            Event::Scrolled { .. } => {
                let at = self.ui.take_hit(self.id).map_or(cursor, |p| p);
                self.forward(event, at, out);
                true
            }
            Event::KeyPressed(Key::Escape, _) => {
                // A popover or menu open inside the content closes first; a text field
                // that merely sees Escape must not keep the dialog open.
                let inner = self.ui.slot(self.id ^ K_INNER).is_some();
                if inner && self.forward(event, cursor, out) {
                    return true;
                }
                if dialog {
                    self.close(out);
                    return true;
                }
                // A plain portal: the content sees Escape, then `on_close` takes it.
                let used = self.forward(event, cursor, out);
                if self.on_close.is_some() {
                    self.close(out);
                    return true;
                }
                used
            }
            Event::KeyPressed(Key::Enter, _) if dialog && self.default_action.is_some() => {
                let inner = self.ui.slot(self.id ^ K_INNER).is_some();
                let typing = self.ui.text_focused();
                if (inner || typing) && self.forward(event, cursor, out) {
                    return true;
                }
                if let Some(f) = &self.default_action {
                    out.push(f());
                }
                true
            }
            _ => self.forward(event, cursor, out),
        }
    }

    fn draw(&self) {
        let k = self.shown();
        let key = self.id ^ K_PREV;
        let held = self.ui.slot(key);
        if !self.open && k <= 0.002 {
            if held.is_some() {
                // Closed: hand the focus back to where it was.
                self.ui.set_slot(key, None);
                let id = self.ui.slot(key ^ K_FOCUS_ID);
                self.ui.set_slot(key ^ K_FOCUS_ID, None);
                self.ui.set_focus(id.map(|v| v as u64));
            }
            return;
        }
        let vp = self.ui.viewport();
        let dialog = self.mode == Mode::Dialog;
        if self.open {
            if held.is_none() {
                self.ui.set_slot(key, Some(1));
                self.ui
                    .set_slot(key ^ K_FOCUS_ID, self.ui.focused().map(|f| f as i64));
            }
            // Claim the window first so overlays inside the content are found before it.
            self.ui.claim(self.id, vp);
            self.ui.set_modal(self.id);
        }
        let t = self.ui.theme();
        let mut list = DrawList::default();
        let mark = self.ui.trap_begin();
        let regions = self.ui.region_count();
        if dialog {
            let card = self.card();
            list.0.push(DrawCommand::Quad {
                bounds: vp,
                fill: Color::hex(0x000000).with_alpha(0.45 * k),
                radius: 0.0,
                border: None,
                shadow: None,
            });
            let dy = (1.0 - k) * RISE;
            let mut body = DrawList::default();
            body.0.push(DrawCommand::Quad {
                bounds: card,
                fill: t.palette.surface_raised,
                radius: t.radii.large,
                border: Some(Border {
                    color: t.palette.border,
                    width: 1.0,
                }),
                shadow: Some(t.elevation.high),
            });
            let mut inner = DrawList::default();
            if !self.open && self.keep {
                if let Some((_, cmds)) = LAST.with(|l| l.borrow().get(&self.id).cloned()) {
                    inner.0 = cmds;
                }
            } else {
                let l = self.content.0.arrange(self.area());
                self.content.0.draw(&l, &mut inner);
                if self.keep {
                    LAST.with(|l| l.borrow_mut().insert(self.id, (card, inner.0.clone())));
                }
            }
            body.0.extend(inner.0);
            for c in body.0 {
                list.0.push(util::fade_translate(c, k, 0.0, dy));
            }
        } else {
            let l = self.content.0.arrange(self.area());
            self.content.0.draw(&l, &mut list);
        }
        if self.open {
            self.ui.set_slot(
                self.id ^ K_INNER,
                (self.ui.region_count() > regions).then_some(1),
            );
        }
        if self.open && dialog {
            self.ui.trap_end(mark);
            // Pull focus inside, so Enter and Space act on the dialog from the first frame.
            let inside = self.ui.focused().is_some_and(|f| self.ui.in_trap(f));
            if !inside {
                self.ui.focus_first_in_trap();
            }
        }
        self.ui.overlay(None, None, list.0);
    }
}

macro_rules! layer_widget {
    ($ty:ident) => {
        impl<M: 'static> Widget<M> for $ty<M> {
            fn style(&self) -> Style {
                Style::default()
            }
            fn measure(&self, _available: Size) -> Size {
                Size::new(0.0, 0.0)
            }
            fn on_event(
                &mut self,
                event: &Event,
                _layout: &Layout,
                cursor: Point,
                out: &mut Vec<M>,
            ) -> bool {
                self.0.on_event(event, cursor, out)
            }
            fn animating(&self) -> bool {
                let k = self.0.shown();
                // Moving toward open (k < 1) or toward closed (k > 0), or content inside moves.
                let moving = if self.0.open { k < 0.998 } else { k > 0.002 };
                moving || (self.0.open && self.0.content.0.animating())
            }
            fn draw(&self, _layout: &Layout, _out: &mut DrawList) {
                self.0.draw()
            }
        }
    };
}

/// Hosts `content` on the overlay layer, over the whole window, however deep in a scrolled or
/// clipped container the portal is written. Put a sheet, a lightbox or any custom panel in it.
/// While open it takes every press (nothing underneath is clicked) and the mouse wheel.
///
/// The content is laid out over the whole window, so give it `Fill` sizing or place it yourself
/// (for example with a [`crate::Stack`]). `id` must be unique per window.
pub struct Portal<M>(Layer<M>);

impl<M: 'static> Portal<M> {
    /// A portal for `content`.
    pub fn new(ui: &Ui, id: u64, content: Element<M>) -> Self {
        Self(Layer::new(ui, id, Mode::Plain, content))
    }
    /// Whether the portal is showing.
    pub fn open(mut self, open: bool) -> Self {
        self.0.open = open;
        self
    }
    /// Message to send when Escape reaches the portal and nothing inside it used the key.
    pub fn on_close(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.0.on_close = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}
layer_widget!(Portal);

/// A dialog: a dimmed scrim over the window, a centred card with `content`, Escape (or a click
/// on the scrim) to close, Tab confined to the card, and a fade-and-rise on the way in and out.
///
/// Controlled like [`crate::Popover`]: you keep `open` and learn about closing from
/// `on_close`, which fires on Escape and on a scrim click. Opening moves focus to the first
/// control in the card; closing puts it back where it was. It is drawn on the overlay layer, so
/// it can be written anywhere in the view.
pub struct Modal<M>(Layer<M>);

impl<M: 'static> Modal<M> {
    /// A modal around `content`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, content: Element<M>) -> Self {
        Self(Layer::new(ui, id, Mode::Dialog, content))
    }
    /// Whether the dialog is showing.
    pub fn open(mut self, open: bool) -> Self {
        self.0.open = open;
        self
    }
    /// Card width in pixels (default 420; never wider than the window).
    pub fn width(mut self, w: f32) -> Self {
        self.0.width = w;
        self
    }
    /// Message to send when the user presses Escape or clicks the scrim.
    pub fn on_close(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.0.on_close = Some(Rc::new(f));
        self
    }
    /// Whether a click on the scrim closes the dialog (default yes).
    pub fn scrim_close(mut self, yes: bool) -> Self {
        self.0.scrim_close = yes;
        self
    }
    /// Tallest the card may be, padding included. Content that is taller is squeezed into it,
    /// so a `scroll(..)` with `Fill` height inside scrolls.
    pub fn max_height(mut self, px: f32) -> Self {
        self.0.max_height = Some(px);
        self
    }
    /// Keep drawing the last content while the dialog leaves, instead of an empty card, for
    /// apps that clear the content when they close it.
    pub fn keep_content_on_exit(mut self, yes: bool) -> Self {
        self.0.keep = yes;
        self
    }
    /// Message to send on Enter (unless a text field or an open menu inside uses it).
    pub fn default_action(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.0.default_action = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}
layer_widget!(Modal);
