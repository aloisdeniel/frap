// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-macros` (layer L0): frap's proc-macros, one file each (ARCHITECTURE §3.2).
//!
//! RNA tier (WP 1.2, needed in M1 W1/W2): `rna_struct!`, `#[rna_func]`, `#[rna_hook]`.
//! Operator tier (WP 1.12): `#[operator]`, `#[modifier]`, `#[bmesh_op]`.
//!
//! Every macro that emits a `linkme` slice entry also emits `#[allow(unsafe_code)]` on it
//! (RFC 0001 F-LINK-1), so hand-written code never needs the allow.
//!
//! Until the owning WP lands, each macro expands to a `compile_error!` naming it.

use proc_macro::TokenStream;

mod bmesh_op;
mod modifier;
mod operator;
mod rna_func;
mod rna_hook;
mod rna_struct;

/// `rna_struct! { Object as frap_spec_rna::OBJECT { location => location (tag = Vector), … } }`
/// (§9.6): generates `impl Rna`/`impl RnaMut`, SoA proxy types and the `HOOK_DECLS` entries.
#[proc_macro]
pub fn rna_struct(input: TokenStream) -> TokenStream {
    rna_struct::expand(input)
}

/// `#[rna_func(srna = "Object", func = "select_set")]` on an `RnaFuncImpl` (§9.5).
#[proc_macro_attribute]
pub fn rna_func(attr: TokenStream, item: TokenStream) -> TokenStream {
    rna_func::expand(attr, item)
}

/// `#[rna_hook(srna = "Object", prop = "rotation_mode")]` on a get and/or set hook (§9.5).
#[proc_macro_attribute]
pub fn rna_hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    rna_hook::expand(attr, item)
}

/// `#[operator(poll = poll::always, tier = T1)]` on `fn exec(cx: &mut OpCtx, p: P) -> OpResult`
/// (§10.4): emits the `OperatorDef` into `OPERATORS` and a spec-exists test.
#[proc_macro_attribute]
pub fn operator(attr: TokenStream, item: TokenStream) -> TokenStream {
    operator::expand(attr, item)
}

/// `#[modifier(MIRROR, kind = Constructive, flags = …)]` (§7.2, M4).
#[proc_macro_attribute]
pub fn modifier(attr: TokenStream, item: TokenStream) -> TokenStream {
    modifier::expand(attr, item)
}

/// `#[bmesh_op(...)]` on a `bmesh.ops` port (§6.2, M2).
#[proc_macro_attribute]
pub fn bmesh_op(attr: TokenStream, item: TokenStream) -> TokenStream {
    bmesh_op::expand(attr, item)
}

/// Shared stub expansion: a `compile_error!` naming the macro and its owner.
fn not_yet(name: &str, owner: &str) -> TokenStream {
    format!("::core::compile_error!(\"frap-macros: `{name}` is not implemented yet ({owner})\");")
        .parse()
        .expect("invariant: valid token stream")
}
