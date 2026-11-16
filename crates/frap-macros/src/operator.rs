// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[operator]` (§10.4). Owned by WP 1.12.
//!
//! ```ignore
//! use frap_op::prelude::*;
//! use frap_spec_ops_mesh::mesh;
//!
//! #[operator(poll = poll::always, tier = T1)]
//! fn exec(cx: &mut OpCtx, p: mesh::PrimitiveCubeAdd) -> OpResult {
//!     Ok(OpStatus::Finished)
//! }
//! ```
//!
//! Keeps the function and emits:
//! - a wrapper `fn(&mut OpCtx, &OpArgs) -> OpResult` calling it with `P::from_args(args)`
//!   (`P: OpProps`, the generated props type of the second parameter; anything else fails to
//!   compile);
//! - `static __FRAP_OPERATOR_<FN>: OperatorDef` registered in `frap_op::OPERATORS`
//!   (`#[allow(unsafe_code)]`, RFC 0001 F-LINK-1);
//! - a `#[test]` checking that the registered spec is `P::spec()` (same static, same idname) and
//!   that `kind = Macro` matches the spec's macro steps.
//!
//! Arguments (`key = value`, comma-separated):
//! - `poll = <path>` (required): a `frap_op::Poll` constant. A lower-case last segment names the
//!   upper-case constant (`poll::always` → `poll::ALWAYS`, `my_poll` → `MY_POLL`).
//! - `tier = T0..T4` (required).
//! - `spec = <path>`: the `OperatorSpec` static. Inferred when the props type is written with its
//!   namespace module (`mesh::PrimitiveCubeAdd` → `mesh::OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD`,
//!   codegen's naming); required for a bare or aliased type (`P`), because a static
//!   initializer cannot call `P::spec()`.
//! - `kind = Normal | Macro` (default `Normal`).
//! - `support = Full | Partial("…") | HeadlessExtended("…")` (default `Full`).
//! - `notes = ["…", …]`, `dyn_enums = [("prop", items_fn), …]` (default empty).
//! - `krate = <path>`: where `frap_op` is (default `::frap_op`).

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, FnArg, Ident, ItemFn, Path, Token, Type};

struct Args {
    poll: Option<Path>,
    tier: Option<Ident>,
    spec: Option<Path>,
    kind: Option<Ident>,
    support: Option<Expr>,
    notes: Option<Expr>,
    dyn_enums: Option<Expr>,
    krate: Option<Path>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut a = Args {
            poll: None,
            tier: None,
            spec: None,
            kind: None,
            support: None,
            notes: None,
            dyn_enums: None,
            krate: None,
        };
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let dup = |set: bool| -> syn::Result<()> {
                if set {
                    Err(syn::Error::new(
                        key.span(),
                        format!("duplicate argument `{key}`"),
                    ))
                } else {
                    Ok(())
                }
            };
            match key.to_string().as_str() {
                "poll" => {
                    dup(a.poll.is_some())?;
                    a.poll = Some(input.parse()?);
                }
                "tier" => {
                    dup(a.tier.is_some())?;
                    a.tier = Some(input.parse()?);
                }
                "spec" => {
                    dup(a.spec.is_some())?;
                    a.spec = Some(input.parse()?);
                }
                "kind" => {
                    dup(a.kind.is_some())?;
                    a.kind = Some(input.parse()?);
                }
                "support" => {
                    dup(a.support.is_some())?;
                    a.support = Some(input.parse()?);
                }
                "notes" => {
                    dup(a.notes.is_some())?;
                    a.notes = Some(input.parse()?);
                }
                "dyn_enums" => {
                    dup(a.dyn_enums.is_some())?;
                    a.dyn_enums = Some(input.parse()?);
                }
                "krate" => {
                    dup(a.krate.is_some())?;
                    a.krate = Some(input.parse()?);
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unexpected argument `{other}` (expected poll, tier, spec, kind, \
                             support, notes, dyn_enums, krate)"
                        ),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(a)
    }
}

/// `PrimitiveCubeAdd` → `PRIMITIVE_CUBE_ADD` (inverse of codegen's `snake → Camel`).
fn camel_to_upper_snake(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_uppercase());
    }
    out
}

/// `ns::Struct` → `ns::OPSPEC_NS_OT_STRUCT`.
fn infer_spec(ty: &Type) -> Option<Path> {
    let Type::Path(tp) = ty else { return None };
    if tp.qself.is_some() || tp.path.segments.len() < 2 {
        return None;
    }
    let segs: Vec<_> = tp.path.segments.iter().collect();
    let last = segs[segs.len() - 1];
    let ns = segs[segs.len() - 2];
    if !last.arguments.is_none() {
        return None;
    }
    let name = format!(
        "OPSPEC_{}_OT_{}",
        ns.ident.to_string().to_uppercase(),
        camel_to_upper_snake(&last.ident.to_string())
    );
    let mut path = tp.path.clone();
    let l = path.segments.last_mut().expect("invariant: ≥ 2 segments");
    l.ident = Ident::new(&name, l.ident.span());
    Some(path)
}

/// `poll::always` → `poll::ALWAYS`.
fn poll_const(mut p: Path) -> Path {
    if let Some(last) = p.segments.last_mut() {
        let s = last.ident.to_string();
        if s.chars().any(|c| c.is_lowercase()) {
            last.ident = Ident::new(&s.to_uppercase(), last.ident.span());
        }
    }
    p
}

/// `Partial("…")` → `::frap_op::Support::Partial("…")`; full paths are kept.
fn qualify(e: Expr, ty: TokenStream2) -> TokenStream2 {
    match &e {
        Expr::Path(p) if p.path.segments.len() == 1 => quote!(#ty::#e),
        Expr::Call(c) => match &*c.func {
            Expr::Path(p) if p.path.segments.len() == 1 => quote!(#ty::#e),
            _ => quote!(#e),
        },
        _ => quote!(#e),
    }
}

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand2(attr.into(), item.into()) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand2(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let args: Args = syn::parse2(attr)?;
    let f: ItemFn = syn::parse2(item)?;
    let span = Span::call_site();
    let krate = args.krate.unwrap_or_else(|| syn::parse_quote!(::frap_op));
    let poll = args
        .poll
        .ok_or_else(|| syn::Error::new(span, "missing `poll = …` (e.g. `poll = poll::ALWAYS`)"))?;
    let poll = poll_const(poll);
    let tier = args
        .tier
        .ok_or_else(|| syn::Error::new(span, "missing `tier = T0..T4`"))?;
    if !matches!(tier.to_string().as_str(), "T0" | "T1" | "T2" | "T3" | "T4") {
        return Err(syn::Error::new(
            tier.span(),
            "tier must be one of T0, T1, T2, T3, T4",
        ));
    }
    let kind = args.kind.unwrap_or_else(|| Ident::new("Normal", span));
    if !matches!(kind.to_string().as_str(), "Normal" | "Macro") {
        return Err(syn::Error::new(kind.span(), "kind must be Normal or Macro"));
    }
    let is_macro = kind == "Macro";

    // fn exec(cx: &mut OpCtx, p: P) -> OpResult
    let inputs: Vec<&FnArg> = f.sig.inputs.iter().collect();
    if inputs.len() != 2 {
        return Err(syn::Error::new_spanned(
            &f.sig,
            "#[operator] expects `fn(cx: &mut OpCtx, p: <generated props type>) -> OpResult`",
        ));
    }
    let FnArg::Typed(props) = inputs[1] else {
        return Err(syn::Error::new_spanned(
            inputs[1],
            "expected a typed parameter",
        ));
    };
    let props_ty: &Type = &props.ty;
    let spec = match args.spec {
        Some(s) => s,
        None => infer_spec(props_ty).ok_or_else(|| {
            syn::Error::new_spanned(
                props_ty,
                "cannot name the OperatorSpec static from this type: write the props type with \
                 its namespace module (`mesh::PrimitiveCubeAdd`) or pass `spec = OPSPEC_…`",
            )
        })?,
    };
    let support = match args.support {
        Some(e) => qualify(e, quote!(#krate::Support)),
        None => quote!(#krate::Support::Full),
    };
    let notes = match args.notes {
        Some(e) => quote!(&#e),
        None => quote!(&[]),
    };
    let dyn_enums = match args.dyn_enums {
        Some(Expr::Array(a)) => {
            let items: Punctuated<TokenStream2, Token![,]> = a
                .elems
                .iter()
                .map(|e| match e {
                    Expr::Tuple(t) if t.elems.len() == 2 => {
                        let (p, func) = (&t.elems[0], &t.elems[1]);
                        Ok(quote!((#p, #func as #krate::DynEnumFn)))
                    }
                    other => Err(syn::Error::new_spanned(
                        other,
                        "expected `(\"prop\", items_fn)`",
                    )),
                })
                .collect::<syn::Result<_>>()?;
            quote!(&[#items])
        }
        Some(other) => {
            return Err(syn::Error::new_spanned(
                other,
                "dyn_enums must be an array of `(\"prop\", items_fn)`",
            ));
        }
        None => quote!(&[]),
    };

    let ident = &f.sig.ident;
    let up = ident.to_string().to_uppercase();
    let wrapper = format_ident!("__frap_exec_{}", ident);
    let stat = format_ident!("__FRAP_OPERATOR_{}", up);
    let test = format_ident!("__frap_operator_spec_{}", ident);
    let kind_check = if is_macro {
        quote!(assert!(!spec.macro_steps.is_empty(), "kind = Macro but {} has no macro steps", spec.idname);)
    } else {
        quote!(assert!(spec.macro_steps.is_empty(), "{} is a macro: pass kind = Macro", spec.idname);)
    };

    Ok(quote! {
        #f

        #[doc(hidden)]
        fn #wrapper(
            cx: &mut #krate::OpCtx<'_>,
            args: &#krate::prelude::OpArgs,
        ) -> #krate::OpResult {
            #ident(cx, <#props_ty as #krate::prelude::OpProps>::from_args(args))
        }

        #[doc(hidden)]
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[#krate::linkme::distributed_slice(#krate::OPERATORS)]
        #[linkme(crate = #krate::linkme)]
        static #stat: #krate::OperatorDef = #krate::OperatorDef {
            spec: &#spec,
            poll: #poll,
            exec: #wrapper,
            kind: #krate::OpKind::#kind,
            dyn_enums: #dyn_enums,
            tier: #krate::Tier::#tier,
            support: #support,
            notes: #notes,
        };

        #[cfg(test)]
        #[test]
        fn #test() {
            let spec = <#props_ty as #krate::prelude::OpProps>::spec();
            assert!(
                ::core::ptr::eq(spec, #stat.spec),
                "#[operator] registered another spec than {}::spec()",
                <#props_ty as #krate::prelude::OpProps>::BL_IDNAME
            );
            assert_eq!(#stat.spec.idname, <#props_ty as #krate::prelude::OpProps>::IDNAME);
            #kind_check
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(
            camel_to_upper_snake("PrimitiveCubeAdd"),
            "PRIMITIVE_CUBE_ADD"
        );
        assert_eq!(camel_to_upper_snake("Gltf"), "GLTF");
        let ty: Type = syn::parse_quote!(frap_spec_ops_mesh::mesh::PrimitiveUvSphereAdd);
        let p = infer_spec(&ty).unwrap();
        assert_eq!(
            quote!(#p).to_string().replace(' ', ""),
            "frap_spec_ops_mesh::mesh::OPSPEC_MESH_OT_PRIMITIVE_UV_SPHERE_ADD"
        );
        let ty: Type = syn::parse_quote!(P);
        assert!(infer_spec(&ty).is_none());
        let p: Path = syn::parse_quote!(poll::always);
        let p = poll_const(p);
        assert_eq!(quote!(#p).to_string().replace(' ', ""), "poll::ALWAYS");
    }

    #[test]
    fn rejects_bad_arguments() {
        let ok = |a: TokenStream2| {
            expand2(
                a,
                quote!(
                    fn exec(cx: &mut OpCtx, p: mesh::X) -> OpResult {
                        Ok(OpStatus::Finished)
                    }
                ),
            )
        };
        assert!(ok(quote!(poll = poll::ALWAYS, tier = T1)).is_ok());
        assert!(ok(quote!(tier = T1)).is_err());
        assert!(ok(quote!(poll = poll::ALWAYS)).is_err());
        assert!(ok(quote!(poll = poll::ALWAYS, tier = T9)).is_err());
        assert!(ok(quote!(poll = poll::ALWAYS, tier = T1, kind = Weird)).is_err());
        assert!(ok(quote!(poll = poll::ALWAYS, tier = T1, frob = 1)).is_err());
        let bare = expand2(
            quote!(poll = poll::ALWAYS, tier = T1),
            quote!(
                fn exec(cx: &mut OpCtx, p: P) -> OpResult {
                    Ok(OpStatus::Finished)
                }
            ),
        );
        assert!(bare.is_err());
    }
}
