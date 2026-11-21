//! `state! { struct S { field: Type = default } }`: a struct and its starting values together.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    Attribute, Expr, Ident, Lit, Token, Type, Visibility,
};

const USAGE: &str = "state! { struct Name { field: Type = start_value, other: Type } }";

struct Field {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    ty: Type,
    default: Option<Expr>,
}

pub struct State {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    fields: Vec<Field>,
}

fn is_string(t: &Type) -> bool {
    matches!(t, Type::Path(p) if p.qself.is_none() && p.path.is_ident("String"))
}

impl Parse for State {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        for a in &attrs {
            if a.path().is_ident("derive") {
                a.parse_nested_meta(|m| {
                    if m.path.is_ident("Default") {
                        Err(m.error(
                            "`state!` already implements `Default` from the starting values; \
                             remove `Default` from this derive",
                        ))
                    } else {
                        Ok(())
                    }
                })?;
            }
        }
        let vis: Visibility = input.parse()?;
        if input.peek(Token![enum]) || input.peek(Token![union]) {
            return Err(input.error(format!(
                "`state!` declares a struct, not an enum or union; write `{USAGE}`"
            )));
        }
        if !input.peek(Token![struct]) {
            return Err(input.error(format!("expected `struct`; write `{USAGE}`")));
        }
        input.parse::<Token![struct]>()?;
        let name: Ident = input.parse()?;
        if input.peek(Token![<]) {
            return Err(input.error(
                "`state!` structs cannot be generic; declare the struct yourself and \
                 `impl Default` by hand",
            ));
        }
        if input.peek(syn::token::Paren) || input.peek(Token![;]) {
            return Err(input.error(
                "`state!` needs named fields: `struct Name { field: Type = start_value }`",
            ));
        }
        if !input.peek(syn::token::Brace) {
            return Err(input.error("expected `{` and the fields after the struct name"));
        }
        let body;
        braced!(body in input);
        let mut fields: Vec<Field> = vec![];
        while !body.is_empty() {
            let attrs = body.call(Attribute::parse_outer)?;
            let vis: Visibility = body.parse()?;
            let fname: Ident = body.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    "expected a field name: `field: Type = start_value`",
                )
            })?;
            if let Some(prev) = fields.iter().find(|f| f.name == fname) {
                let mut e =
                    syn::Error::new(fname.span(), format!("field `{fname}` is declared twice"));
                e.combine(syn::Error::new(prev.name.span(), "first declared here"));
                return Err(e);
            }
            if !body.peek(Token![:]) {
                let hint = if body.peek(Token![=]) {
                    format!(
                        "`{fname}` needs a type before its start value: `{fname}: Type = value`"
                    )
                } else {
                    format!(
                        "`{fname}` needs a type: `{fname}: Type`, or `{fname}: Type = start_value`"
                    )
                };
                return Err(syn::Error::new(fname.span(), hint));
            }
            body.parse::<Token![:]>()?;
            let ty: Type = body.parse().map_err(|e| {
                syn::Error::new(
                    e.span(),
                    format!("expected the type of `{fname}`: `{fname}: Type`"),
                )
            })?;
            let default = if body.peek(Token![=]) {
                body.parse::<Token![=]>()?;
                if body.is_empty() || body.peek(Token![,]) {
                    return Err(syn::Error::new(
                        fname.span(),
                        format!("`{fname}` has `=` but no start value; write `{fname}: Type = value` or drop the `=`"),
                    ));
                }
                Some(body.parse::<Expr>()?)
            } else {
                None
            };
            if !body.is_empty() {
                if body.peek(Token![,]) {
                    body.parse::<Token![,]>()?;
                } else {
                    return Err(body.error(format!(
                        "expected `,` after the field `{fname}`; separate fields with commas"
                    )));
                }
            }
            fields.push(Field {
                attrs,
                vis,
                name: fname,
                ty,
                default,
            });
        }
        if !input.is_empty() {
            return Err(
                input.error("unexpected tokens after the struct; `state!` takes one struct")
            );
        }
        Ok(State {
            attrs,
            vis,
            name,
            fields,
        })
    }
}

impl State {
    pub fn expand(self) -> TokenStream {
        let State {
            attrs,
            vis,
            name,
            fields,
        } = self;
        let decl = fields.iter().map(|f| {
            let (a, v, n, t) = (&f.attrs, &f.vis, &f.name, &f.ty);
            quote!(#(#a)* #v #n: #t)
        });
        let init = fields.iter().map(|f| {
            let n = &f.name;
            let value = match &f.default {
                Some(Expr::Lit(l)) if matches!(l.lit, Lit::Str(_)) && is_string(&f.ty) => {
                    quote!(::std::string::ToString::to_string(#l))
                }
                Some(d) => quote!(#d),
                None => quote!(::std::default::Default::default()),
            };
            quote!(#n: #value)
        });
        quote! {
            #(#attrs)*
            #vis struct #name { #(#decl),* }
            impl ::std::default::Default for #name {
                fn default() -> Self { Self { #(#init),* } }
            }
        }
    }
}
