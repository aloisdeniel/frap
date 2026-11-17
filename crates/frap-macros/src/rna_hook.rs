// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[rna_hook]` (§9.5). Owned by WP 1.2.
//!
//! One registration per `(struct, property)` with a get and/or a set hook:
//!
//! ```ignore
//! // On one function: a set hook (4 parameters) or a get hook (3 parameters).
//! #[rna_hook(srna = "Object", prop = "rotation_mode")]
//! fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> { … }
//!
//! // Both, naming the other function:
//! #[rna_hook(srna = "Object", prop = "matrix_world", get = matrix_world_get)]
//! fn matrix_world_set(…) -> Result<(), RnaError> { … }
//!
//! // Or on an inline module holding `fn get` and/or `fn set`:
//! #[rna_hook(srna = "Object", prop = "parent")]
//! mod parent { pub fn get(…) -> … { … } pub fn set(…) -> … { … } }
//! ```
//!
//! Registers `RnaHookReg { srna, prop, get, set }` in `RNA_HOOKS` (`::frap_scene` unless
//! `krate = …` names another path holding `RnaHookReg`, `RnaGetHook`, `RnaSetHook`,
//! `RNA_HOOKS`).

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Item, Path};

use crate::rna_func::{RegArgs, register};

struct HookArgs(RegArgs);

impl Parse for HookArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        RegArgs::parse_with(input, "prop").map(HookArgs)
    }
}

fn opt(p: Option<Path>, ty: TokenStream2) -> TokenStream2 {
    match p {
        Some(p) => quote!(::core::option::Option::Some(#p as #ty)),
        None => quote!(::core::option::Option::None),
    }
}

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand2(attr, item) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand2(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream2> {
    let HookArgs(args) = syn::parse::<HookArgs>(attr)?;
    let item: Item = syn::parse(item)?;
    let mut get = args.get;
    let mut set = args.set;
    let dup = |what: &str, span| syn::Error::new(span, format!("the {what} hook is given twice"));
    match &item {
        Item::Fn(f) => {
            let ident = &f.sig.ident;
            let path: Path = syn::parse_quote!(#ident);
            match f.sig.inputs.len() {
                3 => {
                    if get.replace(path).is_some() {
                        return Err(dup("get", ident.span()));
                    }
                }
                4 => {
                    if set.replace(path).is_some() {
                        return Err(dup("set", ident.span()));
                    }
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        &f.sig,
                        "a get hook takes (host, this, prop), a set hook (host, this, prop, value)",
                    ));
                }
            }
        }
        Item::Mod(m) => {
            let ident = &m.ident;
            let Some((_, items)) = &m.content else {
                return Err(syn::Error::new_spanned(
                    m,
                    "#[rna_hook] needs an inline module with `fn get` and/or `fn set`",
                ));
            };
            for it in items {
                if let Item::Fn(f) = it {
                    let name = f.sig.ident.to_string();
                    let fi = &f.sig.ident;
                    let path: Path = syn::parse_quote!(#ident::#fi);
                    let slot = match name.as_str() {
                        "get" => &mut get,
                        "set" => &mut set,
                        _ => continue,
                    };
                    if slot.replace(path).is_some() {
                        return Err(dup(&name, fi.span()));
                    }
                }
            }
        }
        other => {
            return Err(syn::Error::new_spanned(
                other,
                "#[rna_hook] applies to a function or an inline module",
            ));
        }
    }
    if get.is_none() && set.is_none() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[rna_hook] found neither a get nor a set hook",
        ));
    }
    let krate = &args.krate;
    let srna = &args.srna;
    let prop = &args.name;
    let get = opt(get, quote!(#krate::RnaGetHook));
    let set = opt(set, quote!(#krate::RnaSetHook));
    let reg = register(
        quote!(#krate::RNA_HOOKS),
        quote!(#krate::RnaHookReg),
        quote!(#krate::RnaHookReg { srna: #srna, prop: #prop, get: #get, set: #set }),
    );
    Ok(quote!(#item #reg))
}
