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

mod at;
mod diag;
mod state;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Expr, Fields, Lit};

const FORM_OPTIONS: [&str; 8] = [
    "label", "email", "required", "long", "skip", "range", "step", "choices",
];

/// Declare the state struct and its starting values together. A field with `= value` starts
/// there; any other field starts at `Default::default()`. The struct gets `Default`, so
/// `sketch(Timer::default(), view)` needs no initialiser block. A string literal start value on
/// a `String` field needs no `.to_string()`.
///
/// ```ignore
/// alloy_ui::state! {
///     struct Timer {
///         minutes: f32 = 25.0,
///         running: bool,
///         title: String = "Focus",
///     }
/// }
/// ```
///
/// Mistakes are reported at the field: a missing type, a doubled field name, a generic or
/// tuple struct, or a `Default` derive that would conflict.
#[proc_macro]
pub fn state(input: TokenStream) -> TokenStream {
    match syn::parse::<state::State>(input) {
        Ok(s) => s.expand().into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// A field of the state, for two-way binding: `input(at!(s.name))`, `check("On", at!(s.on))`,
/// `button("Pause").toggle(at!(s.paused))`. `s` is the `&State` your view receives;
/// nested fields (`at!(s.form.email)`) and indexing (`at!(s.minutes[i])`) work too.
///
/// Anything else (a method call, a literal, the bare state) is a compile error that says what
/// `at!` expects, because edits must be written back to a place in the state.
#[proc_macro]
pub fn at(input: TokenStream) -> TokenStream {
    match at::expand(input.into()) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Derive `alloy_ui::Record` for a struct with named fields.
#[proc_macro_derive(Record, attributes(form))]
pub fn record(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Flags (`email`, `required`, ..) take no value; say so instead of a bare parse error.
fn flag_only(m: &syn::meta::ParseNestedMeta, name: &str) -> syn::Result<()> {
    if m.input.peek(syn::Token![=]) {
        return Err(m.error(format!(
            "`{name}` is a flag, not a setting: write `#[form({name})]`"
        )));
    }
    Ok(())
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            format!(
                "`#[derive(Record)]` needs a struct with named fields; `{}` is not a struct. \
                 Wrap the data in one: `struct {0}Row {{ name: String }}`",
                input.ident
            ),
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(
            input.ident.span(),
            format!(
                "`#[derive(Record)]` needs named fields to label the form and table: \
                 write `struct {} {{ name: String, .. }}` instead of a tuple or unit struct",
                input.ident
            ),
        ));
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
                        return Err(m.error("`label` takes a string: `#[form(label = \"Full name\")]`"));
                    };
                    label = Some(s.value());
                } else if m.path.is_ident("email") {
                    flag_only(&m, "email")?;
                    email = true;
                } else if m.path.is_ident("required") {
                    flag_only(&m, "required")?;
                    required = true;
                } else if m.path.is_ident("long") {
                    flag_only(&m, "long")?;
                    long = true;
                } else if m.path.is_ident("skip") {
                    flag_only(&m, "skip")?;
                    skip = true;
                } else if m.path.is_ident("range") {
                    let e = m.value()?.parse::<Expr>()?;
                    let Expr::Tuple(t) = &e else {
                        return Err(syn::Error::new(
                            e.span(),
                            "`range` takes a (min, max) pair: `#[form(range = (0.0, 100.0))]`",
                        ));
                    };
                    if t.elems.len() != 2 {
                        return Err(syn::Error::new(
                            e.span(),
                            format!(
                                "`range` takes exactly two numbers, (min, max); found {}",
                                t.elems.len()
                            ),
                        ));
                    }
                    let mut it = t.elems.iter().cloned();
                    range = Some((it.next().unwrap(), it.next().unwrap()));
                } else if m.path.is_ident("step") {
                    step = Some(m.value().map_err(|_| m.error("`step` needs a value: `#[form(step = 5.0)]`"))?.parse()?);
                } else if m.path.is_ident("choices") {
                    choices = Some(m.value().map_err(|_| m.error("`choices` needs a list: `#[form(choices = ROLES)]` where `const ROLES: [&str; 3]`"))?.parse()?);
                } else {
                    let name = m.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
                    let hint = match diag::closest(&name, &FORM_OPTIONS) {
                        Some(c) => format!(" Did you mean `{c}`?"),
                        None => String::new(),
                    };
                    return Err(m.error(format!(
                        "unknown form option `{name}`.{hint} Options are {}",
                        diag::list(&FORM_OPTIONS)
                    )));
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
