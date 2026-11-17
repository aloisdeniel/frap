// SPDX-License-Identifier: GPL-3.0-or-later
//! `rna_struct!` (§9.4, §9.6). Owned by WP 1.2.
//!
//! ```ignore
//! rna_struct! { Object as frap_spec_rna::OBJECT {
//!     name            => header.name (set = hook),       // field read, write through RNA_HOOKS
//!     location        => location (tag = Vector),        // [f32; 3] <-> FloatArray { tag: Vector }
//!     rotation_euler  => rotation_euler (tag = Euler(rotation_mode)),
//!     rotation_mode   => hook,                           // get and set through RNA_HOOKS
//!     data            => id(data),                       // ID pointer (Field::Id)
//!     type            => readonly(object_type()),        // `self.object_type()`, owned value
//!     dimensions      => get(|o| o.dims()) set(|o, v| o.set_dims(v)),
//!     pose            => struct(pose),                   // nested in-memory struct
//!     node_tree       => embedded(node_tree, NodeTree),  // embedded ID
//!     modifiers       => coll(modifiers, api = "ObjectModifiers"),
//!     materials       => id_coll(materials, item = frap_spec_rna::MATERIAL),
//!     material_slots  => hook_coll,                      // collection proxy from a get hook
//!     name            => hook(on = "ID"),                // hook registered on a base struct
//!     children        => python_only hook,
//!     vertices        => soa(MeshVertex as frap_spec_rna::MESH_VERTEX, len = |m| m.verts_num()) {
//!         co    => get(|m, i| m.positions()[i]) set(|m, i, v| m.set_position(i, v)) (tag = Vector),
//!         index => get(|_, i| i as i32),
//!         hide  => hook,
//!     },
//! }}
//! ```
//!
//! Generates `impl Rna`, `impl RnaMut`, `impl RnaStruct`, `impl testing::RnaBindings` (for
//! `check_bindings`), one `HookDecl` in `HOOK_DECLS` per hook binding, and for every `soa(…)`
//! binding the proxy types `<Item>Ref`/`<Item>Mut` (items) and `<Item>Coll`/`<Item>CollMut`
//! (collections). Computed getters (`readonly(…)`, `get(…)`) must return owned values.
//! Closure parameters are typed by the macro: owner `&T` (`&mut T` in setters), item index
//! `usize`, new value inferred from the setter's use of it.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Expr, ExprClosure, Ident, LitStr, Member, Pat, Path, Token, Visibility, braced, parenthesized,
};

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match syn::parse::<StructSpec>(input).and_then(|s| generate(&s)) {
        Ok(ts) => ts.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// `ident(.member)*`
#[derive(Clone)]
struct FieldPath(Vec<Member>);

impl FieldPath {
    fn parse_after(first: Ident, input: ParseStream) -> syn::Result<Self> {
        let mut v = vec![Member::Named(first)];
        while input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            v.push(input.parse::<Member>()?);
        }
        Ok(FieldPath(v))
    }
    fn on(&self, recv: TokenStream2) -> TokenStream2 {
        let m = &self.0;
        quote!(#recv #(. #m)*)
    }
}

impl Parse for FieldPath {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let first = Ident::parse_any(input)?;
        FieldPath::parse_after(first, input)
    }
}

enum Tag {
    Default,
    Const(Ident),
    Euler(FieldPath),
}

struct Opts {
    tag: Tag,
    set_hook: bool,
}

impl Opts {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut o = Opts {
            tag: Tag::Default,
            set_hook: false,
        };
        if !input.peek(syn::token::Paren) {
            return Ok(o);
        }
        let content;
        parenthesized!(content in input);
        while !content.is_empty() {
            let key = Ident::parse_any(&content)?;
            content.parse::<Token![=]>()?;
            match key.to_string().as_str() {
                "tag" => {
                    let t: Ident = content.parse()?;
                    o.tag = if t == "Euler" && content.peek(syn::token::Paren) {
                        let inner;
                        parenthesized!(inner in content);
                        Tag::Euler(inner.parse()?)
                    } else {
                        match t.to_string().as_str() {
                            "None" | "Vector" | "Euler" | "Quaternion" | "AxisAngle" | "Matrix"
                            | "Color" => Tag::Const(t),
                            _ => {
                                return Err(syn::Error::new(
                                    t.span(),
                                    "tag must be None, Vector, Euler, Euler(field), Quaternion, \
                                     AxisAngle, Matrix or Color",
                                ));
                            }
                        }
                    };
                }
                "set" => {
                    let h = Ident::parse_any(&content)?;
                    if h != "hook" {
                        return Err(syn::Error::new(h.span(), "expected `set = hook`"));
                    }
                    o.set_hook = true;
                }
                _ => return Err(syn::Error::new(key.span(), "expected `tag` or `set`")),
            }
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        Ok(o)
    }
}

enum Kind {
    Field(FieldPath),
    ReadOnly(Expr),
    Computed {
        get: ExprClosure,
        set: Option<ExprClosure>,
    },
    Hook {
        coll: bool,
        python_only: bool,
        on: Option<LitStr>,
    },
    Id(FieldPath),
    Struct(FieldPath),
    Embedded(FieldPath, Ident),
    Coll {
        field: FieldPath,
        item: Option<Path>,
        ids: bool,
    },
    Soa(Box<SoaSpec>),
}

struct SoaSpec {
    vis: Visibility,
    item: Ident,
    path: Path,
    len: ExprClosure,
    foreach_get: Option<ExprClosure>,
    foreach_set: Option<ExprClosure>,
    bindings: Vec<Binding>,
}

struct Binding {
    prop: Ident,
    kind: Kind,
    opts: Opts,
}

impl Binding {
    fn prop_str(&self) -> String {
        self.prop.unraw().to_string()
    }
}

fn parse_hook_on(input: ParseStream) -> syn::Result<Option<LitStr>> {
    if !input.peek(syn::token::Paren) {
        return Ok(None);
    }
    let content;
    parenthesized!(content in input);
    let key = Ident::parse_any(&content)?;
    if key != "on" {
        return Err(syn::Error::new(key.span(), "expected `on = \"Struct\"`"));
    }
    content.parse::<Token![=]>()?;
    Ok(Some(content.parse()?))
}

/// `key = value` arguments after the first one in `coll(…)`, `id_coll(…)`, `soa(…)`.
fn parse_kv<T>(
    content: ParseStream,
    mut f: impl FnMut(&Ident, ParseStream) -> syn::Result<T>,
) -> syn::Result<()> {
    while !content.is_empty() {
        content.parse::<Token![,]>()?;
        if content.is_empty() {
            break;
        }
        let key = Ident::parse_any(content)?;
        content.parse::<Token![=]>()?;
        f(&key, content)?;
    }
    Ok(())
}

impl Parse for Binding {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let prop = Ident::parse_any(input)?;
        input.parse::<Token![=>]>()?;
        let head = Ident::parse_any(input)?;
        let head_s = head.to_string();
        let paren = input.peek(syn::token::Paren);
        let kind = match head_s.as_str() {
            "hook" => Kind::Hook {
                coll: false,
                python_only: false,
                on: parse_hook_on(input)?,
            },
            "hook_coll" => Kind::Hook {
                coll: true,
                python_only: false,
                on: parse_hook_on(input)?,
            },
            "python_only" => {
                let h = Ident::parse_any(input)?;
                let coll = match h.to_string().as_str() {
                    "hook" => false,
                    "hook_coll" => true,
                    _ => {
                        return Err(syn::Error::new(
                            h.span(),
                            "expected `python_only hook` or `python_only hook_coll`",
                        ));
                    }
                };
                Kind::Hook {
                    coll,
                    python_only: true,
                    on: parse_hook_on(input)?,
                }
            }
            "readonly" if paren => {
                let content;
                parenthesized!(content in input);
                Kind::ReadOnly(content.parse()?)
            }
            "get" if paren => {
                let content;
                parenthesized!(content in input);
                let get: ExprClosure = content.parse()?;
                let set = if input.peek(Ident) && input.fork().parse::<Ident>()? == "set" {
                    input.parse::<Ident>()?;
                    let content;
                    parenthesized!(content in input);
                    Some(content.parse::<ExprClosure>()?)
                } else {
                    None
                };
                Kind::Computed { get, set }
            }
            "id" if paren => {
                let content;
                parenthesized!(content in input);
                Kind::Id(content.parse()?)
            }
            "struct" if paren => {
                let content;
                parenthesized!(content in input);
                Kind::Struct(content.parse()?)
            }
            "embedded" if paren => {
                let content;
                parenthesized!(content in input);
                let f: FieldPath = content.parse()?;
                content.parse::<Token![,]>()?;
                let slot: Ident = content.parse()?;
                Kind::Embedded(f, slot)
            }
            "coll" | "id_coll" if paren => {
                let content;
                parenthesized!(content in input);
                let field: FieldPath = content.parse()?;
                let mut item = None;
                parse_kv(&content, |key, c| {
                    match key.to_string().as_str() {
                        "item" => item = Some(c.parse::<Path>()?),
                        "api" => {
                            c.parse::<LitStr>()?;
                        }
                        _ => return Err(syn::Error::new(key.span(), "expected `item` or `api`")),
                    }
                    Ok(())
                })?;
                let ids = head_s == "id_coll";
                if ids && item.is_none() {
                    return Err(syn::Error::new(
                        head.span(),
                        "id_coll(…) needs `item = <StructDef path>`",
                    ));
                }
                Kind::Coll { field, item, ids }
            }
            "soa" if paren => {
                let content;
                parenthesized!(content in input);
                let vis: Visibility = content.parse()?;
                let item: Ident = content.parse()?;
                content.parse::<Token![as]>()?;
                let path: Path = content.parse()?;
                let mut len = None;
                let mut foreach_get = None;
                let mut foreach_set = None;
                parse_kv(&content, |key, c| {
                    match key.to_string().as_str() {
                        "len" => len = Some(c.parse::<ExprClosure>()?),
                        "foreach_get" => foreach_get = Some(c.parse::<ExprClosure>()?),
                        "foreach_set" => foreach_set = Some(c.parse::<ExprClosure>()?),
                        _ => {
                            return Err(syn::Error::new(
                                key.span(),
                                "expected `len`, `foreach_get` or `foreach_set`",
                            ));
                        }
                    }
                    Ok(())
                })?;
                let len = len.ok_or_else(|| {
                    syn::Error::new(head.span(), "soa(…) needs `len = |owner| …`")
                })?;
                let body;
                braced!(body in input);
                let bindings = Punctuated::<Binding, Token![,]>::parse_terminated(&body)?
                    .into_iter()
                    .collect();
                Kind::Soa(Box::new(SoaSpec {
                    vis,
                    item,
                    path,
                    len,
                    foreach_get,
                    foreach_set,
                    bindings,
                }))
            }
            _ => Kind::Field(FieldPath::parse_after(head, input)?),
        };
        let opts = Opts::parse(input)?;
        Ok(Binding { prop, kind, opts })
    }
}

struct StructSpec {
    ty: Path,
    path: Path,
    srna: Option<ExprClosure>,
    bindings: Vec<Binding>,
}

impl Parse for StructSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ty: Path = input.parse()?;
        input.parse::<Token![as]>()?;
        let path: Path = input.parse()?;
        let mut srna = None;
        if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let key: Ident = content.parse()?;
            if key != "srna" {
                return Err(syn::Error::new(key.span(), "expected `srna = |s| …`"));
            }
            content.parse::<Token![=]>()?;
            srna = Some(content.parse()?);
        }
        let body;
        braced!(body in input);
        let bindings = Punctuated::<Binding, Token![,]>::parse_terminated(&body)?
            .into_iter()
            .collect();
        Ok(StructSpec {
            ty,
            path,
            srna,
            bindings,
        })
    }
}

/// The user's annotation of a closure parameter (`|t, v: String| …`), if any.
fn param_type(c: &ExprClosure, i: usize) -> Option<TokenStream2> {
    match c.inputs.iter().nth(i) {
        Some(Pat::Type(t)) => {
            let ty = &t.ty;
            Some(quote!(: #ty))
        }
        _ => None,
    }
}

fn closure_params(c: &ExprClosure, n: usize, what: &str) -> syn::Result<Vec<Pat>> {
    if c.inputs.len() != n {
        return Err(syn::Error::new_spanned(
            c,
            format!("{what} takes {n} closure parameter(s)"),
        ));
    }
    Ok(c.inputs
        .iter()
        .map(|p| match p {
            Pat::Type(t) => (*t.pat).clone(),
            other => other.clone(),
        })
        .collect())
}

fn tag_expr(opts: &Opts, owner: Option<&TokenStream2>) -> syn::Result<TokenStream2> {
    Ok(match &opts.tag {
        Tag::Default => quote!(::frap_rna::MathTag::for_subtype(prop.subtype)),
        Tag::Const(t) if t == "Euler" => {
            quote!(::frap_rna::MathTag::Euler(::frap_rna::EulerOrder::XYZ))
        }
        Tag::Const(t) => quote!(::frap_rna::MathTag::#t),
        Tag::Euler(f) => {
            let Some(owner) = owner else {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "tag = Euler(field) is not available inside soa(…)",
                ));
            };
            let f = f.on(owner.clone());
            quote!(::frap_rna::MathTag::Euler(::frap_rna::traits::AsEulerOrder::euler_order(&#f)))
        }
    })
}

/// Generated pieces for one struct (or one SoA item proxy).
#[derive(Default)]
struct Arms {
    get: Vec<TokenStream2>,
    set: Vec<TokenStream2>,
    struct_mut: Vec<TokenStream2>,
    coll_mut: Vec<TokenStream2>,
    decls: Vec<TokenStream2>,
    check_fns: Vec<TokenStream2>,
    hook_decls: Vec<TokenStream2>,
    extra: Vec<TokenStream2>,
}

struct Counter(usize);
impl Counter {
    fn next(&mut self, prefix: &str) -> Ident {
        self.0 += 1;
        format_ident!("__frap_{}_{}", prefix, self.0)
    }
}

fn hook_decl(srna: TokenStream2, prop: &str, coll: bool) -> TokenStream2 {
    quote! {
        #[allow(unsafe_code)]
        const _: () = {
            #[allow(unsafe_code)]
            #[::frap_rna::linkme::distributed_slice(::frap_rna::testing::HOOK_DECLS)]
            #[linkme(crate = ::frap_rna::linkme)]
            static __FRAP_HOOK_DECL: ::frap_rna::testing::HookDecl = ::frap_rna::testing::HookDecl {
                srna: #srna,
                prop: #prop,
                coll: #coll,
            };
        };
    }
}

fn decl(
    prop: &str,
    kind: &str,
    check: Option<&Ident>,
    item: Option<&Path>,
    soa: Option<&Ident>,
) -> TokenStream2 {
    let kind = format_ident!("{}", kind);
    let check = match check {
        Some(c) => quote!(::core::option::Option::Some(#c)),
        None => quote!(::core::option::Option::None),
    };
    let item = match item {
        Some(p) => quote!(::core::option::Option::Some({
            fn __item() -> &'static ::frap_rna::StructDef { &#p }
            __item
        })),
        None => quote!(::core::option::Option::None),
    };
    let soa = match soa {
        Some(s) => quote!(::core::option::Option::Some(#s)),
        None => quote!(::core::option::Option::None),
    };
    quote! {
        ::frap_rna::testing::BindingDecl {
            prop: #prop,
            kind: ::frap_rna::testing::BindingKind::#kind,
            check: #check,
            item: #item,
            soa: #soa,
        }
    }
}

fn read_only_err() -> TokenStream2 {
    quote!(::core::result::Result::Err(::frap_rna::traits::read_only(
        ::frap_rna::Rna::srna(self),
        prop.identifier
    )))
}

fn readonly_guard() -> TokenStream2 {
    quote! {
        if prop.is_readonly() {
            return ::core::result::Result::Err(::frap_rna::traits::read_only(
                ::frap_rna::Rna::srna(self),
                prop.identifier,
            ));
        }
    }
}

fn set_result(body: &Expr) -> TokenStream2 {
    quote! {
        let __frap_r = #body;
        ::frap_rna::testing::__macro::SetResult::into_result(__frap_r)?;
        ::core::result::Result::Ok(::frap_rna::SetOutcome::Done)
    }
}

/// Bindings of the main struct (receiver `self`).
fn struct_arms(spec: &StructSpec, arms: &mut Arms, n: &mut Counter) -> syn::Result<()> {
    let ty = &spec.ty;
    let path = &spec.path;
    for b in &spec.bindings {
        let p = b.prop_str();
        let me = quote!(self);
        if let (true, false) = (b.opts.set_hook, matches!(b.kind, Kind::Field(_))) {
            return Err(syn::Error::new(
                b.prop.span(),
                "`set = hook` applies to field bindings only",
            ));
        }
        match &b.kind {
            Kind::Field(f) => {
                let tag = tag_expr(&b.opts, Some(&me))?;
                let fe = f.on(quote!(self));
                arms.get.push(quote! {
                    #p => ::core::result::Result::Ok(::frap_rna::Field::Value(
                        ::frap_rna::traits::ToRnaValue::to_rna_value(&#fe, prop, #tag),
                    )),
                });
                let fs = f.on(quote!(__s));
                let check = n.next("check");
                arms.check_fns.push(quote! {
                    fn #check(prop: &::frap_rna::PropDef) -> ::core::result::Result<(), ::std::string::String> {
                        (::frap_rna::testing::__macro::check_field(|__s: &#ty| &#fs))(prop)
                    }
                });
                if b.opts.set_hook {
                    arms.set.push(quote! {
                        #p => ::core::result::Result::Ok(::frap_rna::SetOutcome::NeedsHook),
                    });
                    arms.hook_decls
                        .push(hook_decl(quote!(#path.identifier), &p, false));
                    arms.decls
                        .push(decl(&p, "SetHook", Some(&check), None, None));
                } else {
                    let guard = readonly_guard();
                    arms.set.push(quote! {
                        #p => {
                            #guard
                            #fe = ::frap_rna::traits::FromRnaValue::from_rna_value(prop, v)?;
                            ::core::result::Result::Ok(::frap_rna::SetOutcome::Done)
                        }
                    });
                    arms.decls.push(decl(&p, "Value", Some(&check), None, None));
                }
            }
            Kind::ReadOnly(e) => {
                let tag = tag_expr(&b.opts, Some(&me))?;
                let (val, chk) = match e {
                    Expr::Closure(c) => {
                        let ps = closure_params(c, 1, "readonly(…)")?;
                        let p0 = &ps[0];
                        let body = &c.body;
                        (
                            quote!({ let #p0: &#ty = self; #body }),
                            quote!(|#p0: &#ty| #body),
                        )
                    }
                    other => (quote!(self.#other), quote!(|__s: &#ty| __s.#other)),
                };
                arms.get.push(quote! {
                    #p => {
                        #[allow(unused_imports)]
                        use ::frap_rna::traits::ToRnaValue as _;
                        let __frap_v = #val;
                        ::core::result::Result::Ok(::frap_rna::Field::Value(
                            __frap_v.to_rna_value(prop, #tag),
                        ))
                    }
                });
                let ro = read_only_err();
                arms.set.push(quote!(#p => #ro,));
                let check = n.next("check");
                arms.check_fns.push(quote! {
                    fn #check(prop: &::frap_rna::PropDef) -> ::core::result::Result<(), ::std::string::String> {
                        (::frap_rna::testing::__macro::check_value(#chk))(prop)
                    }
                });
                arms.decls
                    .push(decl(&p, "ReadOnly", Some(&check), None, None));
            }
            Kind::Computed { get, set } => {
                let tag = tag_expr(&b.opts, Some(&me))?;
                let gp = closure_params(get, 1, "get(…)")?;
                let g0 = &gp[0];
                let gbody = &get.body;
                arms.get.push(quote! {
                    #p => {
                        #[allow(unused_imports)]
                        use ::frap_rna::traits::ToRnaValue as _;
                        let __frap_v = { let #g0: &#ty = self; #gbody };
                        ::core::result::Result::Ok(::frap_rna::Field::Value(
                            __frap_v.to_rna_value(prop, #tag),
                        ))
                    }
                });
                let check = n.next("check");
                arms.check_fns.push(quote! {
                    fn #check(prop: &::frap_rna::PropDef) -> ::core::result::Result<(), ::std::string::String> {
                        (::frap_rna::testing::__macro::check_value(|#g0: &#ty| #gbody))(prop)
                    }
                });
                match set {
                    Some(s) => {
                        let sp = closure_params(s, 2, "set(…)")?;
                        let (s0, s1) = (&sp[0], &sp[1]);
                        let s1t = param_type(s, 1);
                        let guard = readonly_guard();
                        let res = set_result(&s.body);
                        arms.set.push(quote! {
                            #p => {
                                #guard
                                let #s1 #s1t = ::frap_rna::traits::FromRnaValue::from_rna_value(prop, v)?;
                                let #s0: &mut #ty = self;
                                #res
                            }
                        });
                        arms.decls.push(decl(&p, "Value", Some(&check), None, None));
                    }
                    None => {
                        let ro = read_only_err();
                        arms.set.push(quote!(#p => #ro,));
                        arms.decls
                            .push(decl(&p, "ReadOnly", Some(&check), None, None));
                    }
                }
            }
            Kind::Hook {
                coll,
                python_only,
                on,
            } => {
                arms.get
                    .push(quote!(#p => ::core::result::Result::Ok(::frap_rna::Field::NeedsHook),));
                if *coll {
                    let ro = read_only_err();
                    arms.set.push(quote!(#p => #ro,));
                    arms.coll_mut.push(quote! {
                        #p => ::core::result::Result::Err(::frap_rna::traits::coll_needs_hook(
                            ::frap_rna::Rna::srna(self),
                            prop.identifier,
                        )),
                    });
                } else {
                    arms.set.push(
                        quote!(#p => ::core::result::Result::Ok(::frap_rna::SetOutcome::NeedsHook),),
                    );
                }
                let srna = match on {
                    Some(l) => quote!(#l),
                    None => quote!(#path.identifier),
                };
                arms.hook_decls.push(hook_decl(srna, &p, *coll));
                let kind = match (python_only, coll) {
                    (true, _) => "PythonOnlyHook",
                    (false, true) => "HookColl",
                    (false, false) => "Hook",
                };
                arms.decls.push(decl(&p, kind, None, None, None));
            }
            Kind::Id(f) => {
                let fe = f.on(quote!(self));
                let fs = f.on(quote!(__s));
                arms.get.push(quote! {
                    #p => ::core::result::Result::Ok(::frap_rna::Field::Id(
                        ::frap_rna::traits::IdField::get_id(&#fe),
                    )),
                });
                let guard = readonly_guard();
                arms.set.push(quote! {
                    #p => {
                        #guard
                        #fe = ::frap_rna::traits::IdField::set_id(prop, v)?;
                        ::core::result::Result::Ok(::frap_rna::SetOutcome::Done)
                    }
                });
                let check = n.next("check");
                arms.check_fns.push(quote! {
                    fn #check(prop: &::frap_rna::PropDef) -> ::core::result::Result<(), ::std::string::String> {
                        (::frap_rna::testing::__macro::check_id(|__s: &#ty| &#fs))(prop)
                    }
                });
                arms.decls.push(decl(&p, "Id", Some(&check), None, None));
            }
            Kind::Struct(f) | Kind::Embedded(f, _) => {
                let fe = f.on(quote!(self));
                let field = match &b.kind {
                    Kind::Embedded(_, slot) => quote! {
                        ::frap_rna::Field::Embedded(
                            ::frap_rna::EmbeddedSlot::#slot,
                            ::frap_rna::StructRef::Borrowed(&#fe),
                        )
                    },
                    _ => quote!(::frap_rna::Field::Struct(::frap_rna::StructRef::Borrowed(&#fe))),
                };
                arms.get
                    .push(quote!(#p => ::core::result::Result::Ok(#field),));
                let ro = read_only_err();
                arms.set.push(quote!(#p => #ro,));
                arms.struct_mut.push(quote! {
                    #p => ::core::result::Result::Ok(::std::boxed::Box::new(&mut #fe)),
                });
                let kind = if matches!(b.kind, Kind::Embedded(..)) {
                    "Embedded"
                } else {
                    "Struct"
                };
                arms.decls.push(decl(&p, kind, None, None, None));
            }
            Kind::Coll { field, item, ids } => {
                let fe = field.on(quote!(self));
                let (view, view_mut) = if *ids {
                    (
                        quote!(::frap_rna::traits::IdVecColl),
                        quote!(::frap_rna::traits::IdVecCollMut),
                    )
                } else {
                    (
                        quote!(::frap_rna::traits::VecColl),
                        quote!(::frap_rna::traits::VecCollMut),
                    )
                };
                let item_ty = match item {
                    Some(it) => quote!(&#it),
                    None => quote!(::frap_rna::traits::vec_item_type(&#fe)),
                };
                arms.get.push(quote! {
                    #p => ::core::result::Result::Ok(::frap_rna::Field::Coll(
                        ::frap_rna::CollRef::Owned(::std::boxed::Box::new(#view::new(&#fe, #item_ty))),
                    )),
                });
                let ro = read_only_err();
                arms.set.push(quote!(#p => #ro,));
                arms.coll_mut.push(quote! {
                    #p => {
                        let __frap_item = #item_ty;
                        ::core::result::Result::Ok(::std::boxed::Box::new(#view_mut::new(&mut #fe, __frap_item)))
                    }
                });
                let kind = if *ids { "IdColl" } else { "Coll" };
                let check = if item.is_none() && !*ids {
                    let check = n.next("check");
                    let fs = field.on(quote!(__s));
                    arms.check_fns.push(quote! {
                        fn #check(prop: &::frap_rna::PropDef) -> ::core::result::Result<(), ::std::string::String> {
                            (::frap_rna::testing::__macro::check_coll(|__s: &#ty| &#fs))(prop)
                        }
                    });
                    Some(check)
                } else {
                    None
                };
                arms.decls
                    .push(decl(&p, kind, check.as_ref(), item.as_ref(), None));
            }
            Kind::Soa(soa) => {
                soa_struct(spec, b, soa, arms, n)?;
            }
        }
    }
    Ok(())
}

fn soa_names(item: &Ident) -> (Ident, Ident, Ident, Ident) {
    (
        format_ident!("{}Ref", item),
        format_ident!("{}Mut", item),
        format_ident!("{}Coll", item),
        format_ident!("{}CollMut", item),
    )
}

fn soa_struct(
    spec: &StructSpec,
    b: &Binding,
    soa: &SoaSpec,
    arms: &mut Arms,
    n: &mut Counter,
) -> syn::Result<()> {
    let ty = &spec.ty;
    let p = b.prop_str();
    let (rf, mt, coll, coll_mut) = soa_names(&soa.item);
    let vis = &soa.vis;
    let item_path = &soa.path;

    arms.get.push(quote! {
        #p => ::core::result::Result::Ok(::frap_rna::Field::Coll(
            ::frap_rna::CollRef::Owned(::std::boxed::Box::new(#coll { owner: self })),
        )),
    });
    let ro = read_only_err();
    arms.set.push(quote!(#p => #ro,));
    arms.coll_mut.push(quote! {
        #p => ::core::result::Result::Ok(::std::boxed::Box::new(#coll_mut { owner: self })),
    });

    // Item proxies.
    let mut item = Arms::default();
    for ib in &soa.bindings {
        let ip = ib.prop_str();
        match &ib.kind {
            Kind::Computed { get, set } => {
                let tag = tag_expr(&ib.opts, None)?;
                let gp = closure_params(get, 2, "get(…) inside soa(…)")?;
                let (g0, g1) = (&gp[0], &gp[1]);
                let gbody = &get.body;
                item.get.push(quote! {
                    #ip => {
                        #[allow(unused_imports)]
                        use ::frap_rna::traits::ToRnaValue as _;
                        let __frap_v = { let #g0: &#ty = __frap_owner; let #g1: usize = self.i; #gbody };
                        ::core::result::Result::Ok(::frap_rna::Field::Value(
                            __frap_v.to_rna_value(prop, #tag),
                        ))
                    }
                });
                let check = n.next("check");
                item.check_fns.push(quote! {
                    fn #check(prop: &::frap_rna::PropDef) -> ::core::result::Result<(), ::std::string::String> {
                        (::frap_rna::testing::__macro::check_item(|#g0: &#ty, #g1: usize| #gbody))(prop)
                    }
                });
                match set {
                    Some(s) => {
                        let sp = closure_params(s, 3, "set(…) inside soa(…)")?;
                        let (s0, s1, s2) = (&sp[0], &sp[1], &sp[2]);
                        let s2t = param_type(s, 2);
                        let guard = readonly_guard();
                        let res = set_result(&s.body);
                        item.set.push(quote! {
                            #ip => {
                                #guard
                                let #s2 #s2t = ::frap_rna::traits::FromRnaValue::from_rna_value(prop, v)?;
                                let #s1: usize = self.i;
                                let #s0: &mut #ty = &mut *self.owner;
                                #res
                            }
                        });
                        item.decls
                            .push(decl(&ip, "Value", Some(&check), None, None));
                    }
                    None => {
                        let ro = read_only_err();
                        item.set.push(quote!(#ip => #ro,));
                        item.decls
                            .push(decl(&ip, "ReadOnly", Some(&check), None, None));
                    }
                }
            }
            Kind::Hook {
                coll,
                python_only,
                on,
            } => {
                item.get
                    .push(quote!(#ip => ::core::result::Result::Ok(::frap_rna::Field::NeedsHook),));
                if *coll {
                    let ro = read_only_err();
                    item.set.push(quote!(#ip => #ro,));
                } else {
                    item.set.push(
                        quote!(#ip => ::core::result::Result::Ok(::frap_rna::SetOutcome::NeedsHook),),
                    );
                }
                let srna = match on {
                    Some(l) => quote!(#l),
                    None => quote!(#item_path.identifier),
                };
                arms.hook_decls.push(hook_decl(srna, &ip, *coll));
                let kind = match (python_only, coll) {
                    (true, _) => "PythonOnlyHook",
                    (false, true) => "HookColl",
                    (false, false) => "Hook",
                };
                item.decls.push(decl(&ip, kind, None, None, None));
            }
            _ => {
                return Err(syn::Error::new(
                    ib.prop.span(),
                    "inside soa(…) use `get(|owner, i| …) [set(|owner, i, v| …)]` or a hook",
                ));
            }
        }
    }

    // Item bindings for check_bindings.
    let soa_fn = n.next("soa");
    let item_checks = &item.check_fns;
    let item_decls = &item.decls;
    let k = item_decls.len();
    arms.check_fns.push(quote! {
        fn #soa_fn() -> (&'static ::frap_rna::StructDef, &'static [::frap_rna::testing::BindingDecl]) {
            #(#item_checks)*
            static __FRAP_ITEM_BINDINGS: [::frap_rna::testing::BindingDecl; #k] = [#(#item_decls),*];
            (&#item_path, &__FRAP_ITEM_BINDINGS)
        }
    });
    arms.decls
        .push(decl(&p, "Soa", None, Some(item_path), Some(&soa_fn)));

    // len / foreach.
    let lp = closure_params(&soa.len, 1, "len = |owner| …")?;
    let l0 = &lp[0];
    let lbody = &soa.len.body;
    let len_fn = |owner: TokenStream2| {
        quote! {
            fn len(&self) -> usize {
                let #l0: &#ty = #owner;
                #lbody
            }
        }
    };
    let len_ref = len_fn(quote!(self.owner));
    let len_mut = len_fn(quote!(&*self.owner));
    let fg = |owner: TokenStream2| -> syn::Result<TokenStream2> {
        let Some(c) = &soa.foreach_get else {
            return Ok(quote!());
        };
        let ps = closure_params(c, 3, "foreach_get = |owner, prop, range| …")?;
        let (a, bb, cc) = (&ps[0], &ps[1], &ps[2]);
        let body = &c.body;
        Ok(quote! {
            fn foreach_get(
                &self,
                prop: &'static ::frap_rna::PropDef,
                range: ::core::ops::Range<usize>,
            ) -> ::core::result::Result<::frap_rna::Bulk, ::frap_rna::RnaError> {
                let #a: &#ty = #owner;
                let #bb: &'static ::frap_rna::PropDef = prop;
                let #cc: ::core::ops::Range<usize> = range;
                #body
            }
        })
    };
    let fg_ref = fg(quote!(self.owner))?;
    let fg_mut = fg(quote!(&*self.owner))?;
    let fs = match &soa.foreach_set {
        None => quote!(),
        Some(c) => {
            let ps = closure_params(c, 3, "foreach_set = |owner, prop, data| …")?;
            let (a, bb, cc) = (&ps[0], &ps[1], &ps[2]);
            let body = &c.body;
            quote! {
                fn foreach_set(
                    &mut self,
                    prop: &'static ::frap_rna::PropDef,
                    data: &::frap_rna::Bulk,
                ) -> ::core::result::Result<(), ::frap_rna::RnaError> {
                    let #a: &mut #ty = &mut *self.owner;
                    let #bb: &'static ::frap_rna::PropDef = prop;
                    let #cc: &::frap_rna::Bulk = data;
                    #body
                }
            }
        }
    };

    let item_get = &item.get;
    let item_set = &item.set;
    let doc_ref = format!(
        "Read proxy of one `{}` item (generated by `rna_struct!`).",
        soa.item
    );
    let doc_mut = format!(
        "Write proxy of one `{}` item (generated by `rna_struct!`).",
        soa.item
    );
    let doc_coll = format!("The `{p}` collection (generated by `rna_struct!`).");
    let doc_coll_mut = format!("Mutable `{p}` collection (generated by `rna_struct!`).");
    let get_body = |owner: TokenStream2| {
        quote! {
            fn get(
                &self,
                prop: &'static ::frap_rna::PropDef,
            ) -> ::core::result::Result<::frap_rna::Field<'_>, ::frap_rna::RnaError> {
                #[allow(unused_variables)]
                let __frap_owner: &#ty = #owner;
                match prop.identifier {
                    #(#item_get)*
                    _ => ::core::result::Result::Err(::frap_rna::traits::no_attribute(&#item_path, prop.identifier)),
                }
            }
        }
    };
    let get_ref = get_body(quote!(self.owner));
    let get_mut = get_body(quote!(&*self.owner));
    let item_fn = |owner: TokenStream2| {
        quote! {
            fn item(&self, i: usize) -> ::core::result::Result<::frap_rna::Item<'_>, ::frap_rna::RnaError> {
                let n = ::frap_rna::RnaColl::len(self);
                if i >= n {
                    return ::core::result::Result::Err(::frap_rna::traits::index_out_of_range(i, n));
                }
                ::core::result::Result::Ok(::frap_rna::Item::Struct(::frap_rna::StructRef::Owned(
                    ::std::boxed::Box::new(#rf { owner: #owner, i }),
                )))
            }
        }
    };
    let item_ref = item_fn(quote!(self.owner));
    let item_mut_ref = item_fn(quote!(&*self.owner));

    arms.extra.push(quote! {
        #[doc = #doc_ref]
        #vis struct #rf<'a> {
            /// The owner of the columns.
            pub owner: &'a #ty,
            /// Item index.
            pub i: usize,
        }
        #[doc = #doc_mut]
        #vis struct #mt<'a> {
            /// The owner of the columns.
            pub owner: &'a mut #ty,
            /// Item index.
            pub i: usize,
        }
        #[doc = #doc_coll]
        #vis struct #coll<'a> {
            /// The owner of the columns.
            pub owner: &'a #ty,
        }
        #[doc = #doc_coll_mut]
        #vis struct #coll_mut<'a> {
            /// The owner of the columns.
            pub owner: &'a mut #ty,
        }
        impl<'a> ::frap_rna::Rna for #rf<'a> {
            fn srna(&self) -> &'static ::frap_rna::StructDef {
                &#item_path
            }
            #get_ref
        }
        impl<'a> ::frap_rna::Rna for #mt<'a> {
            fn srna(&self) -> &'static ::frap_rna::StructDef {
                &#item_path
            }
            #get_mut
        }
        impl<'a> ::frap_rna::RnaMut for #mt<'a> {
            fn set(
                &mut self,
                prop: &'static ::frap_rna::PropDef,
                v: ::frap_rna::Value,
            ) -> ::core::result::Result<::frap_rna::SetOutcome, ::frap_rna::RnaError> {
                let _ = &v;
                match prop.identifier {
                    #(#item_set)*
                    _ => ::core::result::Result::Err(::frap_rna::traits::no_attribute(&#item_path, prop.identifier)),
                }
            }
            fn struct_mut(
                &mut self,
                prop: &'static ::frap_rna::PropDef,
            ) -> ::core::result::Result<::frap_rna::RnaMutGuard<'_>, ::frap_rna::RnaError> {
                ::core::result::Result::Err(::frap_rna::traits::wrong_access(&#item_path, prop.identifier, "a struct"))
            }
            fn coll_mut(
                &mut self,
                prop: &'static ::frap_rna::PropDef,
            ) -> ::core::result::Result<::frap_rna::RnaCollMutGuard<'_>, ::frap_rna::RnaError> {
                ::core::result::Result::Err(::frap_rna::traits::wrong_access(&#item_path, prop.identifier, "a collection"))
            }
        }
        impl<'a> ::frap_rna::RnaColl for #coll<'a> {
            fn item_type(&self) -> &'static ::frap_rna::StructDef {
                &#item_path
            }
            #len_ref
            fn key_of(&self, _i: usize) -> ::core::option::Option<::frap_rna::traits::SmolStr> {
                ::core::option::Option::None
            }
            fn find(&self, _key: &str) -> ::core::option::Option<usize> {
                ::core::option::Option::None
            }
            #item_ref
            #fg_ref
        }
        impl<'a> ::frap_rna::RnaColl for #coll_mut<'a> {
            fn item_type(&self) -> &'static ::frap_rna::StructDef {
                &#item_path
            }
            #len_mut
            fn key_of(&self, _i: usize) -> ::core::option::Option<::frap_rna::traits::SmolStr> {
                ::core::option::Option::None
            }
            fn find(&self, _key: &str) -> ::core::option::Option<usize> {
                ::core::option::Option::None
            }
            #item_mut_ref
            #fg_mut
        }
        impl<'a> ::frap_rna::RnaCollMut for #coll_mut<'a> {
            fn item_mut(&mut self, i: usize) -> ::core::result::Result<::frap_rna::RnaMutGuard<'_>, ::frap_rna::RnaError> {
                let n = ::frap_rna::RnaColl::len(self);
                if i >= n {
                    return ::core::result::Result::Err(::frap_rna::traits::index_out_of_range(i, n));
                }
                ::core::result::Result::Ok(::std::boxed::Box::new(#mt { owner: &mut *self.owner, i }))
            }
            #fs
        }
    });
    Ok(())
}

fn generate(spec: &StructSpec) -> syn::Result<TokenStream2> {
    let mut arms = Arms::default();
    let mut n = Counter(0);
    struct_arms(spec, &mut arms, &mut n)?;
    let ty = &spec.ty;
    let path = &spec.path;
    let srna_fn = match &spec.srna {
        Some(c) => {
            let ps = closure_params(c, 1, "srna = |s| …")?;
            let p0 = &ps[0];
            let body = &c.body;
            quote!({ let #p0: &#ty = self; #body })
        }
        None => quote!(&#path),
    };
    let Arms {
        get,
        set,
        struct_mut,
        coll_mut,
        decls,
        check_fns,
        hook_decls,
        extra,
    } = arms;
    let k = decls.len();
    Ok(quote! {
        impl ::frap_rna::traits::RnaStruct for #ty {
            fn struct_def() -> &'static ::frap_rna::StructDef {
                &#path
            }
        }

        impl ::frap_rna::Rna for #ty {
            fn srna(&self) -> &'static ::frap_rna::StructDef {
                #srna_fn
            }
            fn get(
                &self,
                prop: &'static ::frap_rna::PropDef,
            ) -> ::core::result::Result<::frap_rna::Field<'_>, ::frap_rna::RnaError> {
                match prop.identifier {
                    #(#get)*
                    _ => ::core::result::Result::Err(::frap_rna::traits::no_attribute(
                        ::frap_rna::Rna::srna(self),
                        prop.identifier,
                    )),
                }
            }
        }

        impl ::frap_rna::RnaMut for #ty {
            fn set(
                &mut self,
                prop: &'static ::frap_rna::PropDef,
                v: ::frap_rna::Value,
            ) -> ::core::result::Result<::frap_rna::SetOutcome, ::frap_rna::RnaError> {
                let _ = &v;
                match prop.identifier {
                    #(#set)*
                    _ => ::core::result::Result::Err(::frap_rna::traits::no_attribute(
                        ::frap_rna::Rna::srna(self),
                        prop.identifier,
                    )),
                }
            }
            fn struct_mut(
                &mut self,
                prop: &'static ::frap_rna::PropDef,
            ) -> ::core::result::Result<::frap_rna::RnaMutGuard<'_>, ::frap_rna::RnaError> {
                match prop.identifier {
                    #(#struct_mut)*
                    _ => ::core::result::Result::Err(::frap_rna::traits::wrong_access(
                        ::frap_rna::Rna::srna(self),
                        prop.identifier,
                        "a struct",
                    )),
                }
            }
            fn coll_mut(
                &mut self,
                prop: &'static ::frap_rna::PropDef,
            ) -> ::core::result::Result<::frap_rna::RnaCollMutGuard<'_>, ::frap_rna::RnaError> {
                match prop.identifier {
                    #(#coll_mut)*
                    _ => ::core::result::Result::Err(::frap_rna::traits::wrong_access(
                        ::frap_rna::Rna::srna(self),
                        prop.identifier,
                        "a collection",
                    )),
                }
            }
        }

        impl ::frap_rna::testing::RnaBindings for #ty {
            fn bindings() -> &'static [::frap_rna::testing::BindingDecl] {
                #(#check_fns)*
                static __FRAP_BINDINGS: [::frap_rna::testing::BindingDecl; #k] = [#(#decls),*];
                &__FRAP_BINDINGS
            }
        }

        #(#hook_decls)*
        #(#extra)*
    })
}
