use std::collections::HashSet;

use itertools::Itertools;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::ToTokens;
use syn::{
    parse::Error, punctuated::Punctuated, spanned::Spanned, token::Paren,
    AngleBracketedGenericArguments, Data, DeriveInput, Type, TypeTuple,
};

static KEYS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

#[proc_macro_derive(HydratableQuery, attributes(result, param))]
pub fn hydratable_query_derive(input: TokenStream) -> TokenStream {
    let DeriveInput {
        attrs, ident, data, ..
    } = syn::parse_macro_input!(input as DeriveInput);
    let ident_name = ident.to_string();

    if !KEYS.lock().insert(ident_name.to_string()) {
        return Error::new_spanned(ident, "duplicate hydratable key")
            .into_compile_error()
            .into();
    }

    let wrong_type = match data {
        Data::Struct(_) => None,
        Data::Enum(e) => Some((e.enum_token.span, "enum")),
        Data::Union(u) => Some((u.union_token.span, "union")),
    };

    if let Some((span, wrong_type)) = wrong_type {
        return Error::new(
            span,
            format!(
                "expected struct, HydratableQuery cannot be derived for {}",
                wrong_type
            ),
        )
        .into_compile_error()
        .into();
    }

    let mut result: Option<AngleBracketedGenericArguments> = None;
    let mut param = None;

    for (k, mut attr) in &attrs.into_iter().group_by(|e| e.path.clone()) {
        if k.is_ident("result") {
            let result_attr = attr.next().unwrap();
            if let Some(dupe) = attr.next() {
                return Error::new_spanned(dupe, "duplicate attribute")
                    .into_compile_error()
                    .into();
            }
            result = Some(match result_attr.parse_args() {
                Ok(r) => r,
                Err(e) => {
                    let mut e1 = Error::new(
                        e.span(),
                        "result attribute should be formatted like #[result(<R, E>)]",
                    );
                    e1.combine(e);
                    return e1.into_compile_error().into();
                }
            });
        } else if k.is_ident("param") {
            let param_attr = attr.next().unwrap();
            if let Some(dupe) = attr.next() {
                return Error::new_spanned(dupe, "duplicate attribute")
                    .into_compile_error()
                    .into();
            }
            param = Some(match param_attr.parse_args() {
                Ok(p) => p,
                Err(e) => {
                    let mut e1 = Error::new(
                        e.span(),
                        "param attribute should be fomatted like #[param(P)]",
                    );
                    e1.combine(e);
                    return e1.into_compile_error().into();
                }
            });
        }
    }

    let flag = result.is_some();
    let result = match result {
        Some(r) if r.args.len() == 2 => {
            let mut iter = r.args.into_iter();
            Ok((iter.next().unwrap(), iter.next().unwrap()))
        }
        Some(r) if r.args.is_empty() => Err(r.lt_token.span()),
        Some(r) if r.args.len() == 1 => Err(r.gt_token.span()),
        Some(r) => Err(r
            .args
            .iter()
            .nth(2)
            .unwrap()
            .to_token_stream()
            .into_iter()
            .next()
            .unwrap()
            .span()),
        None => Err(Span::call_site()),
    };
    let (result, err) = match result {
        Ok((r, e)) => (r, e),
        Err(e) => {
            return Error::new(
                e,
                match flag {
                    true => "expected 2 generic arguments: #[result(<R, E>)]",
                    false => "expected #[result(<R, E>)] attribute",
                },
            )
            .into_compile_error()
            .into()
        }
    };
    let param = match param {
        Some(p) => p,
        None => Type::Tuple(TypeTuple {
            paren_token: Paren(Span::call_site()),
            elems: Punctuated::new(),
        }),
    };

    let crate_ = match crate_name("sycamore-query")
        .expect("sycamore-query should be present in Cargo.toml")
    {
        FoundCrate::Itself => "crate".to_string(),
        FoundCrate::Name(name) => name,
    };
    let crate_ = Ident::new(&crate_, Span::call_site());

    quote::quote! {
        impl HydratableQuery for #ident {
            type Param = #param;
            type Result = #result;
            type Error = #err;

            fn builder() -> #crate_::hydrate::HydratableQueryBuilder<Self::Param, Self::Result, Self::Error> {
                unsafe { #crate_::hydrate::HydratableQueryBuilder::new(#ident_name.to_string()) }
            }
        }
    }
    .into()
}
