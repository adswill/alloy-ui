//! `alloy-ui-rating`: a star rating widget, written as an outside crate would write one.
//!
//! ```
//! use alloy_ui::prelude::*;
//! use alloy_ui_rating::rating;
//!
//! #[derive(Default)]
//! struct Review { stars: u8 }
//!
//! let _app = sketch(Review::default(), |r| {
//!     col![
//!         "How was it?",
//!         rating(r.stars).act(|r: &mut Review, n| r.stars = n),
//!     ]
//! });
//! ```
//!
//! Click a star, or focus the widget and press Left/Right, Home or End. Pressing the current
//! rating's star again clears it (when [`Rating::clearable`] is set). Hovering previews.

use alloy_ui::ext::{self, auto_id};
use alloy_ui::plugin::{Host, Plugin};
use alloy_ui::widgets::{Role, Ui};
use alloy_ui::{
    Act, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout, MouseButton, Node, Point,
    Rectangle, Size, Widget,
};
use std::rc::Rc;

const STARS: u8 = 5;
const STAR: f32 = 26.0;
const GAP: f32 = 4.0;
/// Tag for this widget's slot ids; keeps them clear of other widgets' (see `ext::set_state`).
const HOVER_TAG: u64 = 0x5747_0001 << 20;

/// The plugin: registers the widget's translations. Widgets themselves need no registration.
pub struct RatingPlugin;

impl Plugin for RatingPlugin {
    fn name(&self) -> &str {
        "rating"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn install(&self, host: &mut Host) {
        host.strings("en", "rating.name = Rating\nrating.value = {n} of 5 stars");
        host.strings(
            "fr",
            "rating.name = Note\nrating.value.one = {n} étoile sur 5\nrating.value.other = {n} étoiles sur 5",
        );
    }
}

/// A row of five stars showing `value` (0 to 5) and reporting the star the user picks.
pub struct Rating<M> {
    ui: Ui,
    id: u64,
    value: u8,
    clearable: bool,
    on_change: Option<Rc<dyn Fn(u8) -> M>>,
}

/// A rating of `value` stars for apps written in the `sketch` style; react with
/// [`.act`](Rating::act). Marked `#[track_caller]`: its focus id comes from the call site.
#[track_caller]
pub fn rating<S: 'static>(value: u8) -> Rating<Act<S>> {
    Rating::new(&ext::ui(), auto_id(0x5747), value)
}

impl<M: 'static> Rating<M> {
    /// A rating showing `value` stars under focus id `id`. Use this form in `App`-style apps.
    pub fn new(ui: &Ui, id: u64, value: u8) -> Self {
        Self {
            ui: ui.clone(),
            id,
            value: value.min(STARS),
            clearable: false,
            on_change: None,
        }
    }
    /// Called with the picked rating (1 to 5, or 0 when cleared). Without it the widget is read-only.
    pub fn on_change(mut self, f: impl Fn(u8) -> M + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
    /// Let a click on the current rating's star clear the rating to 0.
    pub fn clearable(mut self) -> Self {
        self.clearable = true;
        self
    }
    fn star_rect(&self, b: &Rectangle, i: u8) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x + i as f32 * (STAR + GAP), b.origin.y),
            Size::new(STAR, STAR),
        )
    }
    fn star_at(&self, b: &Rectangle, p: Point) -> Option<u8> {
        (0..STARS).find(|&i| self.star_rect(b, i).contains(p))
    }
}

impl<M: 'static> Rating<M> {
    fn emit(&self, n: u8, out: &mut Vec<M>) {
        if let Some(f) = &self.on_change {
            out.push(f(n));
        }
    }
    fn hover_slot(&self) -> u64 {
        self.id ^ HOVER_TAG
    }
}

impl<S: 'static> Rating<Act<S>> {
    /// Run `f` on the app state with the picked rating.
    pub fn act(self, f: impl Fn(&mut S, u8) + Send + Sync + Clone + 'static) -> Self {
        self.on_change(move |n| {
            let f = f.clone();
            Act::new(move |s: &mut S| f(s, n))
        })
    }
}

impl<M: 'static> Widget<M> for Rating<M> {
    fn measure(&self, _available: Size) -> Size {
        Size::new(STARS as f32 * STAR + (STARS - 1) as f32 * GAP, STAR)
    }

    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let live = self.on_change.is_some();
        match event {
            Event::CursorMoved(_) => {
                let over = self.star_at(&b, cursor).map(|i| i as i64 + 1);
                if live && ext::state(&self.ui, self.hover_slot()) != over {
                    ext::set_state(&self.ui, self.hover_slot(), over);
                }
                false
            }
            Event::MousePressed(MouseButton::Left) if live && b.contains(cursor) => {
                if let Some(i) = self.star_at(&b, cursor) {
                    self.ui.set_focus(Some(self.id));
                    ext::press(&self.ui, &self.star_rect(&b, i));
                    return true;
                }
                false
            }
            Event::MouseReleased(MouseButton::Left) if live => {
                let Some(i) = self.star_at(&b, cursor) else {
                    ext::release(&self.ui);
                    return false;
                };
                let started_here = ext::was_pressed(&self.ui, &self.star_rect(&b, i));
                ext::release(&self.ui);
                if started_here {
                    let n = i + 1;
                    self.emit(
                        if self.clearable && n == self.value {
                            0
                        } else {
                            n
                        },
                        out,
                    );
                }
                started_here
            }
            Event::KeyPressed(Key::Named(k), _) if live && self.ui.is_focused(Some(self.id)) => {
                let next = match k.as_str() {
                    "ArrowRight" | "ArrowUp" => Some((self.value + 1).min(STARS)),
                    "ArrowLeft" | "ArrowDown" => Some(self.value.saturating_sub(1)),
                    "Home" => Some(0),
                    "End" => Some(STARS),
                    _ => None,
                };
                match next {
                    Some(n) if n != self.value => {
                        self.emit(n, out);
                        true
                    }
                    Some(_) => true,
                    None => false,
                }
            }
            _ => false,
        }
    }

    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let live = self.on_change.is_some();
        ext::focusable(&self.ui, self.id);
        let n = alloy_ui::i18n::translate("rating.value", &[("n", self.value.to_string())]);
        let name = alloy_ui::i18n::translate("rating.name", &[]);
        let id = self.id;
        ext::describe(&self.ui, Role::Slider, &name, &b, |a| {
            a.id = Some(id);
            a.value = Some(n.clone());
            a.enabled = live;
        });
        if self.ui.is_focused(Some(self.id)) {
            let whole = Rectangle::new(b.origin, self.measure(b.size));
            out.push(ext::focus_ring(whole, t.radii.small, p.focus_ring));
        }
        // Hovering a star previews that rating; otherwise show the real one.
        let shown = if live && ext::hovered(&self.ui, &b) {
            ext::state(&self.ui, self.hover_slot()).map_or(self.value, |h| h as u8)
        } else {
            self.value
        };
        for i in 0..STARS {
            let r = self.star_rect(&b, i);
            let on = i < shown;
            let color = match (on, live) {
                (true, true) => p.accent,
                (true, false) => ext::mix(p.border, p.accent, 0.5),
                (false, _) => p.border_control,
            };
            out.push(DrawCommand::Text {
                origin: Point::new(r.origin.x + 1.0, r.origin.y - 2.0),
                content: "\u{2605}".into(),
                size: STAR - 2.0,
                color,
                max_width: None,
                weight: 400,
                family: FontFamily::Sans,
                line_height: Some(STAR),
            });
        }
    }
}

impl<M: 'static> From<Rating<M>> for Node<M> {
    fn from(r: Rating<M>) -> Self {
        Node::from(Element::new(r))
    }
}
