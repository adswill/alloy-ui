//! Single-line text field with selection, clipboard and IME preedit.

use crate::a11y::Role;
use crate::ui::Clipboard;
use crate::{
    text_edit::{
        apply_key, commit_edit, history_edit, nearest_boundary, sanitize, Edit, KeyOutcome,
    },
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
    secure: bool,
    invalid: bool,
    selection: Option<std::ops::Range<usize>>,
    select_all_on_focus: bool,
    on_escape: Option<Box<dyn Fn() -> M>>,
}

/// Clipboard view for password fields: paste works, copy and cut do nothing.
struct PasteOnly<'a>(&'a dyn Clipboard);
impl Clipboard for PasteOnly<'_> {
    fn get(&self) -> Option<String> {
        self.0.get()
    }
    fn set(&self, _: String) {}
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
            secure: false,
            invalid: false,
            selection: None,
            select_all_on_focus: false,
            on_escape: None,
        }
    }
    /// Select the chars `range` (clamped to the text; an empty range places the caret). Applied
    /// whenever the range you pass differs from the last one, so the user can then change it.
    pub fn selection(mut self, range: std::ops::Range<usize>) -> Self {
        self.selection = Some(range);
        self
    }
    /// Select all text when the field gains focus (by click or Tab), as search boxes do.
    pub fn select_all_on_focus(mut self, on: bool) -> Self {
        self.select_all_on_focus = on;
        self
    }
    /// Called when Escape is pressed in the focused field. When set, Escape is consumed and the
    /// field keeps focus (call `ui().set_focus(None)` from the handler to blur); when unset,
    /// Escape blurs the field as before.
    pub fn on_escape(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.on_escape = Some(Box::new(f));
        self
    }
    /// Apply a pending `.selection(..)` request to `e`.
    fn apply_selection(&self, e: &mut Edit) {
        if let Some(r) = &self.selection {
            let req = (r.start, r.end);
            if e.sel_req != Some(req) {
                let len = self.value.chars().count();
                e.sel_req = Some(req);
                e.anchor = r.start.min(len);
                e.caret = r.end.max(r.start).min(len);
            }
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
    /// Password mode: characters are drawn as bullets, the value is hidden from
    /// accessibility, and copy and cut are disabled.
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }
    /// Mark the value as invalid: the border and focus ring turn to the danger colour.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }
    /// What is drawn and measured: the value, or one bullet per character.
    fn shown(&self) -> String {
        if self.secure {
            "\u{2022}".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        }
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
        let s: String = self.shown().chars().take(chars).collect();
        self.ui.metrics().width(&s, self.size())
    }
    /// Character index nearest to `x` (absolute) for a field at `b`.
    fn index_at(&self, b: &Rectangle, scroll: f32, x: f32) -> usize {
        let rel = x - b.origin.x - self.pad() + scroll;
        let m = self.ui.metrics();
        let size = self.size();
        nearest_boundary(&self.shown(), rel, |s| m.width(s, size))
    }
    /// Emit `new` as an edit, remembering the old state for undo.
    fn commit(&self, new: String, caret_before: usize, out: &mut Vec<M>) {
        commit_edit(
            &self.ui,
            self.id,
            &self.value,
            new,
            caret_before,
            &self.on_input,
            out,
        );
    }
    /// Undo/redo shortcut; returns true when the key was one.
    fn history(&self, e: &mut Edit, k: &Key, m: &Modifiers, out: &mut Vec<M>) -> bool {
        history_edit(
            &self.ui,
            self.id,
            &self.value,
            e,
            (k, m),
            &self.on_input,
            out,
        )
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
        self.apply_selection(&mut e);
        let caret0 = e.caret;
        let editable = self.on_input.is_some();
        let key = util::key_of(&b);
        let clip = self.ui.clipboard();
        let paste_only = PasteOnly(&*clip);
        let clipboard: &dyn Clipboard = if self.secure { &paste_only } else { &*clip };
        let consumed = match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                let gaining = !self.ui.is_focused(Some(self.id));
                self.ui.set_focus(Some(self.id));
                e.preedit.clear();
                if gaining && self.select_all_on_focus {
                    e.focus_seen = true;
                    e.select_all(len);
                } else {
                    self.ui.set_pressed(Some(key));
                    let i = self.index_at(&b, e.scroll_x, cursor.x);
                    e.move_to(i, false);
                }
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
            // While the IME composes it owns the keyboard: edit keys would act
            // on text under the preedit (platforms that still deliver them).
            Event::KeyPressed(k, _)
                if self.ui.is_focused(Some(self.id)) && !e.preedit.is_empty() =>
            {
                !matches!(k, Key::Tab)
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
                    if let Some(f) = &self.on_escape {
                        out.push(f());
                    } else {
                        self.ui.set_focus(None);
                    }
                    true
                }
                Key::Character(_) if editable && self.history(&mut e, k, m, out) => true,
                _ => match apply_key(&mut e, &self.value, k, m, clipboard, editable, false) {
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
        self.ui
            .describe(Role::TextInput, &self.placeholder, &b, |n| {
                n.id = Some(self.id);
                n.value = (!self.secure).then(|| self.value.clone());
                n.enabled = self.on_input.is_some();
            });
        let focused = self.ui.is_focused(Some(self.id));
        let editable = self.on_input.is_some();
        self.ui.register_text(self.id);
        let mut e = self.ui.edit(self.id);
        let len = self.value.chars().count();
        e.clamp(len);
        self.apply_selection(&mut e);
        if focused && !e.focus_seen && self.select_all_on_focus {
            e.select_all(len);
        }
        e.focus_seen = focused;
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
            let ring = if self.invalid { p.danger } else { p.focus_ring };
            out.push(focus_ring(b, t.radii.medium, ring));
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
                color: if self.invalid {
                    p.danger
                } else {
                    crate::util::mix(
                        p.border,
                        p.border_strong,
                        self.ui
                            .ease(Ui::anim_key(&b, 4), (focused || hover) as u8 as f32),
                    )
                },
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
                family: alloy_ui_core::FontFamily::Sans,
                weight: 400,
                line_height: None,
                origin: Point::new(ox, ty),
                content: self.placeholder.clone(),
                size: s.size,
                color: p.text_secondary.with_alpha(0.7),
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
            let shown = self.shown();
            let before: String = shown.chars().take(e.caret).collect();
            let after: String = shown.chars().skip(e.caret).collect();
            let m = self.ui.metrics();
            let bw = m.width(&before, s.size);
            let pw = m.width(&e.preedit, s.size);
            let mut text = |x: f32, content: String| {
                if !content.is_empty() {
                    out.push(DrawCommand::Text {
                        family: alloy_ui_core::FontFamily::Sans,
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
