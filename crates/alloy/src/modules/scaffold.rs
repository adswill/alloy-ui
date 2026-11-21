//! Settings-page scaffold and login template.

use super::*;
use crate::{
    blocks::{field_after, local},
    dsl::{button, caption, heading, input, rule, text},
    rich::badge,
    view::{col, row, Flex, View},
    Act,
};
use std::sync::Arc;

/// One labelled row in a settings section: the label and its explanation on the left, the
/// control on the right: `setting("Notifications", "Banners for new activity", toggle(..))`.
pub fn setting<M: 'static>(
    label: impl Into<String>,
    description: impl Into<String>,
    control: impl Into<Node<M>>,
) -> Node<M> {
    let d: String = description.into();
    let mut left: Vec<Node<M>> = vec![text(label).label().fill_w().into()];
    if !d.is_empty() {
        left.push(caption(d).dim().fill_w().into());
    }
    row([col(left).gap(2.0).fill_w().into(), control.into()])
        .gap(24.0)
        .align_center()
        .fill_w()
        .pad_xy(16.0, 13.0)
        .into()
}

/// A settings page under construction. See [`settings_page`].
pub struct SettingsPage<M> {
    title: String,
    intro: Option<String>,
    sections: Vec<(String, Option<String>, Vec<Node<M>>)>,
    width: f32,
}

/// A page of grouped settings: a title, then sections that are each a card of [`setting`] rows
/// separated by hairlines:
/// `settings_page("Settings").section("Appearance", [setting(..), setting(..)])`.
pub fn settings_page<M>(title: impl Into<String>) -> SettingsPage<M> {
    SettingsPage {
        title: title.into(),
        intro: None,
        sections: Vec::new(),
        width: 720.0,
    }
}

impl<M> SettingsPage<M> {
    /// A sentence under the title.
    pub fn intro(mut self, s: impl Into<String>) -> Self {
        self.intro = Some(s.into());
        self
    }
    /// A section of rows.
    pub fn section<V: Into<Node<M>>>(
        mut self,
        title: impl Into<String>,
        rows: impl IntoIterator<Item = V>,
    ) -> Self {
        self.sections.push((
            title.into(),
            None,
            rows.into_iter().map(Into::into).collect(),
        ));
        self
    }
    /// A section with a line of explanation above its card.
    pub fn section_with<V: Into<Node<M>>>(
        mut self,
        title: impl Into<String>,
        about: impl Into<String>,
        rows: impl IntoIterator<Item = V>,
    ) -> Self {
        self.sections.push((
            title.into(),
            Some(about.into()),
            rows.into_iter().map(Into::into).collect(),
        ));
        self
    }
    /// Maximum content width in pixels (default 720).
    pub fn w(mut self, px: f32) -> Self {
        self.width = px;
        self
    }
}

impl<M: 'static> From<SettingsPage<M>> for Node<M> {
    fn from(p: SettingsPage<M>) -> Self {
        let mut items: Vec<Node<M>> = vec![heading(p.title).into()];
        if let Some(i) = p.intro {
            items.push(text(i).dim().fill_w().into());
        }
        for (title, about, rows) in p.sections {
            let mut head: Vec<Node<M>> = vec![text(title).label().into()];
            if let Some(a) = about {
                head.push(caption(a).dim().fill_w().into());
            }
            let n = rows.len();
            let mut body: Vec<Node<M>> = Vec::new();
            for (i, r) in rows.into_iter().enumerate() {
                body.push(r);
                if i + 1 != n {
                    body.push(rule());
                }
            }
            items.push(
                col([
                    Node::from(col(head).gap(2.0).fill_w()),
                    col(body).card().pad(0.0).fill_w().into(),
                ])
                .gap(10.0)
                .pad_xy(0.0, 6.0)
                .fill_w()
                .into(),
            );
        }
        col(items).gap(18.0).w(p.width).into()
    }
}

// ------------------------------------------------------------------ login

type OnCreds<S> = Arc<dyn Fn(&mut S, String, String) + Send + Sync>;
type OnName<S> = Arc<dyn Fn(&mut S, &str) + Send + Sync>;

/// A sign-in card under construction. See [`login`].
pub struct Login<S> {
    app: String,
    subtitle: Option<String>,
    signup: bool,
    providers: Vec<String>,
    on_submit: Option<OnCreds<S>>,
    on_provider: Option<OnName<S>>,
    on_forgot: Option<Arc<dyn Fn(&mut S) + Send + Sync>>,
    error: Option<String>,
}

/// A sign-in card: email, password, a primary button that enables once both are valid, optional
/// provider buttons and a forgot link. The fields live in the view, not your state:
/// `login("Alloy").providers(["GitHub"]).on_submit(|s, email, password| s.sign_in(email, password))`.
pub fn login<S: 'static>(app: impl Into<String>) -> Login<S> {
    Login {
        app: app.into(),
        subtitle: None,
        signup: false,
        providers: Vec::new(),
        on_submit: None,
        on_provider: None,
        on_forgot: None,
        error: None,
    }
}

impl<S: 'static> Login<S> {
    /// A sentence under the title.
    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }
    /// Sign-up variant: asks for a name and a longer password.
    pub fn create_account(mut self) -> Self {
        self.signup = true;
        self
    }
    /// "Continue with ..." buttons.
    pub fn providers(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.providers = names.into_iter().map(Into::into).collect();
        self
    }
    /// Called with the email and password when the form is valid and submitted.
    pub fn on_submit(mut self, f: impl Fn(&mut S, String, String) + Send + Sync + 'static) -> Self {
        self.on_submit = Some(Arc::new(f));
        self
    }
    /// Called with the provider name when its button is pressed.
    pub fn on_provider(mut self, f: impl Fn(&mut S, &str) + Send + Sync + 'static) -> Self {
        self.on_provider = Some(Arc::new(f));
        self
    }
    /// Adds a "Forgot password?" link.
    pub fn on_forgot(mut self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.on_forgot = Some(Arc::new(f));
        self
    }
    /// A failure from the server, shown above the button.
    pub fn error(mut self, s: impl Into<String>) -> Self {
        self.error = Some(s.into());
        self
    }
}

impl<S: 'static> From<Login<S>> for Node<Act<S>> {
    fn from(l: Login<S>) -> Self {
        let name = local(String::new());
        let email = local(String::new());
        let pass = local(String::new());
        let tried = local(false);
        let (e, pw, nm) = (email.get(), pass.get(), name.get());
        let min = if l.signup { 10 } else { 8 };
        let e_err =
            (!e.contains('@') || e.len() < 3).then_some("Enter an email like name@company.com");
        let p_err = (pw.chars().count() < min).then(|| format!("At least {min} characters"));
        let n_err = (l.signup && nm.trim().is_empty()).then_some("Tell us what to call you");
        let valid = e_err.is_none() && p_err.is_none() && n_err.is_none();
        let submit = l.on_submit.clone();
        let (ec, pc) = (e.clone(), pw.clone());
        let go = move |s: &mut S| {
            tried.set(true);
            if valid {
                if let Some(f) = &submit {
                    f(s, ec.clone(), pc.clone());
                }
            }
        };
        let t = tried.get();
        let mut items: Vec<Node<Act<S>>> = vec![heading(if l.signup {
            format!("Create your {} account", l.app)
        } else {
            format!("Sign in to {}", l.app)
        })
        .into()];
        if let Some(s) = l.subtitle {
            items.push(text(s).dim().fill_w().into());
        }
        items.push(crate::dsl::gap(0.0, 6.0));
        for p in &l.providers {
            let (f, n) = (l.on_provider.clone(), p.clone());
            items.push(
                button(format!("Continue with {p}"))
                    .fill_w()
                    .act(move |s| {
                        if let Some(f) = &f {
                            f(s, &n)
                        }
                    })
                    .into(),
            );
        }
        if !l.providers.is_empty() {
            items.push(
                row([rule(), caption("or").dim().into(), rule()])
                    .gap(10.0)
                    .align_center()
                    .fill_w()
                    .into(),
            );
        }
        if l.signup {
            items.push(
                field_after(
                    "Name",
                    input(nm).hint("Mira Okafor").local(&name).fill_w(),
                    n_err,
                    t,
                )
                .fill_w()
                .into(),
            );
        }
        items.push(
            field_after(
                "Email",
                input(e).hint("name@company.com").local(&email).fill_w(),
                e_err,
                t,
            )
            .fill_w()
            .into(),
        );
        let mut pw_in = input(pw)
            .secure()
            .local(&pass)
            .fill_w()
            .enter(Act::new(go.clone()));
        pw_in = pw_in.hint(if l.signup {
            "10+ characters"
        } else {
            "Your password"
        });
        items.push(
            field_after("Password", pw_in, p_err.as_deref(), t)
                .fill_w()
                .into(),
        );
        if let Some(f) = l.on_forgot {
            items.push(
                row([
                    hfill(),
                    button("Forgot password?").ghost().act(move |s| f(s)).into(),
                ])
                .fill_w()
                .into(),
            );
        }
        if let Some(err) = l.error {
            items.push(row([badge(err).danger().dot()]).into());
        }
        items.push(
            button(if l.signup {
                "Create account"
            } else {
                "Sign in"
            })
            .primary()
            .fill_w()
            .act(go)
            .into(),
        );
        Node::from(col(items).gap(10.0).card().pad(28.0).w(400.0).elevated())
    }
}

#[allow(unused)]
fn _keep(_: Flex<()>) {}
