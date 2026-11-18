// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[modifier]` (§7.2). Written by the M4 W0 integrator (contract `ModifierTypeInfo`).
//!
//! ```ignore
//! use frap_modifiers::prelude::*;
//!
//! #[modifier(MIRROR, kind = Constructive, flags = ACCEPTS_MESH | SUPPORTS_EDITMODE)]
//! fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
//!     Ok(mesh)
//! }
//! ```
//!
//! Keeps the function and emits `static __FRAP_MODIFIER_<TYPE>: ModifierTypeInfo` registered in
//! `frap_modifiers::MODIFIER_TYPES` (`#[allow(unsafe_code)]`, RFC 0001 F-LINK-1), plus a
//! `#[test]` checking that the type's identifier is the one written.
//!
//! Arguments: the Blender type identifier first (`MIRROR`, `SIMPLE_DEFORM`; the variant of the
//! generated `ModifierType` is its camel case), then `key = value`:
//! - `kind = OnlyDeform | Constructive | Nonconstructive | DeformOrConstruct | NonGeometrical`
//!   (required);
//! - `flags = A | B | …` (`ModTypeFlags` constants; default empty);
//! - `depends_on = <path>` (default `no_deps`), `is_disabled = <path>` (default
//!   `never_disabled`);
//! - `krate = <path>`: where `frap_modifiers` is (default `::frap_modifiers`).

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemFn, Path, Token};

struct Args {
    ty: Ident,
    kind: Option<Ident>,
    flags: Vec<Ident>,
    depends_on: Option<Path>,
    is_disabled: Option<Path>,
    krate: Option<Path>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ty: Ident = input.parse()?;
        let mut a = Args {
            ty,
            kind: None,
            flags: Vec::new(),
            depends_on: None,
            is_disabled: None,
            krate: None,
        };
        let mut seen: Vec<String> = Vec::new();
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let k = key.to_string();
            if seen.contains(&k) {
                return Err(syn::Error::new(
                    key.span(),
                    format!("duplicate argument `{key}`"),
                ));
            }
            seen.push(k.clone());
            match k.as_str() {
                "kind" => a.kind = Some(input.parse()?),
                "flags" => {
                    a.flags.push(input.parse()?);
                    while input.peek(Token![|]) {
                        input.parse::<Token![|]>()?;
                        a.flags.push(input.parse()?);
                    }
                }
                "depends_on" => a.depends_on = Some(input.parse()?),
                "is_disabled" => a.is_disabled = Some(input.parse()?),
                "krate" => a.krate = Some(input.parse()?),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unexpected argument `{other}` (expected kind, flags, depends_on, \
                             is_disabled, krate)"
                        ),
                    ));
                }
            }
        }
        Ok(a)
    }
}

/// `SIMPLE_DEFORM` → `SimpleDeform` (codegen's `camel`).
fn camel(ident: &str) -> String {
    let mut out = String::new();
    for part in ident.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut cs = part.chars();
        if let Some(f) = cs.next() {
            out.push(f.to_ascii_uppercase());
            out.extend(cs.map(|c| c.to_ascii_lowercase()));
        }
    }
    out
}

const KINDS: [&str; 5] = [
    "OnlyDeform",
    "Constructive",
    "Nonconstructive",
    "DeformOrConstruct",
    "NonGeometrical",
];

fn expand2(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let args: Args = syn::parse2(attr)?;
    let f: ItemFn = syn::parse2(item)?;
    let krate = args
        .krate
        .clone()
        .unwrap_or_else(|| syn::parse_quote!(::frap_modifiers));
    let id = args.ty.to_string();
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(syn::Error::new(
            args.ty.span(),
            "the first argument is the Blender type identifier (`MIRROR`, `SIMPLE_DEFORM`)",
        ));
    }
    let variant = Ident::new(&camel(&id), args.ty.span());
    let kind = args
        .kind
        .ok_or_else(|| syn::Error::new(Span::call_site(), "missing `kind = …`"))?;
    if !KINDS.contains(&kind.to_string().as_str()) {
        return Err(syn::Error::new(
            kind.span(),
            format!(
                "unknown kind `{kind}` (expected one of {})",
                KINDS.join(", ")
            ),
        ));
    }
    let flags = args
        .flags
        .iter()
        .map(|fl| quote!(.union(#krate::ModTypeFlags::#fl)));
    let depends_on = match &args.depends_on {
        Some(p) => quote!(#p),
        None => quote!(#krate::no_deps),
    };
    let is_disabled = match &args.is_disabled {
        Some(p) => quote!(#p),
        None => quote!(#krate::never_disabled),
    };
    let func = &f.sig.ident;
    let stat = format_ident!("__FRAP_MODIFIER_{}", id);
    let test = format_ident!("__frap_modifier_{}_type", id.to_ascii_lowercase());
    Ok(quote! {
        #f

        #[doc(hidden)]
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[#krate::linkme::distributed_slice(#krate::MODIFIER_TYPES)]
        #[linkme(crate = #krate::linkme)]
        static #stat: #krate::ModifierTypeInfo = #krate::ModifierTypeInfo {
            ty: #krate::ModifierType::#variant,
            kind: #krate::ModKind::#kind,
            flags: #krate::ModTypeFlags::empty() #(#flags)*,
            depends_on: #depends_on,
            is_disabled: #is_disabled,
            eval: #func,
        };

        #[cfg(test)]
        #[test]
        fn #test() {
            assert_eq!(#stat.ty.identifier(), #id);
        }
    })
}

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand2(attr.into(), item.into()) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> TokenStream2 {
        quote!(
            fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
                Ok(mesh)
            }
        )
    }

    #[test]
    fn expands_type_kind_and_flags() {
        let out = expand2(
            quote!(
                SIMPLE_DEFORM,
                kind = OnlyDeform,
                flags = ACCEPTS_MESH | SUPPORTS_EDITMODE
            ),
            body(),
        )
        .unwrap()
        .to_string()
        .replace(' ', "");
        assert!(out.contains("ModifierType::SimpleDeform"), "{out}");
        assert!(out.contains("ModKind::OnlyDeform"));
        assert!(out.contains(".union(::frap_modifiers::ModTypeFlags::ACCEPTS_MESH)"));
        assert!(out.contains(".union(::frap_modifiers::ModTypeFlags::SUPPORTS_EDITMODE)"));
        assert!(out.contains("depends_on:::frap_modifiers::no_deps"));
        assert!(out.contains("__FRAP_MODIFIER_SIMPLE_DEFORM"));
    }

    #[test]
    fn rejects_bad_arguments() {
        let ok = |a: TokenStream2| expand2(a, body());
        assert!(ok(quote!(MIRROR, kind = Constructive)).is_ok());
        assert!(ok(quote!(MIRROR, kind = Constructive,)).is_ok());
        assert!(
            ok(quote!(
                MIRROR,
                kind = Constructive,
                depends_on = deps,
                is_disabled = off
            ))
            .is_ok()
        );
        assert!(ok(quote!(MIRROR)).is_err());
        assert!(ok(quote!(mirror, kind = Constructive)).is_err());
        assert!(ok(quote!(MIRROR, kind = Weird)).is_err());
        assert!(ok(quote!(MIRROR, kind = Constructive, kind = Constructive)).is_err());
        assert!(ok(quote!(MIRROR, kind = Constructive, frob = 1)).is_err());
    }

    #[test]
    fn camel_matches_codegen() {
        assert_eq!(camel("UV_PROJECT"), "UvProject");
        assert_eq!(camel("LAPLACIANSMOOTH"), "Laplaciansmooth");
        assert_eq!(camel("EDGE_SPLIT"), "EdgeSplit");
    }
}
