//! Command bar and search with results.

use super::*;
use crate::{
    blocks::local,
    dsl::{caption, input, rule, text},
    icons::{icon, Icon},
    palette,
    view::{col, row, View},
    Act, Shortcuts,
};
use std::sync::Arc;

/// One result a [`command_bar`] can show.
#[derive(Clone)]
pub struct Hit {
    group: String,
    title: String,
    sub: String,
    keys: String,
}

/// A result in a group: `hit("Actions", "New project").keys("Ctrl+N")`.
pub fn hit(group: impl Into<String>, title: impl Into<String>) -> Hit {
    Hit {
        group: group.into(),
        title: title.into(),
        sub: String::new(),
        keys: String::new(),
    }
}

impl Hit {
    /// A dim line after the title (a path, a description).
    pub fn detail(mut self, s: impl Into<String>) -> Self {
        self.sub = s.into();
        self
    }
    /// A shortcut hint shown on the right.
    pub fn keys(mut self, s: impl Into<String>) -> Self {
        self.keys = s.into();
        self
    }
}

/// The inline command bar. See [`command_bar`].
pub struct Bar<S> {
    hits: Vec<Hit>,
    hint: String,
    on_pick: Option<Arc<dyn Fn(&mut S, usize) + Send + Sync>>,
    rows: usize,
}

/// A search field with ranked, grouped results under it. Up and Down move the selection, Enter or a
/// click picks. The query and selection live in the view; `on_pick` gets the index into the list
/// you passed. (The window-wide palette on `mod+k` is separate and always there.)
pub fn command_bar<S: 'static>(hits: impl IntoIterator<Item = Hit>) -> Bar<S> {
    Bar {
        hits: hits.into_iter().collect(),
        hint: "Search or run a command".into(),
        on_pick: None,
        rows: 7,
    }
}

impl<S: 'static> Bar<S> {
    /// Placeholder text.
    pub fn hint(mut self, s: impl Into<String>) -> Self {
        self.hint = s.into();
        self
    }
    /// Called with the picked result's index.
    pub fn on_pick(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on_pick = Some(Arc::new(f));
        self
    }
    /// The most results to list (default 7).
    pub fn rows(mut self, n: usize) -> Self {
        self.rows = n.max(1);
        self
    }
}

impl<S: 'static> From<Bar<S>> for Node<Act<S>> {
    fn from(b: Bar<S>) -> Self {
        type M<S> = Act<S>;
        let query = local(String::new());
        let sel = local(0usize);
        let q = query.get();
        let titles: Vec<String> = b.hits.iter().map(|h| h.title.clone()).collect();
        let mut order = if q.trim().is_empty() {
            (0..b.hits.len()).collect()
        } else {
            palette::rank(&q, &titles)
        };
        order.truncate(b.rows);
        let cur = sel.get().min(order.len().saturating_sub(1));
        let n = order.len();
        let pick = b.on_pick.clone();
        let chosen = order.clone();
        let choose = move |s: &mut S| {
            if let (Some(&i), Some(f)) = (chosen.get(sel.get().min(n.saturating_sub(1))), &pick) {
                f(s, i);
            }
        };
        let field = row([
            nn(icon(Icon::Search).size(16.0).dim()),
            input(q.clone())
                .hint(b.hint)
                .local(&query)
                .fill_w()
                .enter(Act::new(choose))
                .into(),
        ])
        .gap(10.0)
        .align_center()
        .pad_xy(14.0, 10.0)
        .fill_w();
        let mut items: Vec<Node<M<S>>> = vec![field.into(), rule()];
        if order.is_empty() {
            items.push(
                empty_state(Art::Search, "No matches", "Try fewer or different words.").into(),
            );
        }
        let mut last_group: Option<&str> = None;
        for (pos, &i) in order.iter().enumerate() {
            let h = &b.hits[i];
            if q.trim().is_empty() && last_group != Some(h.group.as_str()) {
                items.push(caption(h.group.clone()).dim().pad_xy(14.0, 6.0).into());
            }
            last_group = Some(h.group.as_str());
            let mut line: Vec<Node<M<S>>> = vec![text(h.title.clone()).into()];
            if !h.sub.is_empty() {
                line.push(caption(h.sub.clone()).dim().into());
            }
            line.push(hfill());
            if !h.keys.is_empty() {
                line.push(caption(h.keys.clone()).dim().into());
            }
            let pick = b.on_pick.clone();
            let row_el = row(line)
                .gap(10.0)
                .align_center()
                .pad_xy(14.0, 8.0)
                .fill_w();
            items.push(Node::from(
                tapped(row_el, move |s| {
                    sel.set(pos);
                    if let Some(f) = &pick {
                        f(s, i);
                    }
                })
                .selected(pos == cur)
                .radius(6.0)
                .into_element(),
            ));
        }
        let down = move |_: &mut S| sel.set((sel.get() + 1).min(n.saturating_sub(1)));
        let up = move |_: &mut S| sel.set(sel.get().saturating_sub(1));
        let node: Node<M<S>> = col(items)
            .gap(1.0)
            .card()
            .pad(0.0)
            .w(560.0)
            .elevated()
            .into();
        node.shortcut("down", down).shortcut("up", up)
    }
}
