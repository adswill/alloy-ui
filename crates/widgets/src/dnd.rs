//! Drag and drop inside the app: [`Draggable`] picks a value up, [`DropZone`]
//! takes it.
//!
//! A drag starts when the pointer moves a few pixels with the left button down
//! on a draggable; the dragged widget dims in place and a translucent copy
//! follows the pointer on the overlay layer. Releasing over a drop zone that
//! accepts the payload's type sends its message; releasing anywhere else, or
//! pressing Escape, cancels. Drop zones that accept the payload light up while
//! it is in flight, and more strongly under the pointer.
//!
//! Payloads are any `'static` value: an id, an enum, a whole record. A zone
//! only reacts to payloads of the type it was made for.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use std::any::{Any, TypeId};
use std::rc::Rc;

/// Pointer travel, in logical pixels, before a press becomes a drag.
const THRESHOLD: f32 = 4.0;

/// The drag in progress (or about to start), kept in the window's [`Ui`].
pub(crate) struct DragState {
    pub(crate) source: u64,
    payload: Rc<dyn Any>,
    type_id: TypeId,
    press: Point,
    pub(crate) active: bool,
    ghost: Vec<DrawCommand>,
}

impl Ui {
    /// The payload of the drag in flight, if it is a `T`.
    pub fn drag_payload<T: Clone + 'static>(&self) -> Option<T> {
        self.drag_state(|d| {
            d.as_ref()
                .filter(|d| d.active)
                .and_then(|d| d.payload.downcast_ref::<T>().cloned())
        })
    }
    fn drag_is(&self, type_id: TypeId) -> bool {
        self.drag_state(|d| d.as_ref().is_some_and(|d| d.active && d.type_id == type_id))
    }
}

/// Makes its child draggable, carrying `payload`.
pub struct Draggable<M> {
    ui: Ui,
    id: u64,
    payload: Rc<dyn Any>,
    type_id: TypeId,
    child: Vec<Element<M>>,
}

impl<M: 'static> Draggable<M> {
    /// Let `child` be picked up and dropped, carrying `payload`. `id` tells
    /// this draggable from others across view rebuilds.
    pub fn new<T: 'static>(ui: &Ui, id: u64, payload: T, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            payload: Rc::new(payload),
            type_id: TypeId::of::<T>(),
            child: vec![child],
        }
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
    fn mine(&self) -> bool {
        self.ui
            .drag_state(|d| d.as_ref().is_some_and(|d| d.source == self.id && d.active))
    }
}

impl<M: 'static> Widget<M> for Draggable<M> {
    fn style(&self) -> Style {
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
        let b = layout.bounds;
        match event {
            Event::MousePressed(MouseButton::Left) => {
                let consumed = util::dispatch(&mut self.child, event, layout, cursor, out);
                if !consumed && b.contains(cursor) {
                    let (payload, type_id, id) = (self.payload.clone(), self.type_id, self.id);
                    self.ui.drag_state(|d| {
                        *d = Some(DragState {
                            source: id,
                            payload,
                            type_id,
                            press: cursor,
                            active: false,
                            ghost: Vec::new(),
                        })
                    });
                }
                consumed
            }
            Event::CursorMoved(p) => {
                util::dispatch(&mut self.child, event, layout, cursor, out);
                let armed = self.ui.drag_state(|d| {
                    d.as_ref().is_some_and(|d| {
                        d.source == self.id
                            && !d.active
                            && (p.x - d.press.x).hypot(p.y - d.press.y) > THRESHOLD
                    })
                });
                if armed {
                    // Snapshot what the pointer is carrying while it is still where it was.
                    let mut snap = DrawList::default();
                    util::draw_children(&self.child, layout, &mut snap);
                    self.ui.drag_state(|d| {
                        if let Some(d) = d {
                            d.active = true;
                            d.ghost = snap.0;
                        }
                    });
                    // The child must not treat the eventual release as a click.
                    self.ui.set_pressed(None);
                }
                // Never consume: other zones and widgets keep tracking the pointer.
                false
            }
            // Not consumed either: the drop zones elsewhere in the tree must see the release
            // that ends the drag. `Root` clears the drag once everyone has.
            Event::MouseReleased(MouseButton::Left) if self.mine() => false,
            Event::KeyPressed(Key::Escape, _) if self.mine() => {
                self.ui.drag_state(|d| *d = None);
                true
            }
            _ => util::dispatch(&mut self.child, event, layout, cursor, out),
        }
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        if !self.mine() {
            util::draw_children(&self.child, layout, out);
            return;
        }
        // The original stays, dimmed, so the layout does not jump.
        let mut own = DrawList::default();
        util::draw_children(&self.child, layout, &mut own);
        for c in own.0 {
            out.push(util::fade_translate(c, 0.35, 0.0, 0.0));
        }
        let (ghost, press) = self
            .ui
            .drag_state(|d| d.as_ref().map(|d| (d.ghost.clone(), d.press)))
            .unwrap_or_default();
        let at = self.ui.cursor();
        let (dx, dy) = (at.x - press.x, at.y - press.y - 2.0);
        self.ui.overlay(
            None,
            None,
            ghost
                .into_iter()
                .map(|c| util::fade_translate(c, 0.92, dx, dy))
                .collect(),
        );
    }
}

type Taker<M> = Box<dyn Fn(&dyn Any) -> Option<M>>;

/// Takes a dropped payload of one type and turns it into a message.
pub struct DropZone<M> {
    ui: Ui,
    type_id: TypeId,
    take: Taker<M>,
    child: Vec<Element<M>>,
}

impl<M: 'static> DropZone<M> {
    /// Accept payloads of type `T` released over `child`; `on_drop` makes the message.
    pub fn new<T: Clone + 'static>(
        ui: &Ui,
        child: Element<M>,
        on_drop: impl Fn(T) -> M + 'static,
    ) -> Self {
        Self {
            ui: ui.clone(),
            type_id: TypeId::of::<T>(),
            take: Box::new(move |p| p.downcast_ref::<T>().cloned().map(&on_drop)),
            child: vec![child],
        }
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for DropZone<M> {
    fn style(&self) -> Style {
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
        // Inner zones first: the innermost one under the pointer wins.
        if util::dispatch(&mut self.child, event, layout, cursor, out) {
            return true;
        }
        if let Event::MouseReleased(MouseButton::Left) = event {
            if layout.bounds.contains(cursor) && self.ui.drag_is(self.type_id) {
                let msg = self
                    .ui
                    .drag_state(|d| d.as_ref().and_then(|d| (self.take)(d.payload.as_ref())));
                if let Some(m) = msg {
                    out.push(m);
                    return true;
                }
            }
        }
        false
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let live = self.ui.drag_is(self.type_id);
        let over = live && b.contains(self.ui.cursor());
        let show = self.ui.ease(Ui::anim_key(&b, 11), live as u8 as f32);
        let hot = self.ui.ease(Ui::anim_key(&b, 12), over as u8 as f32);
        if show <= 0.01 {
            util::draw_children(&self.child, layout, out);
            return;
        }
        let t = self.ui.theme();
        let accent = t.palette.accent;
        // A tint behind the content, an outline over it: the content stays crisp.
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: accent.with_alpha(0.05 * show + 0.10 * hot),
            radius: t.radii.medium,
            border: None,
            shadow: None,
        });
        util::draw_children(&self.child, layout, out);
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: accent.with_alpha(0.0),
            radius: t.radii.medium,
            border: Some(Border {
                color: accent.with_alpha(0.35 * show + 0.55 * hot),
                width: 1.0 + hot,
            }),
            shadow: None,
        });
    }
}
