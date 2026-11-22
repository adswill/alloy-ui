//! The overlay layer in the DSL: [`modal`] (a dialog) and [`portal`] (anything above the
//! window). Both are drawn above the whole window and take the pointer while open, however deep
//! in a scroll area or clipped card they are written.

use crate::{
    collections::Cell,
    cx::{auto_id, ui},
    dsl::Act,
    view::Node,
};
use alloy_ui_widgets as w;
use std::sync::Arc;

/// A dialog under construction. See [`modal`].
pub struct Modal<M> {
    content: Node<M>,
    open: bool,
    width: f32,
    scrim_close: bool,
    on_close: Option<Arc<dyn Fn() -> M + Send + Sync>>,
    id: u64,
}

/// A dialog: dimmed scrim, centred card holding `content`, Escape (or a click on the scrim)
/// to close, Tab kept inside the card, and a fade-and-rise in and out. Focus returns to where
/// it was when it closes. It is drawn above everything, so it can sit anywhere in the view,
/// even inside a scroll area. You keep `open`; `.close(..)` says what Escape and the scrim do.
///
/// ```
/// use alloy_ui_widgets as _;
/// use alloy_ui::prelude::*;
///
/// #[derive(Default)]
/// struct App { confirm: bool }
///
/// let _ = sketch(App::default(), |s: &App| {
///     col![
///         button("Delete").act(|s: &mut App| s.confirm = true),
///         modal(s.confirm, col![
///             heading("Delete this?"),
///             button("Cancel").act(|s: &mut App| s.confirm = false),
///         ])
///         .close(|s: &mut App| s.confirm = false),
///     ]
/// });
/// ```
#[track_caller]
pub fn modal<M: 'static>(open: bool, content: impl Into<Node<M>>) -> Modal<M> {
    Modal {
        content: content.into(),
        open,
        width: 420.0,
        scrim_close: true,
        on_close: None,
        id: auto_id(90),
    }
}

impl<M: 'static> Modal<M> {
    /// Card width in pixels (default 420; never wider than the window).
    pub fn width(mut self, px: f32) -> Self {
        self.width = px;
        self
    }
    /// Whether a click on the scrim closes it (default yes).
    pub fn scrim_close(mut self, yes: bool) -> Self {
        self.scrim_close = yes;
        self
    }
    /// The message to send when the user presses Escape or clicks the scrim.
    pub fn on_close(mut self, msg: M) -> Self
    where
        M: Clone + Send + Sync,
    {
        self.on_close = Some(Arc::new(move || msg.clone()));
        self
    }
}

impl<S: 'static> Modal<Act<S>> {
    /// Closure style: `.close(|s| s.confirm = false)` runs on Escape and on a scrim click.
    pub fn close(mut self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        let a = Act::new(f);
        self.on_close = Some(Arc::new(move || a.clone()));
        self
    }
    /// Close by writing `false` into a view-local flag (`let open = local(false);`).
    pub fn local(self, cell: &impl Cell<bool>) -> Self {
        let cell = *cell;
        self.close(move |_| cell.write(false))
    }
}

impl<M: 'static> From<Modal<M>> for Node<M> {
    fn from(m: Modal<M>) -> Self {
        let mut e = w::Modal::new(&ui(), m.id, m.content.into_element())
            .open(m.open)
            .width(m.width)
            .scrim_close(m.scrim_close);
        if let Some(f) = m.on_close {
            e = e.on_close(move || f());
        }
        Node(e.into_element())
    }
}

/// Hosts `content` above the whole window while `open`, wherever in the view this is written:
/// a sheet, a lightbox or any panel that must not be clipped by a scroll area or covered by a
/// later sibling. While open it takes every press and the mouse wheel. Lay `content` out over
/// the window yourself (give it `Fill` sizing, or put it in a [`stack`](crate::stack)).
#[track_caller]
pub fn portal<M: 'static>(open: bool, content: impl Into<Node<M>>) -> Node<M> {
    let id = auto_id(91);
    Node(
        w::Portal::new(&ui(), id, content.into().into_element())
            .open(open)
            .into_element(),
    )
}
