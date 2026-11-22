//! Small controls built from existing widgets: [`pagination`], [`rating`], [`accordion`] and
//! [`combobox`].
//!
//! ```ignore
//! pagination(s.page, 12).act(|s, p| s.page = p)      // 1-based pages
//! rating(s.stars, 5).act(|s, n| s.stars = n)
//! ```

use super::*;
use crate::{
    dsl::{button, input, text},
    local,
    view::{col, row},
};
use std::sync::Arc;

type Pick<M> = Option<Box<dyn Fn(usize) -> M>>;

macro_rules! picker {
    ($name:ident) => {
        impl<M: 'static> $name<M> {
            /// Called with the chosen number or index.
            pub fn on(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
                self.on = Some(Box::new(f));
                self
            }
        }
        impl<S: 'static> $name<Act<S>> {
            /// Closure style: `.act(|state, picked| ..)`.
            pub fn act(self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
                let f = Arc::new(f);
                self.on(move |v| {
                    let f = f.clone();
                    Act::new(move |s: &mut S| f(s, v))
                })
            }
        }
    };
}

// ------------------------------------------------------------------ pagination

/// Page buttons under construction. See [`pagination`].
pub struct Pager<M> {
    page: usize,
    pages: usize,
    on: Pick<M>,
}

/// Which entries a pager shows: the first and last page, the current one and its neighbours,
/// with `None` standing for a gap (`1 … 4 5 6 … 12`). `page` is clamped to `1..=pages`.
pub fn page_window(page: usize, pages: usize) -> Vec<Option<usize>> {
    let page = page.clamp(1, pages.max(1));
    let mut out = Vec::new();
    let mut last = 0;
    for p in 1..=pages {
        if p == 1 || p == pages || p.abs_diff(page) <= 1 {
            if p > last + 1 {
                out.push(None);
            }
            out.push(Some(p));
            last = p;
        }
    }
    out
}

/// Previous / numbered / next buttons for `pages` pages with `page` (1-based) current:
/// `pagination(3, 12).act(|s, p| s.page = p)`.
pub fn pagination<M>(page: usize, pages: usize) -> Pager<M> {
    Pager {
        page: page.clamp(1, pages.max(1)),
        pages,
        on: None,
    }
}
picker!(Pager);

impl<M: Clone + 'static> From<Pager<M>> for Node<M> {
    fn from(p: Pager<M>) -> Self {
        let go = |label: String, to: usize, live: bool| -> Node<M> {
            let mut b = button::<M>(label).ghost();
            if to == p.page {
                b = b.primary();
            } else if let (true, Some(f)) = (live, &p.on) {
                b = b.on(f(to));
            }
            b.into()
        };
        let mut items: Vec<Node<M>> = Vec::new();
        items.push(go("‹".into(), p.page.saturating_sub(1), p.page > 1));
        for e in page_window(p.page, p.pages) {
            items.push(match e {
                Some(n) => go(n.to_string(), n, true),
                None => text("…").dim().into(),
            });
        }
        items.push(go("›".into(), p.page + 1, p.page < p.pages));
        row(items).gap(4.0).align_center().into()
    }
}

// ---------------------------------------------------------------------- rating

/// A star rating under construction. See [`rating`].
pub struct Stars<M> {
    value: usize,
    max: usize,
    on: Pick<M>,
}

/// `max` stars with `value` of them filled. Clicking star `n` reports `n` (1-based); without
/// `.on` or `.act` the stars are display only.
pub fn rating<M>(value: usize, max: usize) -> Stars<M> {
    Stars {
        value: value.min(max),
        max,
        on: None,
    }
}
picker!(Stars);

impl<M: Clone + 'static> From<Stars<M>> for Node<M> {
    fn from(s: Stars<M>) -> Self {
        let stars = (1..=s.max).map(|n| -> Node<M> {
            let label = if n <= s.value { "★" } else { "☆" };
            match &s.on {
                Some(f) => button::<M>(label).ghost().on(f(n)).into(),
                None => text(label).into(),
            }
        });
        row(stars.collect::<Vec<_>>()).gap(2.0).into()
    }
}

// ------------------------------------------------------------------- accordion

/// A section that opens and closes when its title is clicked. It remembers being open in
/// view-local state, so there is nothing to add to your app: `accordion("Advanced", body)`.
/// Use [`accordion_open`] for one that starts open.
#[track_caller]
pub fn accordion<S: 'static>(
    title: impl Into<String>,
    body: impl Into<Node<Act<S>>>,
) -> Node<Act<S>> {
    fold(local(false), title.into(), body.into())
}

/// Like [`accordion`] but open the first time it is shown.
#[track_caller]
pub fn accordion_open<S: 'static>(
    title: impl Into<String>,
    body: impl Into<Node<Act<S>>>,
) -> Node<Act<S>> {
    fold(local(true), title.into(), body.into())
}

fn fold<S: 'static>(open: crate::Local<bool>, title: String, body: Node<Act<S>>) -> Node<Act<S>> {
    let shown = open.get();
    let head = button(format!("{} {title}", if shown { "▾" } else { "▸" }))
        .ghost()
        .act(move |_: &mut S| open.toggle());
    if shown {
        col([Node::from(head), body]).gap(6.0).into()
    } else {
        head.into()
    }
}

/// A collapsible section for crowded pages: a header with a chevron over a body that opens and
/// closes with a size animation. Its open state is saved under `id` (like [`store_as`]), so it
/// survives rebuilds, page changes and, with persistence on, restarts, and two places using the
/// same `id` share it: `fold_section("advanced", "Advanced", body)`. Starts closed; see
/// [`fold_section_open`]. [`accordion`] keeps its call-site state and is unchanged.
#[track_caller]
pub fn fold_section<S: 'static>(
    id: impl std::fmt::Display,
    title: impl Into<String>,
    body: impl Into<Node<Act<S>>>,
) -> Node<Act<S>> {
    fold_section_with(id, false, title.into(), body.into())
}

/// Like [`fold_section`] but open the first time it is shown (when nothing is saved under `id`).
#[track_caller]
pub fn fold_section_open<S: 'static>(
    id: impl std::fmt::Display,
    title: impl Into<String>,
    body: impl Into<Node<Act<S>>>,
) -> Node<Act<S>> {
    fold_section_with(id, true, title.into(), body.into())
}

fn fold_section_with<S: 'static>(
    id: impl std::fmt::Display,
    start_open: bool,
    title: String,
    body: Node<Act<S>>,
) -> Node<Act<S>> {
    use crate::input::Interact;
    let open = crate::store_as(&format!("section.{id}"), move || start_open);
    let shown = open.get();
    let chevron = crate::icon(if shown {
        crate::Icon::ChevronDown
    } else {
        crate::Icon::ChevronRight
    })
    .dim();
    let head = row([Node::from(chevron), Node::from(text(title))])
        .gap(6.0)
        .tap(move |_: &mut S| open.toggle());
    let body: Node<Act<S>> = if shown {
        body
    } else {
        Node::from(row(Vec::<Node<Act<S>>>::new()))
    };
    col([head, crate::grow(body)]).gap(6.0).into()
}

// -------------------------------------------------------------------- combobox

/// A text field that suggests matching options as you type. See [`combobox`].
pub struct Combo<S> {
    options: Vec<String>,
    current: String,
    hint: String,
    max: usize,
    on: Option<Arc<dyn Fn(String) -> Act<S> + Send + Sync>>,
}

/// Type to filter `options` (case-insensitive, anywhere in the text) and click one to choose
/// it. `current` is the chosen value, shown while you are not typing:
/// `combobox(["Apple", "Banana"], &s.fruit).act(|s, v| s.fruit = v)`.
pub fn combobox<S: 'static>(
    options: impl IntoIterator<Item = impl Into<String>>,
    current: impl Into<String>,
) -> Combo<S> {
    Combo {
        options: options.into_iter().map(Into::into).collect(),
        current: current.into(),
        hint: String::new(),
        max: 6,
        on: None,
    }
}

impl<S: 'static> Combo<S> {
    /// Grey text shown in the empty field.
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = h.into();
        self
    }
    /// Most suggestions shown at once (default 6).
    pub fn max_shown(mut self, n: usize) -> Self {
        self.max = n;
        self
    }
    /// Called with the chosen option.
    pub fn on(mut self, f: impl Fn(String) -> Act<S> + Send + Sync + 'static) -> Self {
        self.on = Some(Arc::new(f));
        self
    }
    /// Closure style: `.act(|state, chosen| ..)`.
    pub fn act(self, f: impl Fn(&mut S, String) + Send + Sync + 'static) -> Self {
        let f = Arc::new(f);
        self.on(move |v| {
            let f = f.clone();
            Act::new(move |s: &mut S| f(s, v.clone()))
        })
    }
}

/// The options that contain `query` (ignoring case), in their original order, at most `max`.
pub fn suggestions<'a>(options: &'a [String], query: &str, max: usize) -> Vec<&'a str> {
    let q = query.trim().to_lowercase();
    options
        .iter()
        .filter(|o| !q.is_empty() && o.to_lowercase().contains(&q))
        .map(String::as_str)
        .take(max)
        .collect()
}

impl<S: 'static> From<Combo<S>> for Node<Act<S>> {
    #[track_caller]
    fn from(c: Combo<S>) -> Self {
        let (query, touched) = (local(String::new()), local(false));
        let shown = if touched.get() {
            query.get()
        } else {
            c.current.clone()
        };
        let field = input(shown).hint(c.hint.clone()).on(move |t: String| {
            Act::new(move |_: &mut S| {
                query.set(t.clone());
                touched.set(true);
            })
        });
        let mut items: Vec<Node<Act<S>>> = vec![field.into()];
        if touched.get() {
            for opt in suggestions(&c.options, &query.get(), c.max) {
                let pick = c.on.clone();
                let value = opt.to_string();
                items.push(
                    button(opt)
                        .ghost()
                        .fill_w()
                        .act(move |s: &mut S| {
                            query.set(String::new());
                            touched.set(false);
                            if let Some(f) = &pick {
                                (f(value.clone()).0)(s);
                            }
                        })
                        .into(),
                );
            }
        }
        col(items).gap(2.0).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{prelude::*, testing::Harness};

    #[test]
    fn window_collapses_gaps_and_clamps() {
        let shown = |p, n| page_window(p, n);
        assert_eq!(shown(1, 3), vec![Some(1), Some(2), Some(3)]);
        assert_eq!(
            shown(6, 12),
            vec![Some(1), None, Some(5), Some(6), Some(7), None, Some(12)]
        );
        assert_eq!(shown(99, 5), vec![Some(1), None, Some(4), Some(5)]);
        assert_eq!(shown(1, 0), Vec::<Option<usize>>::new());
        assert_eq!(shown(1, 1), vec![Some(1)]);
    }

    #[test]
    fn pagination_clicks_move_the_page_and_ends_are_inert() {
        let app = sketch(1usize, |p| {
            col![
                text(format!("page={p}")),
                pagination(*p, 5).act(|s: &mut usize, n| *s = n)
            ]
        });
        let mut ui = Harness::new(app, 400.0, 120.0);
        ui.expect("page=1").click("›").expect("page=2");
        ui.click("5").expect("page=5").click("›").expect("page=5");
        ui.click("‹").expect("page=4");
        ui.click("1").click("‹").expect("page=1");
    }

    #[test]
    fn rating_sets_stars_and_zero_max_is_empty() {
        let app = sketch(0usize, |n| {
            col![
                text(format!("stars={n}")),
                rating(*n, 5).act(|s: &mut usize, v| *s = v),
                rating::<Act<usize>>(0, 0)
            ]
        });
        let mut ui = Harness::new(app, 400.0, 120.0);
        ui.expect("stars=0");
        let at = ui.position("☆").expect("a star");
        ui.click_at(at.x + 1.0, at.y + 1.0);
        ui.expect("stars=1").expect("★");
    }

    #[test]
    fn accordion_opens_and_closes_on_click() {
        let mut ui = Harness::new(
            sketch((), |_| {
                col![
                    accordion("More", text("hidden body")),
                    accordion_open("Open", text("shown body"))
                ]
            }),
            400.0,
            200.0,
        );
        ui.expect_no("hidden body").expect("shown body");
        ui.click("▸ More")
            .expect("hidden body")
            .click("▾ More")
            .expect_no("hidden body");
        ui.click("▾ Open").expect_no("shown body");
    }

    #[test]
    fn tinted_button_lightens_on_hover_and_darkens_when_pressed() {
        use alloy_ui_core::{Color, DrawCommand};
        let green = Color::hex(0x2E9E5B);
        let mut ui = Harness::new(
            sketch(0u8, move |_| {
                button("Play").tint(green).act(|n: &mut u8| *n += 1)
            }),
            200.0,
            80.0,
        );
        // The button is the smallest filled quad on screen (the window background is larger).
        let fill = |ui: &Harness<_>| {
            ui.draw_list()
                .0
                .iter()
                .filter_map(|c| match c {
                    DrawCommand::Quad { fill, bounds, .. } if fill.a > 0.5 => {
                        Some((bounds.size.width * bounds.size.height, *fill))
                    }
                    _ => None,
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, f)| f)
        };
        let sum = |c: Color| c.r + c.g + c.b;
        ui.advance(400);
        let rest = fill(&ui).expect("a filled button");
        assert!(
            (sum(rest) - sum(green)).abs() < 0.02,
            "rest colour is the tint"
        );
        ui.hover("Play").advance(16).advance(400);
        assert!(
            sum(fill(&ui).unwrap()) > sum(rest) + 0.1,
            "hover is lighter"
        );
        ui.mouse_down().advance(16).advance(400);
        assert!(sum(fill(&ui).unwrap()) < sum(rest), "pressed is darker");
    }

    #[test]
    fn suggestions_match_anywhere_ignore_case_and_cap() {
        let o: Vec<String> = ["Apple", "Banana", "Blueberry", "Pineapple"]
            .map(String::from)
            .into();
        assert_eq!(suggestions(&o, "an", 6), ["Banana"]);
        assert_eq!(suggestions(&o, "APPLE", 6), ["Apple", "Pineapple"]);
        assert_eq!(suggestions(&o, "b", 1), ["Banana"]);
        assert!(suggestions(&o, "", 6).is_empty() && suggestions(&o, "zzz", 6).is_empty());
    }

    #[test]
    fn combobox_filters_while_typing_and_picks_on_click() {
        let app = sketch("Apple".to_string(), |s: &String| {
            col![
                text(format!("picked={s}")),
                combobox(["Apple", "Banana", "Blueberry"], s.clone())
                    .hint("Fruit")
                    .act(|s: &mut String, v| *s = v)
            ]
        });
        let mut ui = Harness::new(app, 300.0, 220.0);
        ui.expect("picked=Apple").expect_no("Banana");
        ui.fill("Apple", "bl")
            .expect("Blueberry")
            .expect_no("Banana");
        ui.click("Blueberry")
            .expect("picked=Blueberry")
            .expect_no("Banana");
    }
}
