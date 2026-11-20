//! Single-line text field with selection, clipboard and IME preedit.

use crate::{
    text_edit::{apply_key, history_key, sanitize, Edit, KeyOutcome},
    ui::Ui,
    util::{self, focus_ring},
};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, Modifiers,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// An editable single-line field. Controlled: you own the `value` and
/// receive the new string through `on_input`. Caret, selection and IME
/// preedit live in the [`Ui`] under `id`.
pub struct TextInput<M> {
    ui: Ui,
    id: u64,
    value: String,
    placeholder: String,
    on_input: Option<Box<dyn Fn(String) -> M>>,
    on_submit: Option<Box<dyn Fn() -> M>>,
    width: Length,
}

impl<M: 'static> TextInput<M> {
    /// A field showing `value`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, value: impl Into<String>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            value: value.into(),
            placeholder: String::new(),
            on_input: None,
            on_submit: None,
            width: Length::Fill(1),
        }
    }
    /// Hint shown while empty.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// Called with the new text on every edit; without it the field is read-only.
    pub fn on_input(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.on_input = Some(Box::new(f));
        self
    }
    /// Called when Enter is pressed.
    pub fn on_submit(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.on_submit = Some(Box::new(f));
        self
    }
    /// Width sizing (default fills).
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn pad(&self) -> f32 {
        self.ui.theme().spacing.md
    }
    fn size(&self) -> f32 {
        self.ui.theme().typography.body.size
    }
    fn prefix_width(&self, chars: usize) -> f32 {
        let s: String = self.value.chars().take(chars).collect();
        self.ui.metrics().width(&s, self.size())
    }
    /// Character index nearest to `x` (absolute) for a field at `b`.
    fn index_at(&self, b: &Rectangle, scroll: f32, x: f32) -> usize {
        let rel = x - b.origin.x - self.pad() + scroll;
        let m = self.ui.metrics();
        let mut best = (0usize, f32::MAX);
        let mut acc = String::new();
        for i in 0..=self.value.chars().count() {
            if i > 0 {
                acc.extend(self.value.chars().nth(i - 1));
            }
            let d = (m.width(&acc, self.size()) - rel).abs();
            if d < best.1 {
                best = (i, d);
            }
        }
        best.0
    }
    /// Emit `new` as an edit, remembering the old state for undo.
    fn commit(&self, new: String, caret_before: usize, out: &mut Vec<M>) {
        let now = self.ui.now();
        self.ui
            .with_history(self.id, |h| h.record(&self.value, caret_before, &new, now));
        if let Some(f) = &self.on_input {
            out.push(f(new));
        }
    }
    /// Undo/redo shortcut; returns true when the key was one.
    fn history(&self, e: &mut Edit, k: &Key, m: &Modifiers, out: &mut Vec<M>) -> bool {
        let Some(f) = &self.on_input else {
            return false;
        };
        match self
            .ui
            .with_history(self.id, |h| history_key(e, h, &self.value, k, m))
        {
            Some(v) => {
                out.push(f(v));
                true
            }
            None => false,
        }
    }
}

impl<M: 'static> Widget<M> for TextInput<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        Size::new(200.0, t.typography.body.line_height + 2.0 * t.spacing.sm)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let len = self.value.chars().count();
        let mut e = self.ui.edit(self.id);
        e.clamp(len);
        let caret0 = e.caret;
        let editable = self.on_input.is_some();
        let key = util::key_of(&b);
        let consumed = match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
                self.ui.set_pressed(Some(key));
                let i = self.index_at(&b, e.scroll_x, cursor.x);
                e.move_to(i, false);
                e.preedit.clear();
                true
            }
            Event::CursorMoved(p) if self.ui.pressed() == Some(key) => {
                let i = self.index_at(&b, e.scroll_x, p.x);
                e.move_to(i, true);
                true
            }
            Event::MouseReleased(MouseButton::Left) => self.ui.pressed() == Some(key),
            Event::ImePreedit(s) if self.ui.is_focused(Some(self.id)) => {
                e.preedit = s.clone();
                true
            }
            Event::TextInput(s) if self.ui.is_focused(Some(self.id)) => {
                e.preedit.clear();
                let clean = sanitize(s, false);
                if editable && !clean.is_empty() {
                    let v = e.insert(&self.value, &clean);
                    self.commit(v, caret0, out);
                }
                true
            }
            Event::KeyPressed(k, m) if self.ui.is_focused(Some(self.id)) => match k {
                Key::Tab => return false,
                Key::Enter => {
                    if let Some(f) = &self.on_submit {
                        out.push(f());
                    }
                    true
                }
                Key::Escape => {
                    self.ui.set_focus(None);
                    true
                }
                Key::Character(_) if editable && self.history(&mut e, k, m, out) => true,
                _ => match apply_key(
                    &mut e,
                    &self.value,
                    k,
                    m,
                    &*self.ui.clipboard(),
                    editable,
                    false,
                ) {
                    KeyOutcome::Ignored => false,
                    KeyOutcome::Handled => true,
                    KeyOutcome::Changed(v) => {
                        self.commit(v, caret0, out);
                        true
                    }
                },
            },
            _ => false,
        };
        if consumed {
            e.blink_epoch = self.ui.now();
        }
        self.ui.set_edit(self.id, e);
        consumed
    }
    fn animating(&self) -> bool {
        // Only a focused, editable field blinks; everything else idles.
        self.on_input.is_some() && self.ui.is_focused(Some(self.id))
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let focused = self.ui.is_focused(Some(self.id));
        let editable = self.on_input.is_some();
        self.ui.register(self.id);
        let mut e = self.ui.edit(self.id);
        e.clamp(self.value.chars().count());
        let s = t.typography.body;
        let pad = self.pad();
        let inner_w = (b.size.width - 2.0 * pad).max(0.0);

        // Keep the caret visible.
        let caret_x = self.prefix_width(e.caret) + self.ui.metrics().width(&e.preedit, s.size);
        if caret_x - e.scroll_x > inner_w {
            e.scroll_x = caret_x - inner_w;
        }
        if caret_x < e.scroll_x {
            e.scroll_x = caret_x;
        }
        e.scroll_x = e.scroll_x.max(0.0);
        self.ui.set_edit(self.id, e.clone());

        if focused {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        let hover = self.ui.hovered(&b);
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: if editable {
                p.surface_raised
            } else {
                p.surface
            },
            radius: t.radii.medium,
            border: Some(Border {
                color: crate::util::mix(
                    p.border,
                    p.border_strong,
                    self.ui
                        .ease(Ui::anim_key(&b, 4), (focused || hover) as u8 as f32),
                ),
                width: 1.0,
            }),
            shadow: None,
        });
        let clip = Rectangle::new(
            Point::new(b.origin.x + pad, b.origin.y),
            Size::new(inner_w, b.size.height),
        );
        out.push(DrawCommand::PushClip(clip));
        let ox = clip.origin.x - e.scroll_x;
        let ty = b.origin.y + (b.size.height - s.line_height) / 2.0;
        if self.value.is_empty() && e.preedit.is_empty() {
            out.push(DrawCommand::Text {
                weight: 400,
                line_height: None,
                origin: Point::new(ox, ty),
                content: self.placeholder.clone(),
                size: s.size,
                color: p.text_secondary,
                max_width: None,
            });
        } else {
            if focused && e.has_selection() {
                let (a, z) = e.range();
                let (xa, xz) = (self.prefix_width(a), self.prefix_width(z));
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(ox + xa, ty),
                        Size::new(xz - xa, s.line_height),
                    ),
                    fill: p.selection,
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
            let color = if editable { p.text } else { p.text_secondary };
            let before: String = self.value.chars().take(e.caret).collect();
            let after: String = self.value.chars().skip(e.caret).collect();
            let m = self.ui.metrics();
            let bw = m.width(&before, s.size);
            let pw = m.width(&e.preedit, s.size);
            let mut text = |x: f32, content: String| {
                if !content.is_empty() {
                    out.push(DrawCommand::Text {
                        weight: 400,
                        line_height: None,
                        origin: Point::new(ox + x, ty),
                        content,
                        size: s.size,
                        color,
                        max_width: None,
                    });
                }
            };
            text(0.0, before);
            text(bw, e.preedit.clone());
            text(bw + pw, after);
            if pw > 0.0 {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(ox + bw, ty + s.line_height - 2.0),
                        Size::new(pw, 1.0),
                    ),
                    fill: p.text,
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
        }
        let blink_on = (self.ui.now().saturating_sub(e.blink_epoch) / 530) % 2 == 0;
        if focused && editable && blink_on {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(Point::new(ox + caret_x, ty), Size::new(1.5, s.line_height)),
                fill: p.text,
                radius: 0.0,
                border: None,
                shadow: None,
            });
        }
        out.push(DrawCommand::PopClip);
    }
}
