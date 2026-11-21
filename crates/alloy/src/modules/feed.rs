//! Timeline, changelog, activity feed and notification centre.

use super::*;
use crate::{
    dsl::{button, caption, heading, rule, text},
    rich::badge,
    tone,
    view::{col, row, Flex, View},
};
use alloy_ui_widgets::Tone;
use std::sync::Arc;

// ------------------------------------------------------------- timeline

/// One entry of a [`moments`].
pub struct Moment {
    when: String,
    title: String,
    detail: Option<String>,
    tone: Tone,
}

/// A timeline entry: when it happened and what: `moment("2 min ago", "Deploy finished").success()`.
pub fn moment(when: impl Into<String>, title: impl Into<String>) -> Moment {
    Moment {
        when: when.into(),
        title: title.into(),
        detail: None,
        tone: Tone::Neutral,
    }
}

impl Moment {
    /// A sentence under the title.
    pub fn detail(mut self, s: impl Into<String>) -> Self {
        self.detail = Some(s.into());
        self
    }
    /// Colour the marker.
    pub fn tone(mut self, t: Tone) -> Self {
        self.tone = t;
        self
    }
    /// Green marker.
    pub fn success(self) -> Self {
        self.tone(Tone::Success)
    }
    /// Amber marker.
    pub fn warning(self) -> Self {
        self.tone(Tone::Warning)
    }
    /// Red marker.
    pub fn danger(self) -> Self {
        self.tone(Tone::Danger)
    }
    /// Accent marker.
    pub fn accent(self) -> Self {
        self.tone(Tone::Accent)
    }
}

/// A vertical timeline: a marker and a hairline rail on the left, the story on the right.
pub fn moments<M: 'static>(items: impl IntoIterator<Item = Moment>) -> Node<M> {
    let items: Vec<Moment> = items.into_iter().collect();
    let last = items.len().saturating_sub(1);
    col(items.into_iter().enumerate().map(|(i, m)| {
        let c = match m.tone {
            Tone::Neutral => quiet(),
            t => tone_color(t),
        };
        let ty = &cx::theme().typography;
        let item_h = ty.label.line_height
            + if m.detail.is_some() {
                3.0 + ty.body.line_height
            } else {
                0.0
            }
            + 18.0;
        let mut rail: Vec<Node<M>> = vec![col([tone::dot::<M>(c, 10.0)]).pad_xy(0.0, 4.0).into()];
        if i != last {
            rail.push(
                col(Vec::<Node<M>>::new())
                    .w(1.0)
                    .h((item_h - 18.0 - 4.0).max(8.0))
                    .bg(cx::palette().border)
                    .into(),
            );
        }
        let mut body: Vec<Node<M>> = vec![row([
            text(m.title).label().into(),
            hfill(),
            caption(m.when).dim().into(),
        ])
        .align_center()
        .fill_w()
        .into()];
        if let Some(d) = m.detail {
            body.push(text(d).dim().fill_w().into());
        }
        body.push(crate::dsl::gap(0.0, 15.0));
        Node::<M>::from(
            row([
                Node::from(col(rail).align_center().w(12.0).gap(4.0)),
                col(body).gap(3.0).fill_w().into(),
            ])
            .gap(14.0)
            .fill_w(),
        )
    }))
    .gap(0.0)
    .fill_w()
    .into()
}

// ------------------------------------------------------------ changelog

/// One version of a [`changelog`].
pub struct Release {
    version: String,
    date: String,
    note: Option<String>,
    latest: bool,
    groups: Vec<(&'static str, Vec<String>)>,
}

/// A release: `release("0.4.0", "Nov 18").added(["Gauges"]).fixed(["Scroll jitter"])`.
pub fn release(version: impl Into<String>, date: impl Into<String>) -> Release {
    Release {
        version: version.into(),
        date: date.into(),
        note: None,
        latest: false,
        groups: Vec::new(),
    }
}

impl Release {
    fn group(
        mut self,
        name: &'static str,
        items: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.groups
            .push((name, items.into_iter().map(Into::into).collect()));
        self
    }
    /// New things.
    pub fn added(self, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.group("Added", items)
    }
    /// Behaviour that moved.
    pub fn changed(self, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.group("Changed", items)
    }
    /// Bugs squashed.
    pub fn fixed(self, items: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.group("Fixed", items)
    }
    /// A sentence of context under the version.
    pub fn note(mut self, s: impl Into<String>) -> Self {
        self.note = Some(s.into());
        self
    }
    /// Mark as the newest release.
    pub fn latest(mut self) -> Self {
        self.latest = true;
        self
    }
}

/// A changelog: one block per release, newest first, with Added / Changed / Fixed groups.
pub fn changelog<M: 'static>(releases: impl IntoIterator<Item = Release>) -> Node<M> {
    let p = cx::palette();
    let releases: Vec<Release> = releases.into_iter().collect();
    let last = releases.len().saturating_sub(1);
    col(releases.into_iter().enumerate().map(|(i, r)| {
        let mut head: Vec<Node<M>> = vec![heading(r.version).into()];
        if r.latest {
            head.push(badge("Latest").accent().into());
        }
        head.push(hfill());
        head.push(caption(r.date).dim().into());
        let mut items: Vec<Node<M>> = vec![row(head).align_center().gap(10.0).fill_w().into()];
        if let Some(n) = r.note {
            items.push(text(n).dim().fill_w().into());
        }
        for (name, lines) in r.groups {
            let c = match name {
                "Added" => p.success,
                "Fixed" => p.warning,
                _ => p.accent,
            };
            items.push(
                row([tone::dot::<M>(c, 7.0), text(name).label().color(c).into()])
                    .gap(7.0)
                    .align_center()
                    .into(),
            );
            items.push(
                col(lines.into_iter().map(|l| {
                    row([
                        Node::from(col([tone::dot::<M>(p.border_strong, 4.0)]).pad_xy(2.0, 8.0)),
                        text(l).fill_w().into(),
                    ])
                    .gap(10.0)
                    .fill_w()
                }))
                .gap(2.0)
                .pad_xy(14.0, 0.0)
                .fill_w()
                .into(),
            );
        }
        let mut block: Vec<Node<M>> = vec![col(items).gap(8.0).fill_w().into()];
        if i != last {
            block.push(rule());
        }
        col(block).gap(20.0).fill_w()
    }))
    .gap(20.0)
    .fill_w()
    .into()
}

// -------------------------------------------------------- activity feed

/// One line of an [`activity_feed`].
pub struct Activity {
    who: String,
    verb: String,
    target: String,
    when: String,
}

/// "Mira commented on Q4 plan, 5 min ago": `activity("Mira Okafor", "commented on", "Q4 plan", "5 min ago")`.
pub fn activity(
    who: impl Into<String>,
    verb: impl Into<String>,
    target: impl Into<String>,
    when: impl Into<String>,
) -> Activity {
    Activity {
        who: who.into(),
        verb: verb.into(),
        target: target.into(),
        when: when.into(),
    }
}

/// An avatar per line, the sentence in two weights and the time on the right.
pub fn activity_feed<M: 'static>(items: impl IntoIterator<Item = Activity>) -> Node<M> {
    col(items.into_iter().map(|a| {
        row([
            Node::from(super::people::avatar(a.who.clone()).size(30.0)),
            row([
                nn(text(a.who).label()),
                nn(text(a.verb).dim()),
                nn(text(a.target).label()),
            ])
            .gap(5.0)
            .into(),
            hfill(),
            caption(a.when).dim().into(),
        ])
        .gap(12.0)
        .align_center()
        .fill_w()
        .pad_xy(0.0, 6.0)
    }))
    .fill_w()
    .into()
}

// --------------------------------------------------- notification centre

/// One entry of a [`notification_center`].
pub struct Notice {
    id: u64,
    title: String,
    body: String,
    when: String,
    unread: bool,
    tone: Tone,
}

/// A notification with a stable id: `notice(1, "Build failed").body("main, 3 tests").when("2 min").unread()`.
pub fn notice(id: u64, title: impl Into<String>) -> Notice {
    Notice {
        id,
        title: title.into(),
        body: String::new(),
        when: String::new(),
        unread: false,
        tone: Tone::Neutral,
    }
}

impl Notice {
    /// The detail line.
    pub fn body(mut self, s: impl Into<String>) -> Self {
        self.body = s.into();
        self
    }
    /// When it arrived, in words.
    pub fn when(mut self, s: impl Into<String>) -> Self {
        self.when = s.into();
        self
    }
    /// Not read yet.
    pub fn unread(mut self) -> Self {
        self.unread = true;
        self
    }
    /// Tint the leading marker.
    pub fn tone(mut self, t: Tone) -> Self {
        self.tone = t;
        self
    }
}

type OnId<S> = Arc<dyn Fn(&mut S, u64) + Send + Sync>;
type OnAll<S> = Arc<dyn Fn(&mut S) + Send + Sync>;

/// The notification list. See [`notification_center`].
pub struct Notices<S> {
    items: Vec<Notice>,
    on_read: Option<OnId<S>>,
    on_read_all: Option<OnAll<S>>,
    width: f32,
}

/// A panel of notifications: unread count, "Mark all read", and a click on a row marks it read.
/// You own the list; the closures update it.
pub fn notification_center<S: 'static>(items: impl IntoIterator<Item = Notice>) -> Notices<S> {
    Notices {
        items: items.into_iter().collect(),
        on_read: None,
        on_read_all: None,
        width: 380.0,
    }
}

impl<S: 'static> Notices<S> {
    /// Called with a notification's id when its row is clicked.
    pub fn on_read(mut self, f: impl Fn(&mut S, u64) + Send + Sync + 'static) -> Self {
        self.on_read = Some(Arc::new(f));
        self
    }
    /// Called when "Mark all read" is pressed.
    pub fn on_read_all(mut self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.on_read_all = Some(Arc::new(f));
        self
    }
    /// Panel width in pixels (default 380).
    pub fn w(mut self, px: f32) -> Self {
        self.width = px;
        self
    }
}

impl<S: 'static> From<Notices<S>> for Node<crate::Act<S>> {
    fn from(n: Notices<S>) -> Self {
        type M<S> = crate::Act<S>;
        let p = cx::palette();
        let unread = n.items.iter().filter(|i| i.unread).count();
        let mut head: Vec<Node<M<S>>> = vec![text("Notifications").sub().into()];
        if unread > 0 {
            head.push(badge(unread).accent().into());
        }
        head.push(hfill());
        if let (Some(f), true) = (n.on_read_all.clone(), unread > 0) {
            head.push(button("Mark all read").ghost().act(move |s| f(s)).into());
        }
        let mut rows: Vec<Node<M<S>>> = vec![
            Node::from(
                row(head)
                    .align_center()
                    .gap(10.0)
                    .fill_w()
                    .pad_xy(16.0, 12.0),
            ),
            rule(),
        ];
        if n.items.is_empty() {
            rows.push(
                empty_state(
                    Art::Inbox,
                    "You are all caught up",
                    "New activity shows up here.",
                )
                .into(),
            );
        }
        let last = n.items.len().saturating_sub(1);
        for (i, it) in n.items.into_iter().enumerate() {
            let c = match it.tone {
                Tone::Neutral => p.accent,
                t => tone_color(t),
            };
            let marker: Node<M<S>> = if it.unread {
                col([tone::dot::<M<S>>(c, 8.0)])
                    .pad_xy(0.0, 5.0)
                    .w(8.0)
                    .into()
            } else {
                col(Vec::<Node<M<S>>>::new()).w(8.0).into()
            };
            let mut body: Vec<Node<M<S>>> = vec![row([
                if it.unread {
                    nn(text(it.title).label().fill_w())
                } else {
                    nn(text(it.title).dim().fill_w())
                },
                caption(it.when).dim().into(),
            ])
            .gap(8.0)
            .fill_w()
            .into()];
            if !it.body.is_empty() {
                body.push(text(it.body).dim().fill_w().into());
            }
            let line = row([marker, col(body).gap(2.0).fill_w().into()])
                .gap(12.0)
                .fill_w()
                .pad_xy(16.0, 11.0);
            let id = it.id;
            let el: Node<M<S>> = match n.on_read.clone() {
                Some(f) => Node::from(tapped(line, move |s| f(s, id)).radius(0.0).into_element()),
                None => line.into(),
            };
            rows.push(el);
            if i != last {
                rows.push(rule());
            }
        }
        col(rows).card().pad(0.0).w(n.width).into()
    }
}

#[allow(unused)]
fn _assert(_: Flex<()>) {}
