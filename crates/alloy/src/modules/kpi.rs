//! Stat and KPI cards.

use super::*;
use crate::{
    dsl::{caption, heading, text},
    icons::{icon, Icon},
    rich::sparkline,
    view::{col, row, View},
};

/// A KPI card under construction. See [`kpi`].
pub struct Kpi {
    label: String,
    value: String,
    delta: Option<f32>,
    lower_better: bool,
    hint: Option<String>,
    trend: Vec<f32>,
}

/// A number under its label with an optional change and trend line:
/// `kpi("Revenue", "$48.2k").delta(12.4).hint("vs last week").trend(series(24))`.
pub fn kpi(label: impl Into<String>, value: impl ToString) -> Kpi {
    Kpi {
        label: label.into(),
        value: value.to_string(),
        delta: None,
        lower_better: false,
        hint: None,
        trend: Vec::new(),
    }
}

impl Kpi {
    /// Percent change. Positive reads green unless [`lower_is_better`](Self::lower_is_better).
    pub fn delta(mut self, percent: f32) -> Self {
        self.delta = Some(percent);
        self
    }
    /// A falling number is good news (latency, churn, error rate).
    pub fn lower_is_better(mut self) -> Self {
        self.lower_better = true;
        self
    }
    /// Small text under the number: what the change compares to.
    pub fn hint(mut self, s: impl Into<String>) -> Self {
        self.hint = Some(s.into());
        self
    }
    /// A sparkline of recent values, coloured by the change.
    pub fn trend(mut self, values: impl IntoIterator<Item = f32>) -> Self {
        self.trend = values.into_iter().collect();
        self
    }
}

impl<M: 'static> From<Kpi> for Node<M> {
    fn from(k: Kpi) -> Self {
        let p = cx::palette();
        let tone = match k.delta {
            Some(d) if d.abs() < 0.05 => p.text_secondary,
            Some(d) if (d > 0.0) != k.lower_better => p.success,
            Some(_) => p.danger,
            None => p.accent,
        };
        let mut top: Vec<Node<M>> = vec![caption(k.label).into(), hfill()];
        if let Some(d) = k.delta {
            let mut tag: Vec<Node<M>> = Vec::new();
            if d.abs() >= 0.05 {
                let arrow = if d >= 0.0 {
                    Icon::ArrowUp
                } else {
                    Icon::ArrowDown
                };
                tag.push(icon(arrow).size(12.0).color(tone).into());
            }
            tag.push(text(format!("{:.1}%", d.abs())).label().color(tone).into());
            top.push(row(tag).gap(2.0).align_center().into());
        }
        let mut items: Vec<Node<M>> = vec![
            row(top).align_center().fill_w().into(),
            heading(k.value).into(),
        ];
        if k.hint.is_some() || !k.trend.is_empty() {
            let mut bottom: Vec<Node<M>> = vec![];
            if let Some(h) = k.hint {
                bottom.push(caption(h).dim().into());
            }
            bottom.push(hfill());
            if !k.trend.is_empty() {
                bottom.push(sparkline(k.trend).color(tone).w(88.0).into());
            }
            items.push(row(bottom).align_center().fill_w().into());
        }
        col(items)
            .gap(6.0)
            .card()
            .pad_xy(18.0, 14.0)
            .fill_w()
            .into()
    }
}

/// A row of KPI cards that share the width equally.
pub fn kpi_row<M: 'static>(items: impl IntoIterator<Item = Kpi>) -> crate::view::Flex<M> {
    row(items.into_iter().map(Node::<M>::from)).gap(12.0)
}
