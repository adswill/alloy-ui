//! Empty states with small drawn illustrations.

use super::*;
use crate::{dsl::text, view::col};
use alloy_ui_layout::Length;
use alloy_ui_widgets::{Frame, Path};
use std::f32::consts::PI;

/// Which picture an [`empty_state`] shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Art {
    /// Nothing waiting: a tray.
    Inbox,
    /// A search with no hits.
    Search,
    /// An empty folder.
    Folder,
    /// No connection.
    Offline,
    /// No data yet.
    Chart,
}

/// An empty state under construction. See [`empty_state`].
pub struct Empty<M> {
    art: Art,
    title: String,
    body: String,
    action: Option<Node<M>>,
}

/// A centred picture, a title, one explaining sentence and optionally an action:
/// `empty_state(Art::Search, "No results", "Try another word.").action(button("Clear"))`.
pub fn empty_state<M>(art: Art, title: impl Into<String>, body: impl Into<String>) -> Empty<M> {
    Empty {
        art,
        title: title.into(),
        body: body.into(),
        action: None,
    }
}

impl<M> Empty<M> {
    /// The one thing to do next (usually a primary button).
    pub fn action(mut self, v: impl Into<Node<M>>) -> Self {
        self.action = Some(v.into());
        self
    }
}

fn card_at(f: &mut Frame, x: f32, y: f32, w: f32, h: f32, fill: Color, line: Color) {
    let r = alloy_ui_core::Rectangle::new(pt(x, y), alloy_ui_core::Size::new(w, h));
    f.round_rect(r, 8.0, fill);
    f.stroke_rect(r, 8.0, 1.0, line);
}

fn bar(f: &mut Frame, x: f32, y: f32, w: f32, c: Color) {
    f.round_rect(
        alloy_ui_core::Rectangle::new(pt(x, y), alloy_ui_core::Size::new(w, 6.0)),
        3.0,
        c,
    );
}

fn picture(f: &mut Frame, art: Art) {
    let p = f.palette();
    let cx = f.size().width / 2.0;
    let (line, soft, ink) = (p.border_strong, p.border, p.accent);
    match art {
        Art::Inbox | Art::Search => {
            card_at(f, cx - 52.0, 14.0, 104.0, 74.0, p.background, soft);
            card_at(f, cx - 60.0, 26.0, 104.0, 74.0, p.surface, line);
            bar(f, cx - 48.0, 42.0, 58.0, soft);
            bar(f, cx - 48.0, 56.0, 76.0, soft);
            bar(f, cx - 48.0, 70.0, 36.0, soft);
            if art == Art::Search {
                let c = pt(cx + 34.0, 78.0);
                f.circle(c, 17.0, p.surface);
                f.stroke_circle(c, 17.0, 3.0, ink);
                f.line(
                    pt(c.x + 12.0, c.y + 12.0),
                    pt(c.x + 26.0, c.y + 26.0),
                    4.0,
                    ink,
                );
            } else {
                let c = pt(cx + 42.0, 40.0);
                f.circle(c, 13.0, ink);
                f.line(
                    pt(c.x - 5.0, c.y),
                    pt(c.x - 1.0, c.y + 4.0),
                    2.5,
                    p.on_accent,
                );
                f.line(
                    pt(c.x - 1.0, c.y + 4.0),
                    pt(c.x + 6.0, c.y - 4.0),
                    2.5,
                    p.on_accent,
                );
            }
        }
        Art::Folder => {
            let tab = Path::rounded_rect(
                alloy_ui_core::Rectangle::new(
                    pt(cx - 58.0, 24.0),
                    alloy_ui_core::Size::new(48.0, 22.0),
                ),
                7.0,
            );
            f.fill(&tab, p.border);
            card_at(f, cx - 58.0, 36.0, 116.0, 66.0, p.surface, line);
            f.line(pt(cx - 50.0, 52.0), pt(cx + 50.0, 52.0), 1.0, soft);
            f.circle(pt(cx + 38.0, 82.0), 4.0, ink);
        }
        Art::Offline => {
            let c = pt(cx, 82.0);
            for (i, r) in [18.0, 36.0, 54.0].into_iter().enumerate() {
                let col = if i == 0 { line } else { soft };
                f.stroke(&Path::new().arc(c, r, PI * 1.2, PI * 1.8), 4.0, col);
            }
            f.circle(c, 5.0, line);
            f.line(pt(cx - 46.0, 18.0), pt(cx + 46.0, 96.0), 4.0, p.surface);
            f.line(pt(cx - 46.0, 18.0), pt(cx + 46.0, 96.0), 2.5, ink);
        }
        Art::Chart => {
            card_at(f, cx - 62.0, 14.0, 124.0, 88.0, p.surface, line);
            for (i, h) in [20.0, 34.0, 26.0, 46.0].into_iter().enumerate() {
                let x = cx - 44.0 + i as f32 * 24.0;
                let col = if i == 3 { ink } else { soft };
                f.round_rect(
                    alloy_ui_core::Rectangle::new(
                        pt(x, 90.0 - h),
                        alloy_ui_core::Size::new(14.0, h),
                    ),
                    3.0,
                    col,
                );
            }
        }
    }
}

impl<M: 'static> From<Empty<M>> for Node<M> {
    fn from(e: Empty<M>) -> Self {
        let art = e.art;
        let mut items: Vec<Node<M>> = vec![
            paint(Length::Fixed(200.0), 112.0, move |f| picture(f, art)),
            vspace_n(6.0),
            text(e.title).sub().into(),
            text(e.body).dim().into(),
        ];
        if let Some(a) = e.action {
            items.push(vspace_n(8.0));
            items.push(a);
        }
        col(items).gap(6.0).align_center().pad(16.0).fill_w().into()
    }
}

fn vspace_n<M: 'static>(h: f32) -> Node<M> {
    crate::dsl::gap(0.0, h)
}
