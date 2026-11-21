//! Pricing and plan picker.

use super::*;
use crate::{
    blocks::local,
    choose,
    dsl::{button, caption, heading, rule, text},
    icons::{icon, Icon},
    rich::badge,
    view::{col, row},
    Act,
};
use alloy_ui_core::style::Padding;
use alloy_ui_layout::Length;
use alloy_ui_widgets as w;
use std::sync::Arc;

/// One plan in a [`plan_picker`].
pub struct Plan {
    name: String,
    monthly: f32,
    about: String,
    features: Vec<String>,
    recommended: bool,
}

/// A plan with its monthly price (0 is free): `plan("Team", 12.0).features(["10 seats"]).recommended()`.
pub fn plan(name: impl Into<String>, monthly: f32) -> Plan {
    Plan {
        name: name.into(),
        monthly,
        about: String::new(),
        features: Vec::new(),
        recommended: false,
    }
}

impl Plan {
    /// A sentence on who it is for.
    pub fn about(mut self, s: impl Into<String>) -> Self {
        self.about = s.into();
        self
    }
    /// What is included, one line each.
    pub fn features(mut self, f: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.features = f.into_iter().map(Into::into).collect();
        self
    }
    /// Preselected and marked "Most popular".
    pub fn recommended(mut self) -> Self {
        self.recommended = true;
        self
    }
}

/// The plan cards. See [`plan_picker`].
pub struct Plans<S> {
    plans: Vec<Plan>,
    on_choose: Option<Arc<dyn Fn(&mut S, usize) + Send + Sync>>,
    yearly_discount: f32,
}

/// Cards side by side with a monthly/yearly switch. The chosen card gets an accent border; the
/// choice and the billing period live in the view: `plan_picker([..]).on_choose(|s, i| s.plan = i)`.
pub fn plan_picker<S: 'static>(plans: impl IntoIterator<Item = Plan>) -> Plans<S> {
    Plans {
        plans: plans.into_iter().collect(),
        on_choose: None,
        yearly_discount: 0.2,
    }
}

impl<S: 'static> Plans<S> {
    /// Called with the index of the plan whose button was pressed.
    pub fn on_choose(mut self, f: impl Fn(&mut S, usize) + Send + Sync + 'static) -> Self {
        self.on_choose = Some(Arc::new(f));
        self
    }
    /// The saving on yearly billing, 0 to 1 (default 0.2).
    pub fn yearly_discount(mut self, d: f32) -> Self {
        self.yearly_discount = d.clamp(0.0, 0.9);
        self
    }
}

fn price(v: f32) -> String {
    if v <= 0.0 {
        "Free".into()
    } else if v.fract() == 0.0 {
        format!("${v:.0}")
    } else {
        format!("${v:.2}")
    }
}

impl<S: 'static> From<Plans<S>> for Node<Act<S>> {
    fn from(pp: Plans<S>) -> Self {
        type M<S> = Act<S>;
        let p = cx::palette();
        let t = cx::theme();
        let start = pp.plans.iter().position(|x| x.recommended).unwrap_or(0);
        let chosen = local(start);
        let yearly = local(false);
        let sel = chosen.get();
        let pct = (pp.yearly_discount * 100.0).round();
        let switch: Node<M<S>> = choose([("Monthly", false), ("Yearly", true)], yearly.get())
            .local(&yearly)
            .into();
        let most = pp.plans.iter().map(|p| p.features.len()).max().unwrap_or(0);
        let cards: Vec<Node<M<S>>> = pp
            .plans
            .into_iter()
            .enumerate()
            .map(|(i, pl)| {
                let on = i == sel;
                let amount = if yearly.get() {
                    pl.monthly * (1.0 - pp.yearly_discount)
                } else {
                    pl.monthly
                };
                let mut head: Vec<Node<M<S>>> = vec![text(pl.name.clone()).label().into()];
                if pl.recommended {
                    head.push(badge("Most popular").accent().into());
                }
                let mut body: Vec<Node<M<S>>> = vec![
                    row(head).gap(8.0).align_center().into(),
                    row([
                        heading(price(amount)).into(),
                        if pl.monthly > 0.0 {
                            caption(if yearly.get() {
                                "per month, billed yearly"
                            } else {
                                "per month"
                            })
                            .dim()
                            .into()
                        } else {
                            hfill()
                        },
                    ])
                    .gap(8.0)
                    .align_center()
                    .into(),
                ];
                if !pl.about.is_empty() {
                    body.push(text(pl.about.clone()).dim().fill_w().into());
                }
                body.push(rule());
                for f in &pl.features {
                    body.push(
                        row([
                            icon(Icon::Check).size(14.0).accent().into(),
                            Node::from(text(f.clone()).fill_w()),
                        ])
                        .gap(8.0)
                        .align_center()
                        .into(),
                    );
                }
                if pl.features.len() < most {
                    body.push(crate::dsl::gap(
                        0.0,
                        28.0 * (most - pl.features.len()) as f32 - 10.0,
                    ));
                }
                let f = pp.on_choose.clone();
                let mut b = button(if on {
                    "Selected".to_string()
                } else {
                    format!("Choose {}", pl.name)
                })
                .fill_w()
                .act(move |s| {
                    chosen.set(i);
                    if let Some(f) = &f {
                        f(s, i);
                    }
                });
                if on {
                    b = b.primary();
                }
                body.push(b.into());
                let (bc, bw) = if on { (p.accent, 2.0) } else { (p.border, 1.0) };
                Node::from(
                    w::Container::new(Node::from(col(body).gap(10.0)).into_element())
                        .padding(Padding::all(20.0 - (bw - 1.0)))
                        .radius(t.radii.large)
                        .background(p.surface)
                        .border(bc, bw)
                        .width(Length::Fill(1))
                        .into_element(),
                )
            })
            .collect();
        col([
            Node::from(
                row([
                    hfill(),
                    caption(format!("Save {pct:.0}% yearly")).dim().into(),
                    switch,
                ])
                .gap(12.0)
                .align_center()
                .fill_w(),
            ),
            row(cards).gap(14.0).fill_w().into(),
        ])
        .gap(14.0)
        .fill_w()
        .into()
    }
}
