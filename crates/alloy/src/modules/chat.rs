//! Chat bubbles and a message composer.

use super::*;
use crate::{
    blocks::local,
    dsl::{button, caption, input, text},
    view::{col, row, View},
    Act,
};
use std::sync::Arc;

/// One message of a [`chat`].
pub struct Msg {
    mine: bool,
    who: String,
    text: String,
    when: String,
}

/// A message from someone else: `message("Jon Reyes", "Looks good to me", "10:42")`.
pub fn message(who: impl Into<String>, text: impl Into<String>, when: impl Into<String>) -> Msg {
    Msg {
        mine: false,
        who: who.into(),
        text: text.into(),
        when: when.into(),
    }
}

/// A message from the person using the app, on the right in the accent colour.
pub fn reply(text: impl Into<String>, when: impl Into<String>) -> Msg {
    Msg {
        mine: true,
        who: String::new(),
        text: text.into(),
        when: when.into(),
    }
}

const MAX: f32 = 380.0;

fn bubble<M: 'static>(m: &Msg, first: bool, last: bool) -> Node<M> {
    let p = cx::palette();
    let size = cx::theme().typography.body.size;
    // Wrap long messages at MAX; short ones stay as narrow as their text.
    let w = (text_width(&m.text, size, 400) + 2.0).min(MAX);
    let t = text(m.text.clone()).w(w);
    let t = if m.mine { t.color(p.on_accent) } else { t };
    let _ = (first, last);
    let b = t.pad_xy(13.0, 9.0).round(15.0);
    if m.mine {
        b.bg(p.accent).into()
    } else {
        b.bg(p.surface).outline().into()
    }
}

/// A conversation: bubbles grouped by sender, the name and avatar on the first of each group and
/// the time under the last.
pub fn chat<M: 'static>(msgs: impl IntoIterator<Item = Msg>) -> Node<M> {
    let msgs: Vec<Msg> = msgs.into_iter().collect();
    let mut rows: Vec<Node<M>> = Vec::new();
    for (i, m) in msgs.iter().enumerate() {
        let same_prev = i > 0 && msgs[i - 1].mine == m.mine && msgs[i - 1].who == m.who;
        let same_next = msgs
            .get(i + 1)
            .is_some_and(|n| n.mine == m.mine && n.who == m.who);
        let b = bubble::<M>(m, !same_prev, !same_next);
        let mut stack: Vec<Node<M>> = Vec::new();
        if !m.mine && !same_prev {
            stack.push(text(m.who.clone()).label().into());
        }
        stack.push(b);
        if !same_next && !m.when.is_empty() {
            stack.push(caption(m.when.clone()).dim().into());
        }
        let col_gap = 3.0;
        let stack = col(stack).gap(col_gap);
        let line: Node<M> = if m.mine {
            row([space_fill(), stack.into()]).fill_w().into()
        } else {
            let lead: Node<M> = if same_next {
                crate::dsl::gap(30.0, 0.0)
            } else {
                // The avatar sits beside the last bubble of the group, like a messenger.
                super::people::avatar(m.who.clone()).size(30.0).into()
            };
            row([lead, stack.into()]).gap(10.0).fill_w().into()
        };
        if i > 0 && !same_prev {
            rows.push(crate::dsl::gap(0.0, 10.0));
        }
        rows.push(line);
    }
    col(rows).gap(4.0).fill_w().into()
}

fn space_fill<M: 'static>() -> Node<M> {
    hfill()
}

/// The text box under a conversation. See [`composer`].
pub struct Composer<S> {
    hint: String,
    on_send: Option<Arc<dyn Fn(&mut S, String) + Send + Sync>>,
}

/// An input and a Send button; Enter sends too. The draft lives in the view and clears after a
/// send: `composer().on_send(|s, text| s.messages.push(reply(text, "now")))`.
pub fn composer<S: 'static>() -> Composer<S> {
    Composer {
        hint: "Write a message".into(),
        on_send: None,
    }
}

impl<S: 'static> Composer<S> {
    /// Placeholder text.
    pub fn hint(mut self, s: impl Into<String>) -> Self {
        self.hint = s.into();
        self
    }
    /// Called with the trimmed text when it is sent.
    pub fn on_send(mut self, f: impl Fn(&mut S, String) + Send + Sync + 'static) -> Self {
        self.on_send = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<Composer<S>> for Node<Act<S>> {
    fn from(c: Composer<S>) -> Self {
        let draft = local(String::new());
        let cur = draft.get();
        let empty = cur.trim().is_empty();
        let f = c.on_send;
        let send = move |s: &mut S| {
            let t = draft.get().trim().to_string();
            if t.is_empty() {
                return;
            }
            if let Some(f) = &f {
                f(s, t);
            }
            draft.set(String::new());
        };
        row([
            nn(input(cur)
                .hint(c.hint)
                .local(&draft)
                .fill_w()
                .enter(Act::new(send.clone()))),
            button("Send").primary().enabled(!empty).act(send).into(),
        ])
        .gap(8.0)
        .align_center()
        .fill_w()
        .into()
    }
}
