//! `#[derive(Record)]` for Alloy: describe a struct once and get a sortable table
//! column and a validated edit field per struct field.
//!
//! Field types pick the control: `String` is a text field, `bool` a checkbox,
//! numbers a slider. Options go in `#[form(..)]`:
//! `label = "Full name"`, `email`, `required`, `range = (40.0, 200.0)`, `step = 5.0`,
//! `choices = ROLES` (a `[&str; N]` const; the field is a `String`), `skip`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr, Fields, Lit};

/// Derive `alloy_ui::Record` for a struct with named fields.
#[proc_macro_derive(Record, attributes(form))]
pub fn record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "Record needs a struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(input, "Record needs named fields"));
    };
    let name = &input.ident;
    let (mut infos, mut gets, mut sets) = (vec![], vec![], vec![]);
    for f in &fields.named {
        let id = f.ident.as_ref().unwrap();
        let ty = match &f.ty {
            syn::Type::Path(p) => p.path.segments.last().unwrap().ident.to_string(),
            _ => String::new(),
        };
        let (mut label, mut email, mut required, mut skip) = (None, false, false, false);
        let (mut range, mut step, mut choices): (Option<(Expr, Expr)>, Option<Expr>, Option<Expr>) =
            (None, None, None);
        for a in f.attrs.iter().filter(|a| a.path().is_ident("form")) {
            a.parse_nested_meta(|m| {
                if m.path.is_ident("label") {
                    let Lit::Str(s) = m.value()?.parse::<Lit>()? else {
                        return Err(m.error("label must be a string"));
                    };
                    label = Some(s.value());
                } else if m.path.is_ident("email") {
                    email = true;
                } else if m.path.is_ident("required") {
                    required = true;
                } else if m.path.is_ident("skip") {
                    skip = true;
                } else if m.path.is_ident("range") {
                    let Expr::Tuple(t) = m.value()?.parse::<Expr>()? else {
                        return Err(m.error("range = (min, max)"));
                    };
                    let mut it = t.elems.into_iter();
                    range = Some((it.next().unwrap(), it.next().unwrap()));
                } else if m.path.is_ident("step") {
                    step = Some(m.value()?.parse()?);
                } else if m.path.is_ident("choices") {
                    choices = Some(m.value()?.parse()?);
                } else {
                    return Err(m.error("unknown field option"));
                }
                Ok(())
            })?;
        }
        if skip {
            continue;
        }
        let i = infos.len();
        let label = label.unwrap_or_else(|| {
            let s = id.to_string().replace('_', " ");
            let mut c = s.chars();
            c.next()
                .map(|h| h.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        });
        let kind = match ty.as_str() {
            "String" => match &choices {
                Some(c) => quote!(::alloy_ui::Kind::Choice(&#c)),
                None if email => quote!(::alloy_ui::Kind::Email),
                None => quote!(::alloy_ui::Kind::Text),
            },
            "bool" => quote!(::alloy_ui::Kind::Bool),
            _ => {
                let (lo, hi) = range
                    .clone()
                    .map_or((quote!(0.0), quote!(100.0)), |(a, b)| {
                        (quote!(#a), quote!(#b))
                    });
                let st = step.as_ref().map_or(quote!(1.0), |s| quote!(#s));
                quote!(::alloy_ui::Kind::Number { min: #lo, max: #hi, step: #st })
            }
        };
        infos.push(
            quote!(::alloy_ui::FieldInfo { label: #label, kind: #kind, required: #required }),
        );
        let (get, set) = match ty.as_str() {
            "String" => (
                quote!(::alloy_ui::Value::Text(self.#id.clone())),
                quote!(if let ::alloy_ui::Value::Text(t) = v { self.#id = t }),
            ),
            "bool" => (
                quote!(::alloy_ui::Value::Bool(self.#id)),
                quote!(if let ::alloy_ui::Value::Bool(b) = v { self.#id = b }),
            ),
            _ => (
                quote!(::alloy_ui::Value::Num(self.#id as f32)),
                quote!(if let ::alloy_ui::Value::Num(n) = v { self.#id = n as _ }),
            ),
        };
        gets.push(quote!(#i => #get));
        sets.push(quote!(#i => #set));
    }
    Ok(quote! {
        impl ::alloy_ui::Record for #name {
            const FIELDS: &'static [::alloy_ui::FieldInfo] = &[#(#infos),*];
            fn get(&self, field: usize) -> ::alloy_ui::Value {
                match field { #(#gets,)* _ => unreachable!() }
            }
            fn set(&mut self, field: usize, v: ::alloy_ui::Value) {
                match field { #(#sets,)* _ => unreachable!() }
            }
        }
    })
}
