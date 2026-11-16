// SPDX-License-Identifier: GPL-3.0-or-later
//! Binding checks (ARCHITECTURE §4.3). WP 1.2 implements `check_bindings`, which every crate
//! with hand-written `Rna` bindings runs in its tests, and makes `rna_struct!` emit one
//! `HookDecl` per `=> hook` / `hook_coll` binding into `HOOK_DECLS`.
//!
//! The completeness test (every `HOOK_DECLS` entry has exactly one `RNA_HOOKS` entry and vice
//! versa) lives in `crates/frap-ops-all/tests/registry.rs`, where everything is linked.

/// One `=> hook` / `hook_coll` binding declared by `rna_struct!`.
#[derive(Debug)]
pub struct HookDecl {
    /// Struct identifier ("Object").
    pub srna: &'static str,
    /// Property identifier ("material_slots").
    pub prop: &'static str,
    /// True for `hook_coll` (the get hook returns a collection proxy).
    pub coll: bool,
}

/// Every hook binding of every linked crate.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static HOOK_DECLS: [HookDecl];
