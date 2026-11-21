//! Onboarding tour with a spotlight.

use super::*;
use crate::{
    blocks::{local, Local},
    dsl::{button, caption, text},
    view::{col, row},
    Act,
};
use alloy_ui_widgets as w;

/// The state of a product tour. See [`tour`].
#[derive(Clone, Copy)]
pub struct Tour {
    step: Local<usize>,
    on: Local<bool>,
    total: usize,
}

/// A guided tour of `total` stops. Wrap each widget to introduce with [`Tour::spot`]; the window
/// dims around the current one and a card explains it. State lives in the view; it starts open
/// the first time the view is built (`restart` brings it back, e.g. from a Help button).
#[track_caller]
pub fn tour(total: usize) -> Tour {
    Tour {
        step: local(0),
        on: local(true),
        total,
    }
}

impl Tour {
    /// The step being shown, from 0.
    pub fn current(&self) -> usize {
        self.step.get()
    }
    /// Whether the tour is on screen.
    pub fn running(&self) -> bool {
        self.on.get()
    }
    /// Start over from the first stop.
    pub fn restart(&self) {
        self.step.set(0);
        self.on.set(true);
    }
    /// Close the tour.
    pub fn stop(&self) {
        self.on.set(false);
    }

    /// Introduce `target` as stop number `index` (from 0) with a title and a sentence.
    #[track_caller]
    pub fn spot<S: 'static>(
        &self,
        index: usize,
        target: impl Into<Node<Act<S>>>,
        title: &str,
        body: &str,
    ) -> Node<Act<S>> {
        let id = cx::auto_id(80) ^ (index as u64).wrapping_mul(0x9E37_79B9);
        let (step, on, total) = (self.step, self.on, self.total);
        let active = on.get() && step.get() == index;
        let last = index + 1 >= total;
        let mut nav: Vec<Node<Act<S>>> = vec![hfill()];
        if index > 0 {
            nav.push(
                button("Back")
                    .ghost()
                    .act(move |_| step.set(index - 1))
                    .into(),
            );
        }
        nav.push(
            button(if last { "Done" } else { "Next" })
                .primary()
                .act(move |_| {
                    if last {
                        on.set(false);
                    } else {
                        step.set(index + 1);
                    }
                })
                .into(),
        );
        let card = col([
            Node::from(
                row([
                    caption(format!("{} of {}", index + 1, total)).dim().into(),
                    hfill(),
                    button("Skip").ghost().act(move |_| on.set(false)).into(),
                ])
                .align_center()
                .w(300.0),
            ),
            text(title.to_string()).sub().into(),
            text(body.to_string()).dim().w(300.0).into(),
            crate::dsl::gap(0.0, 4.0),
            row(nav).gap(8.0).align_center().w(300.0).into(),
        ])
        .gap(6.0);
        Node::from(
            w::Spotlight::new(
                &cx::ui(),
                id,
                target.into().into_element(),
                Node::from(card).into_element(),
            )
            .active(active)
            .on_dismiss(move || Act::new(move |_: &mut S| on.set(false)))
            .into_element(),
        )
    }
}
