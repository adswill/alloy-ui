//! Layout helpers shared by every page.
use crate::Studio;
use alloy_ui::prelude::*;

pub type N = Node<Act<Studio>>;

/// A page title with a one-line description and optional actions on the right.
pub fn header(title: impl Into<String>, sub: impl Into<String>, actions: impl Into<N>) -> N {
    row![
        col![heading(title), text(sub).dim()].gap(3.0).fill_w(),
        actions.into(),
    ]
    .align_center()
    .gap(12.0)
    .into()
}

/// A titled card: a small label above the content.
pub fn panel(title: impl Into<String>, body: impl Into<N>) -> N {
    col![caption(title).dim(), body.into()]
        .gap(10.0)
        .card()
        .pad(16.0)
        .fill_w()
        .into()
}

/// One key figure: label, value, a coloured change badge and a sparkline.
pub fn kpi_card(label: &str, value: &str, delta: f32, good_up: bool, spark: Vec<f32>) -> N {
    let up = delta >= 0.0;
    let good = up == good_up;
    let b = badge(format!("{}{:.1}%", if up { "+" } else { "" }, delta));
    let b = if good { b.success() } else { b.danger() };
    col![
        row![caption(label).dim().fill_w(), b.dot()].align_center(),
        row![
            heading(value).fill_w(),
            sparkline(spark)
                .color(if good {
                    palette().success
                } else {
                    palette().danger
                })
                .w(88.0),
        ]
        .align_center()
        .gap(8.0),
    ]
    .gap(8.0)
    .card()
    .pad(16.0)
    .fill_w()
    .into()
}

/// A scrolling page with the standard gutters.
pub fn page(body: impl Into<N>) -> N {
    scroll(col![body.into()].pad(24.0).gap(18.0)).fill().into()
}

/// A page that manages its own scrolling (grids, editors, boards).
pub fn fixed(body: impl Into<N>) -> N {
    col![body.into()].pad(24.0).gap(14.0).fill().into()
}
