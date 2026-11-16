// SPDX-License-Identifier: GPL-3.0-or-later
//! Binding checks (ARCHITECTURE §4.3). Every crate with hand-written `Rna` bindings runs
//! [`check_bindings`] in its tests:
//!
//! ```ignore
//! #[cfg(test)] #[test] fn bindings() { frap_rna::testing::check_bindings::<Object>(); }
//! ```
//!
//! `rna_struct!` records what it bound ([`RnaBindings`]); `check_bindings` asserts that every
//! bound property exists in the `StructDef` (walking the base chain) with the same type, array
//! shape and enum items, and that no binding names an unknown property. The set of implemented
//! properties is derived from the bindings themselves.
//!
//! `rna_struct!` also emits one [`HookDecl`] per `=> hook` / `hook_coll` binding into
//! [`HOOK_DECLS`]. The completeness test (every `HOOK_DECLS` entry has exactly one `RNA_HOOKS`
//! entry and vice versa) lives in `crates/frap-ops-all/tests/registry.rs`, where everything is
//! linked.

use std::collections::BTreeSet;

use crate::def::{PropDef, PropFlags, PropKind, StructDef};
use crate::traits::{RnaStruct, ToRnaValue};

/// One `=> hook` / `hook_coll` binding declared by `rna_struct!`.
#[derive(Debug)]
pub struct HookDecl {
    /// Struct identifier ("Object"; or the struct named by `hook(on = "ID")`).
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

/// How a property is bound by `rna_struct!`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BindingKind {
    /// A value read and written through a field or `get`/`set` expressions.
    Value,
    /// A value with a getter only (`readonly(…)`, or `get(…)` without `set(…)`).
    ReadOnly,
    /// `=> hook`: get and set go through `RNA_HOOKS`.
    Hook,
    /// `=> hook_coll`: a collection served by a get hook.
    HookColl,
    /// `=> python_only hook`: a Python-only member (`PYTHON_ONLY` in the `StructDef`).
    PythonOnlyHook,
    /// A value read from the field and written through a set hook (`(set = hook)`).
    SetHook,
    /// `id(field)`: an ID pointer.
    Id,
    /// `struct(field)`: a nested in-memory struct.
    Struct,
    /// `embedded(field, Slot)`: an embedded ID.
    Embedded,
    /// `coll(field)`: a `Vec` of structs.
    Coll,
    /// `id_coll(field, …)`: a `Vec` of ID pointers.
    IdColl,
    /// `soa(Item as …)`: a struct-of-arrays collection with generated proxy items.
    Soa,
}

/// Item bindings of a `soa(…)` collection: item struct and its bindings.
pub type SoaItems = fn() -> (&'static StructDef, &'static [BindingDecl]);

/// Type check of a bound Rust type against a `PropDef`.
pub type BindingCheck = fn(&PropDef) -> Result<(), String>;

/// One binding recorded by `rna_struct!`.
#[derive(Debug)]
pub struct BindingDecl {
    /// Property identifier.
    pub prop: &'static str,
    /// Binding kind.
    pub kind: BindingKind,
    /// Type check of the bound Rust type against the `PropDef` (value bindings).
    pub check: Option<BindingCheck>,
    /// Item struct of `coll(…, item = …)` / `id_coll` / `soa` bindings.
    pub item: Option<fn() -> &'static StructDef>,
    /// Item bindings of `soa(…)` bindings.
    pub soa: Option<SoaItems>,
}

/// Implemented by `rna_struct!`: the bindings of one type.
pub trait RnaBindings {
    /// Every binding, in declaration order.
    fn bindings() -> &'static [BindingDecl];
}

/// Asserts that the bindings of `T` match its `StructDef` (§4.3). Panics with every problem
/// found.
pub fn check_bindings<T: RnaBindings + RnaStruct>() {
    let problems = binding_problems::<T>();
    assert!(
        problems.is_empty(),
        "rna_struct! bindings of {} do not match the RNA definition:\n  {}",
        T::struct_def().identifier,
        problems.join("\n  ")
    );
}

/// The problems [`check_bindings`] reports (empty when the bindings are correct).
pub fn binding_problems<T: RnaBindings + RnaStruct>() -> Vec<String> {
    let mut out = Vec::new();
    check_decls(T::struct_def(), T::bindings(), &mut out);
    out
}

fn check_decls(srna: &'static StructDef, decls: &[BindingDecl], out: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for d in decls {
        let at = format!("{}.{}", srna.identifier, d.prop);
        if !seen.insert(d.prop) {
            out.push(format!("{at}: bound twice"));
            continue;
        }
        let Some(prop) = srna.find_prop(d.prop) else {
            out.push(format!("{at}: no such property in the RNA definition"));
            continue;
        };
        if let Err(e) = check_one(prop, d) {
            out.push(format!("{at}: {e}"));
        }
        if let Some(soa) = d.soa {
            let (item, item_decls) = soa();
            check_decls(item, item_decls, out);
        }
    }
}

fn check_one(prop: &'static PropDef, d: &BindingDecl) -> Result<(), String> {
    let is_value = !matches!(
        prop.kind,
        PropKind::Pointer { .. } | PropKind::Collection { .. }
    );
    match d.kind {
        BindingKind::Value | BindingKind::SetHook => {
            if !is_value {
                return Err(format!("{} property bound as a value", prop.type_str()));
            }
            if d.kind == BindingKind::Value && prop.is_readonly() {
                return Err("read-only property bound with a setter".into());
            }
        }
        BindingKind::ReadOnly => {
            if !is_value {
                return Err(format!("{} property bound as a value", prop.type_str()));
            }
        }
        BindingKind::Hook => {}
        BindingKind::HookColl => {
            if !matches!(prop.kind, PropKind::Collection { .. }) {
                return Err(format!("{} property bound as hook_coll", prop.type_str()));
            }
        }
        BindingKind::PythonOnlyHook => {
            if !prop.flags.contains(PropFlags::PYTHON_ONLY) {
                return Err("bound as python_only but the property is not PYTHON_ONLY".into());
            }
        }
        BindingKind::Id => {
            if !matches!(prop.kind, PropKind::Pointer { is_id: true, .. }) {
                return Err(format!("{} property bound as id(…)", kind_desc(prop)));
            }
        }
        BindingKind::Struct => {
            if !matches!(prop.kind, PropKind::Pointer { is_id: false, .. }) {
                return Err(format!("{} property bound as struct(…)", kind_desc(prop)));
            }
        }
        BindingKind::Embedded => {
            if !matches!(prop.kind, PropKind::Pointer { is_id: true, .. }) {
                return Err(format!("{} property bound as embedded(…)", kind_desc(prop)));
            }
        }
        BindingKind::Coll | BindingKind::IdColl | BindingKind::Soa => {
            let PropKind::Collection { item_type, .. } = prop.kind else {
                return Err(format!(
                    "{} property bound as a collection",
                    prop.type_str()
                ));
            };
            let bound = d.item.map(|f| f()).or_else(|| d.soa.map(|f| f().0));
            if let Some(item) = bound
                && item.identifier != item_type
                && !item.is_a(item_type)
            {
                return Err(format!(
                    "collection of {item_type} bound with items of {}",
                    item.identifier
                ));
            }
        }
    }
    if let Some(check) = d.check {
        check(prop)?;
    }
    Ok(())
}

fn kind_desc(prop: &PropDef) -> String {
    match prop.kind {
        PropKind::Pointer { is_id: true, .. } => "ID pointer".into(),
        PropKind::Pointer { is_id: false, .. } => "struct pointer".into(),
        _ => prop.type_str().into(),
    }
}

/// Support code for `rna_struct!` expansions. Not a stable API.
#[doc(hidden)]
pub mod __macro {
    use super::*;

    /// Type check of a field binding: `T` is inferred from `|s: &Owner| &s.field`.
    pub fn check_field<O, T, F>(_: F) -> BindingCheck
    where
        T: ToRnaValue,
        F: for<'r> Fn(&'r O) -> &'r T,
    {
        T::check_prop
    }

    /// Type check of a computed binding: `T` is inferred from `|s: &Owner| expr` (owned).
    pub fn check_value<O, T, F>(_: F) -> BindingCheck
    where
        T: ToRnaValue,
        F: Fn(&O) -> T,
    {
        T::check_prop
    }

    /// Type check of a SoA item getter: `T` is inferred from `|o: &Owner, i: usize| expr`.
    pub fn check_item<O, T, F>(_: F) -> BindingCheck
    where
        T: ToRnaValue,
        F: Fn(&O, usize) -> T,
    {
        T::check_prop
    }

    /// Item check of a `coll(field)` binding without `item = …`: `T` from `&s.field: &Vec<T>`.
    pub fn check_coll<O, T, F>(_: F) -> BindingCheck
    where
        T: RnaStruct,
        F: for<'r> Fn(&'r O) -> &'r Vec<T>,
    {
        check_item_type::<T>
    }

    fn check_item_type<T: RnaStruct>(prop: &PropDef) -> Result<(), String> {
        let PropKind::Collection { item_type, .. } = prop.kind else {
            return Ok(());
        };
        let it = T::struct_def();
        if it.identifier == item_type || it.is_a(item_type) {
            Ok(())
        } else {
            Err(format!(
                "collection of {item_type} bound with items of {}",
                it.identifier
            ))
        }
    }

    /// What a setter body may return: `()` or `Result<(), RnaError>`.
    pub trait SetResult {
        /// Normalizes to a `Result`.
        fn into_result(self) -> Result<(), crate::error::RnaError>;
    }
    impl SetResult for () {
        fn into_result(self) -> Result<(), crate::error::RnaError> {
            Ok(())
        }
    }
    impl SetResult for Result<(), crate::error::RnaError> {
        fn into_result(self) -> Result<(), crate::error::RnaError> {
            self
        }
    }

    /// Type check of an `id(field)` binding.
    pub fn check_id<O, T, F>(_: F) -> BindingCheck
    where
        T: crate::traits::IdField,
        F: for<'r> Fn(&'r O) -> &'r T,
    {
        T::check_prop
    }
}
