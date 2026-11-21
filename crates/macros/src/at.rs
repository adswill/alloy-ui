//! `at!(s.field)`: a two-way binding to a field of the view's state.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Expr};

const USAGE: &str = "at!(s.name), at!(s.form.email) or at!(s.minutes[i])";

fn bad(e: &impl Spanned, msg: impl std::fmt::Display) -> syn::Error {
    syn::Error::new(e.span(), msg)
}

/// Rebuild `expr` with its root variable replaced by `root`, rejecting anything that is not a
/// field or index chain that starts at a plain variable.
fn rebase(expr: &Expr, root: &syn::Ident) -> syn::Result<(Expr, syn::Ident)> {
    match expr {
        Expr::Path(p) if p.qself.is_none() && p.path.get_ident().is_some() => {
            let id = p.path.get_ident().unwrap().clone();
            let new: Expr = syn::parse_quote!(#root);
            Ok((new, id))
        }
        Expr::Field(f) => {
            let (base, id) = rebase(&f.base, root)?;
            let member = &f.member;
            Ok((syn::parse_quote!(#base.#member), id))
        }
        Expr::Index(i) => {
            let (base, id) = rebase(&i.expr, root)?;
            let idx = &i.index;
            Ok((syn::parse_quote!(#base[#idx]), id))
        }
        Expr::MethodCall(m) => Err(bad(
            m,
            format!(
                "`at!` can only point at a field or an index, not call `{}()`: the value has to \
                 live in the state so edits can be written back (try {USAGE})",
                m.method
            ),
        )),
        Expr::Reference(r) => Err(bad(
            r,
            format!("drop the `&`: `at!` already borrows the field (write {USAGE})"),
        )),
        Expr::Unary(u) => Err(bad(
            u,
            format!("`at!` points at a field, so drop the operator (write {USAGE})"),
        )),
        Expr::Lit(_) => Err(bad(
            expr,
            format!("`at!` needs a field of the state, not a literal (write {USAGE})"),
        )),
        Expr::Call(c) => Err(bad(
            c,
            format!("`at!` needs a field of the state, not a function call (write {USAGE})"),
        )),
        other => Err(bad(
            other,
            format!("`at!` expects a field of the view's state, like {USAGE}"),
        )),
    }
}

pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    if input.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("`at!` needs a field of the state: {USAGE}"),
        ));
    }
    let expr: Expr = syn::parse2(input)?;
    let root = syn::Ident::new("__alloy_state", expr.span());
    let (writable, var) = rebase(&expr, &root)?;
    if matches!(&expr, Expr::Path(_)) {
        return Err(bad(
            &expr,
            format!(
                "`at!({var})` names the whole state; point at one of its fields, like `at!({var}.name)`"
            ),
        ));
    }
    Ok(quote! {
        ::alloy_ui::place(&*#var, &#expr, move |#root| &mut #writable)
    })
}
