//! Breadcrumbs, page headers and step trackers.

use super::*;
use crate::{
    dsl::{heading, rule, text},
    icons::{icon, Icon},
    view::{col, row, View},
};
use alloy_ui_layout::Length;
use std::sync::Arc;

/// Breadcrumbs under construction. See [`breadcrumb`].
pub struct Crumbs<S> {
    items: Vec<String>,
    on: Option<Arc<dyn Fn(&mut S, usize) + Send + Sync>>,
}

/// A path: `breadcrumb(["Projects", "Alloy", "Settings"]).act(|s, i| s.go_up_to(i))`. Earlier
/// crumbs are links, the last is the current page.
pub fn breadcrumb<S: 'static>(items: impl IntoIterator<Item = impl Into<String>>) -> Crumbs<S> {
    Crumbs {
        items: items.into_iter().map(Into::into).collect(),
        on: None,
    }
}

impl<S: 'static> Crumbs<S> {
    /// Called with the index of the crumb that was clicked.
    pub fn act(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
}

impl<S: 'static> From<Crumbs<S>> for Node<crate::Act<S>> {
    fn from(c: Crumbs<S>) -> Self {
        type M<S> = crate::Act<S>;
        let last = c.items.len().saturating_sub(1);
        let mut out: Vec<Node<M<S>>> = Vec::new();
        for (i, name) in c.items.into_iter().enumerate() {
            if i > 0 {
                out.push(icon(Icon::ChevronRight).size(12.0).dim().into());
            }
            if i == last {
                out.push(text(name).label().into());
            } else {
                let label = text(name).dim().pad_xy(5.0, 2.0);
                out.push(match c.on.clone() {
                    Some(f) => {
                        Node::from(tapped(label, move |s| f(s, i)).radius(4.0).into_element())
                    }
                    None => label.into(),
                });
            }
        }
        row(out).gap(2.0).align_center().into()
    }
}

/// A page header under construction. See [`page_header`].
pub struct PageHeader<M> {
    title: String,
    subtitle: Option<String>,
    crumbs: Option<Node<M>>,
    badge: Option<Node<M>>,
    actions: Vec<Node<M>>,
}

/// The top of a page: optional breadcrumbs, the title with a badge, actions on the right, a
/// sentence of context, then a hairline:
/// `page_header("Deployments").crumbs(breadcrumb(["Projects", "Alloy"])).actions([button("New")])`.
pub fn page_header<M>(title: impl Into<String>) -> PageHeader<M> {
    PageHeader {
        title: title.into(),
        subtitle: None,
        crumbs: None,
        badge: None,
        actions: Vec::new(),
    }
}

impl<M> PageHeader<M> {
    /// A sentence under the title.
    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }
    /// Breadcrumbs above the title.
    pub fn crumbs(mut self, v: impl Into<Node<M>>) -> Self {
        self.crumbs = Some(v.into());
        self
    }
    /// A badge beside the title.
    pub fn badge(mut self, v: impl Into<Node<M>>) -> Self {
        self.badge = Some(v.into());
        self
    }
    /// Buttons on the right.
    pub fn actions<V: Into<Node<M>>>(mut self, v: impl IntoIterator<Item = V>) -> Self {
        self.actions = v.into_iter().map(Into::into).collect();
        self
    }
}

impl<M: 'static> From<PageHeader<M>> for Node<M> {
    fn from(h: PageHeader<M>) -> Self {
        let mut items: Vec<Node<M>> = Vec::new();
        if let Some(c) = h.crumbs {
            items.push(c);
        }
        let mut title: Vec<Node<M>> = vec![heading(h.title).into()];
        if let Some(b) = h.badge {
            title.push(b);
        }
        title.push(hfill());
        if !h.actions.is_empty() {
            title.push(row(h.actions).gap(8.0).align_center().into());
        }
        items.push(row(title).gap(10.0).align_center().fill_w().into());
        if let Some(s) = h.subtitle {
            items.push(text(s).dim().fill_w().into());
        }
        items.push(crate::dsl::gap(0.0, 4.0));
        items.push(rule());
        col(items).gap(8.0).fill_w().into()
    }
}

/// A horizontal progress tracker: numbered circles joined by hairlines, a tick for steps done,
/// an accent ring for the current one: `tracker(["Cart", "Shipping", "Payment"], 1)`.
pub fn tracker<M: 'static>(
    steps: impl IntoIterator<Item = impl Into<String>>,
    current: usize,
) -> Node<M> {
    let steps: Vec<String> = steps.into_iter().map(Into::into).collect();
    paint(Length::Fill(1), 62.0, move |f| {
        let p = f.palette();
        let n = steps.len().max(1);
        let w = f.size().width;
        let x = |i: usize| w * (i as f32 + 0.5) / n as f32;
        let (r, y) = (12.0, 14.0);
        for i in 0..n.saturating_sub(1) {
            let done = i < current;
            f.line(
                pt(x(i) + r + 6.0, y),
                pt(x(i + 1) - r - 6.0, y),
                2.0,
                if done { p.accent } else { p.border },
            );
        }
        for (i, name) in steps.iter().enumerate() {
            let c = pt(x(i), y);
            if i < current {
                f.circle(c, r, p.accent);
                f.line(
                    pt(c.x - 5.0, c.y + 0.5),
                    pt(c.x - 1.5, c.y + 4.0),
                    2.2,
                    p.on_accent,
                );
                f.line(
                    pt(c.x - 1.5, c.y + 4.0),
                    pt(c.x + 5.0, c.y - 3.5),
                    2.2,
                    p.on_accent,
                );
            } else {
                let on = i == current;
                f.circle(c, r, p.surface);
                f.stroke_circle(
                    c,
                    r,
                    if on { 2.0 } else { 1.0 },
                    if on { p.accent } else { p.border_strong },
                );
                let t = (i + 1).to_string();
                let tw = f.text_width(&t, 12.0);
                f.text_weight(
                    pt(c.x - tw / 2.0, c.y - 7.5),
                    t,
                    12.0,
                    600,
                    if on { p.accent } else { p.text_secondary },
                );
            }
            let size = 13.0;
            let tw = f.text_width(name, size) * if i == current { 1.06 } else { 1.0 };
            f.text_weight(
                pt(x(i) - tw / 2.0, y + r + 10.0),
                name.clone(),
                size,
                if i == current { 600 } else { 400 },
                if i <= current {
                    p.text
                } else {
                    p.text_secondary
                },
            );
        }
    })
}
