//! `#[derive(Record)]` for Alloy: describe a struct once and get a sortable table
//! column, a validated edit field per struct field, and JSON persistence.
//!
//! The field's *type* picks the control (`String` text, `bool` checkbox, numbers a
//! slider, a `choices!` enum a dropdown). Conventions: the first field is the row's
//! title and is required; a `String` field named `email` is validated as an email.
//! Options go in `#[form(..)]`: `label = "Full name"`, `email`, `required`,
//! `range = (40.0, 200.0)`, `step = 5.0`, `long` (a multi-line editor), `choices = ROLES` (a `[&str; N]`; the field is
//! a `String`), `skip`.
//!
//! The derive also implements `Serialize` and `Deserialize` (missing keys keep their
//! `Default`), so do not derive them as well.

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
    let (mut ser, mut de) = (vec![], vec![]);
    let mut stamp: Option<syn::Ident> = None;
    for f in &fields.named {
        let id = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        let (mut label, mut email, mut required, mut skip, mut long) =
            (None, false, false, false, false);
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
                } else if m.path.is_ident("long") {
                    long = true;
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
        let key = id.to_string();
        ser.push(quote!(s.serialize_field(#key, &self.#id)?;));
        de.push(quote!(
            if let Some(v) = map.remove(#key) {
                out.#id = ::alloy_ui::serde_json::from_value(v).map_err(D::Error::custom)?;
            }
        ));
        if let syn::Type::Path(p) = ty {
            if p.path.segments.last().is_some_and(|s| s.ident == "Stamp") {
                stamp = Some(id.clone());
            }
        }
        if skip {
            continue;
        }
        let i = infos.len();
        let label = label.unwrap_or_else(|| {
            let s = key.replace('_', " ");
            let mut c = s.chars();
            c.next()
                .map(|h| h.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        });
        let kind = if long {
            quote!(::alloy_ui::Kind::Long)
        } else if let Some(c) = &choices {
            quote!(::alloy_ui::Kind::Choice(&#c))
        } else if range.is_some() || step.is_some() {
            let (lo, hi) = range.map_or((quote!(0.0), quote!(100.0)), |(a, b)| {
                (quote!(#a), quote!(#b))
            });
            let st = step.as_ref().map_or(quote!(1.0), |s| quote!(#s));
            quote!(::alloy_ui::Kind::Number { min: #lo, max: #hi, step: #st })
        } else if email || key == "email" {
            quote!(::alloy_ui::Kind::Email)
        } else {
            quote!(<#ty as ::alloy_ui::Field>::kind())
        };
        let required = required || i == 0;
        infos.push(
            quote!(::alloy_ui::FieldInfo { label: #label, kind: #kind, required: #required }),
        );
        gets.push(quote!(#i => ::alloy_ui::Field::get(&self.#id)));
        sets.push(quote!(#i => ::alloy_ui::Field::set(&mut self.#id, v)));
    }
    let count = fields.named.len();
    let title = name.to_string();
    let stamp_fns = stamp.map(|id| {
        quote! {
            fn stamp(&self) -> Option<u64> { Some(self.#id.0) }
            fn touch(&mut self) { self.#id = ::alloy_ui::Stamp::now(); }
        }
    });
    Ok(quote! {
        impl ::alloy_ui::Record for #name {
            fn fields() -> ::std::vec::Vec<::alloy_ui::FieldInfo> {
                vec![#(#infos),*]
            }
            fn get(&self, field: usize) -> ::alloy_ui::Value {
                match field { #(#gets,)* _ => unreachable!() }
            }
            fn set(&mut self, field: usize, v: ::alloy_ui::Value) {
                match field { #(#sets,)* _ => unreachable!() }
            }
            #stamp_fns
        }
        impl ::alloy_ui::serde::Serialize for #name {
            fn serialize<S: ::alloy_ui::serde::Serializer>(&self, s: S) -> ::std::result::Result<S::Ok, S::Error> {
                use ::alloy_ui::serde::ser::SerializeStruct;
                let mut s = s.serialize_struct(#title, #count)?;
                #(#ser)*
                s.end()
            }
        }
        impl<'de> ::alloy_ui::serde::Deserialize<'de> for #name {
            fn deserialize<D: ::alloy_ui::serde::Deserializer<'de>>(d: D) -> ::std::result::Result<Self, D::Error> {
                use ::alloy_ui::serde::de::Error;
                let mut map = <::std::collections::HashMap<String, ::alloy_ui::serde_json::Value> as ::alloy_ui::serde::Deserialize>::deserialize(d)?;
                let mut out = <Self as ::std::default::Default>::default();
                #(#de)*
                Ok(out)
            }
        }
    })
}
