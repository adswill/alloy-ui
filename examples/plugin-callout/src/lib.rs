//! `alloy-ui-callout`: a tinted note/warning box that wraps any content. The second example
//! plugin: where `alloy-ui-rating` is a leaf widget with its own input handling, this one is a
//! **container**, so it shows `children`, `arrange`, `ext::dispatch` and `ext::draw_children`.
//!
//! ```
//! use alloy_ui::prelude::*;
//! use alloy_ui::Tone;
//! use alloy_ui_callout::callout;
//!
//! let _app = sketch(0u32, |_n| {
//!     col![
//!         callout(Tone::Warning, "Disk almost full")
//!             .title("Heads up")
//!             .content(col!["Free some space before the next sync."]),
//!     ]
//! });
//! ```

use alloy_ui::ext::{self, auto_id};
use alloy_ui::layout::Length;
use alloy_ui::plugin::{Host, Plugin};
use alloy_ui::style::{Align, Axis, Padding, Style};
use alloy_ui::widgets::{Role, Ui};
use alloy_ui::{
    Border, DrawCommand, DrawList, Element, Event, Layout, Node, Point, Rectangle, Size, Tone,
    Widget,
};

/// Registers the callout's translations (the screen-reader names of each tone).
pub struct CalloutPlugin;

impl Plugin for CalloutPlugin {
    fn name(&self) -> &str {
        "callout"
    }
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }
    fn description(&self) -> &str {
        "A tinted note, success, warning or error box around any content."
    }
    fn install(&self, host: &mut Host) {
        host.strings(
            "en",
            "callout.neutral = Note\ncallout.accent = Tip\ncallout.success = Success\ncallout.warning = Warning\ncallout.danger = Error",
        );
        host.strings(
            "fr",
            "callout.neutral = Note\ncallout.accent = Astuce\ncallout.success = Succès\ncallout.warning = Attention\ncallout.danger = Erreur",
        );
    }
}

const BAR: f32 = 4.0;

/// A box with a tone-coloured bar on its left edge and a faint tinted fill, around one child.
pub struct Callout<M> {
    ui: Ui,
    tone: Tone,
    title: Option<String>,
    /// The child, or empty: kept as a one-element vec so `children()` can borrow it.
    child: Vec<Element<M>>,
}

/// A callout of `tone` containing `text`. Swap the body with [`Callout::content`].
#[track_caller]
pub fn callout<M: 'static>(tone: Tone, text: impl Into<Node<M>>) -> Callout<M> {
    let _ = auto_id(0xCA11); // a distinct id per call site, so two callouts never share state
    Callout::new(&ext::ui(), tone, text.into().into_element())
}

impl<M: 'static> Callout<M> {
    /// A callout of `tone` around `child`. Use this form in `App`-style apps.
    pub fn new(ui: &Ui, tone: Tone, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            tone,
            title: None,
            child: vec![child],
        }
    }
    /// The screen-reader name; defaults to the tone's name ("Warning", "Tip", ...).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
    /// Replace the body with any view.
    pub fn content(mut self, body: impl Into<Node<M>>) -> Self {
        self.child = vec![body.into().into_element()];
        self
    }
    fn key(&self) -> &'static str {
        match self.tone {
            Tone::Neutral => "callout.neutral",
            Tone::Accent => "callout.accent",
            Tone::Success => "callout.success",
            Tone::Warning => "callout.warning",
            Tone::Danger => "callout.danger",
        }
    }
}

impl<M: 'static> Widget<M> for Callout<M> {
    fn style(&self) -> Style {
        let sp = self.ui.theme().spacing;
        Style {
            axis: Axis::Vertical,
            padding: Padding {
                top: sp.sm,
                right: sp.md,
                bottom: sp.sm,
                left: sp.md + BAR,
            },
            align: Align::Start,
            width: Length::Fill(1),
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        // Containers ask the layout engine; `arrange_widget` does the same for the real pass.
        ext::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        ext::arrange_widget(self, bounds)
    }
    fn on_event(&mut self, e: &Event, l: &Layout, c: Point, out: &mut Vec<M>) -> bool {
        ext::dispatch(&mut self.child, e, l, c, out)
    }
    fn animating(&self) -> bool {
        self.child.iter().any(|c| c.0.animating())
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        let ink = self.tone.color(&t);
        let r = t.radii.small;
        let name = self
            .title
            .clone()
            .unwrap_or_else(|| alloy_ui::i18n::translate(self.key(), &[]));
        ext::describe(&self.ui, Role::Label, &name, &b, |_| {});
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: ext::mix(t.palette.surface, ink, 0.10),
            radius: r,
            border: Some(Border {
                color: ext::mix(t.palette.border, ink, 0.35),
                width: 1.0,
            }),
            shadow: None,
        });
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(b.origin, Size::new(BAR, b.size.height)),
            fill: ink,
            radius: r.min(BAR),
            border: None,
            shadow: None,
        });
        ext::draw_children(&self.child, layout, out);
    }
}

impl<M: 'static> From<Callout<M>> for Node<M> {
    fn from(c: Callout<M>) -> Self {
        Node::from(Element::new(c))
    }
}
