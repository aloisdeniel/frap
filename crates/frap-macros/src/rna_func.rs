// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[rna_func]` (§9.5). Owned by WP 1.2.
//!
//! ```ignore
//! #[rna_func(srna = "Object", func = "select_set")]
//! fn select_set(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> { … }
//! ```
//!
//! Keeps the function and registers `RnaFuncReg { srna, func, imp }` in `RNA_FUNCS`. The
//! registry types are looked up in `::frap_scene` (L6, where `RNA_FUNCS` lives); code inside
//! `frap-scene` itself (or a test) passes `krate = crate` (any path that names `RnaFuncReg` and
//! `RNA_FUNCS`). The positional form `#[rna_func("Object", "select_set")]` is accepted too.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemFn, LitStr, Path, Token};

/// `srna = "…", func|prop = "…", [get = path,] [set = path,] [krate = path]` (shared with
/// `#[rna_hook]`).
pub(crate) struct RegArgs {
    pub srna: LitStr,
    pub name: LitStr,
    pub krate: Path,
    pub get: Option<Path>,
    pub set: Option<Path>,
}

impl RegArgs {
    pub(crate) fn parse_with(input: ParseStream, name_key: &str) -> syn::Result<Self> {
        let mut srna = None;
        let mut name = None;
        let mut krate = None;
        let mut get = None;
        let mut set = None;
        let mut positional = 0;
        while !input.is_empty() {
            if input.peek(LitStr) {
                let l: LitStr = input.parse()?;
                match positional {
                    0 => srna = Some(l),
                    1 => name = Some(l),
                    _ => return Err(syn::Error::new(l.span(), "too many positional arguments")),
                }
                positional += 1;
            } else {
                let key: Ident = input.parse()?;
                input.parse::<Token![=]>()?;
                let k = key.to_string();
                if k == "srna" {
                    srna = Some(input.parse()?);
                } else if k == name_key {
                    name = Some(input.parse()?);
                } else if k == "krate" {
                    krate = Some(input.parse()?);
                } else if k == "get" && name_key == "prop" {
                    get = Some(input.parse()?);
                } else if k == "set" && name_key == "prop" {
                    set = Some(input.parse()?);
                } else {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unexpected argument `{k}`"),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        let span = proc_macro2::Span::call_site();
        Ok(RegArgs {
            srna: srna.ok_or_else(|| syn::Error::new(span, "missing `srna = \"Struct\"`"))?,
            name: name
                .ok_or_else(|| syn::Error::new(span, format!("missing `{name_key} = \"…\"`")))?,
            krate: krate.unwrap_or_else(|| syn::parse_quote!(::frap_scene)),
            get,
            set,
        })
    }
}

struct FuncArgsSpec(RegArgs);

impl Parse for FuncArgsSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        RegArgs::parse_with(input, "func").map(FuncArgsSpec)
    }
}

/// `#[allow(unsafe_code)]` registration of `value` (of type `ty`) into the slice `slice`.
pub(crate) fn register(slice: TokenStream2, ty: TokenStream2, value: TokenStream2) -> TokenStream2 {
    quote! {
        #[allow(unsafe_code)]
        const _: () = {
            #[allow(unsafe_code)]
            #[::frap_rna::linkme::distributed_slice(#slice)]
            #[linkme(crate = ::frap_rna::linkme)]
            static __FRAP_REGISTRATION: #ty = #value;
        };
    }
}

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = match syn::parse::<FuncArgsSpec>(attr) {
        Ok(a) => a.0,
        Err(e) => return e.to_compile_error().into(),
    };
    let f = match syn::parse::<ItemFn>(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };
    let ident = &f.sig.ident;
    let RegArgs {
        srna, name, krate, ..
    } = args;
    let reg = register(
        quote!(#krate::RNA_FUNCS),
        quote!(#krate::RnaFuncReg),
        quote!(#krate::RnaFuncReg { srna: #srna, func: #name, imp: #ident }),
    );
    quote!(#f #reg).into()
}
