//! Multi-step wizards.

use super::*;

/// A multi-step flow: a numbered header, the current page, and Back / Next buttons where Next
/// stays disabled until the page says it is valid. Made by [`wizard`].
pub struct Wizard<S> {
    at: usize,
    lens: crate::bind::Lens<S, usize>,
    pages: Vec<(String, bool, Node<Act<S>>)>,
    finish: String,
    on_finish: Option<Act<S>>,
    done: Option<Node<Act<S>>>,
}

/// A step-by-step flow whose position lives in a `usize` field:
///
/// ```ignore
/// wizard(at!(s.step))
///     .page("Account", s.account_ok(), col![field("Name", input(at!(s.name)), None)])
///     .page("Review", s.terms, col!["Check everything", check("I agree", at!(s.terms))])
///     .finish("Create account")
///     .on_finish(|s| s.submit())
///     .done(col![heading("All set")])
/// ```
pub fn wizard<S: 'static>(step: Place<'_, S, usize>) -> Wizard<S> {
    Wizard {
        at: *step.cur,
        lens: step.lens,
        pages: Vec::new(),
        finish: "Finish".into(),
        on_finish: None,
        done: None,
    }
}

impl<S: 'static> Wizard<S> {
    /// Add a page. `valid` gates the Next button.
    pub fn page(mut self, title: &str, valid: bool, body: impl Into<Node<Act<S>>>) -> Self {
        self.pages.push((title.into(), valid, body.into()));
        self
    }
    /// Label of the last page's button (default "Finish").
    pub fn finish(mut self, label: &str) -> Self {
        self.finish = label.into();
        self
    }
    /// Run `f` when the last page's button is pressed, before moving on.
    pub fn on_finish(mut self, f: impl Fn(&mut S) + Send + Sync + 'static) -> Self {
        self.on_finish = Some(Act::new(f));
        self
    }
    /// Shown after the last page's button was pressed.
    pub fn done(mut self, body: impl Into<Node<Act<S>>>) -> Self {
        self.done = Some(body.into());
        self
    }
}

impl<S: 'static> From<Wizard<S>> for Node<Act<S>> {
    fn from(w: Wizard<S>) -> Self {
        let n = w.pages.len();
        let at = w.at.min(n);
        let lens = w.lens;
        if at == n {
            return w.done.unwrap_or_else(|| Node::from(caption("Done")));
        }
        let header = crate::view::row(w.pages.iter().enumerate().map(|(i, (t, ..))| {
            text(format!("{}. {t}", i + 1))
                .accent_if(i == at)
                .dim_if(i > at)
        }))
        .gap(24.0);
        let (_, valid, body) = w.pages.into_iter().nth(at).unwrap();
        let back = lens.clone();
        let next = lens;
        let last = at + 1 == n;
        let on_finish = w.on_finish;
        let go = button(if last { w.finish } else { "Next".into() })
            .primary()
            .enabled(valid)
            .act(move |s| {
                if last {
                    if let Some(f) = &on_finish {
                        (f.0)(s);
                    }
                }
                *next(s) += 1;
            });
        let prev = button("Back")
            .enabled(at > 0)
            .act(move |s| *back(s) = back(s).saturating_sub(1));
        crate::col![header, body, crate::row![prev, space(), go].fill_w()]
            .gap(24.0)
            .into()
    }
}
