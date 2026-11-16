// SPDX-License-Identifier: GPL-3.0-or-later
//! Binding declarations of the hand-written mesh bindings (ARCHITECTURE §4.3).
//!
//! `rna_struct!` cannot express named, refined or nested struct-of-arrays collections
//! (`attributes["x"].data[i]` is a `FloatAttributeValue` of a `FloatAttribute`), so the mesh
//! bindings are written by hand and declare what they bind with these macros. The tables feed
//! `frap_rna::testing::check_bindings` exactly like `rna_struct!`'s: every bound property must
//! exist in the dumped `StructDef` with the same type, array shape and access, and the Rust
//! type each getter converts from is checked against the `PropDef`.

/// A writable value binding read from a Rust value of type `$ty`.
macro_rules! value {
    ($p:literal, $ty:ty) => {
        ::frap_rna::testing::BindingDecl {
            prop: $p,
            kind: ::frap_rna::testing::BindingKind::Value,
            check: Some(<$ty as ::frap_rna::traits::ToRnaValue>::check_prop),
            item: None,
            soa: None,
        }
    };
}

/// A read-only value binding read from a Rust value of type `$ty`.
macro_rules! readonly {
    ($p:literal, $ty:ty) => {
        ::frap_rna::testing::BindingDecl {
            prop: $p,
            kind: ::frap_rna::testing::BindingKind::ReadOnly,
            check: Some(<$ty as ::frap_rna::traits::ToRnaValue>::check_prop),
            item: None,
            soa: None,
        }
    };
}

/// A binding of another kind (`Hook`, `SetHook`, `Id`, `IdColl`) without a type check.
macro_rules! other {
    ($p:literal, $kind:ident) => {
        ::frap_rna::testing::BindingDecl {
            prop: $p,
            kind: ::frap_rna::testing::BindingKind::$kind,
            check: None,
            item: None,
            soa: None,
        }
    };
    ($p:literal, $kind:ident, item = $item:path) => {
        ::frap_rna::testing::BindingDecl {
            prop: $p,
            kind: ::frap_rna::testing::BindingKind::$kind,
            check: None,
            item: Some({
                fn item() -> &'static ::frap_rna::StructDef {
                    &$item
                }
                item
            }),
            soa: None,
        }
    };
}

/// A struct-of-arrays collection binding whose items are `$item` bound by `$decls`.
macro_rules! soa {
    ($p:literal, $item:path, $decls:expr) => {
        ::frap_rna::testing::BindingDecl {
            prop: $p,
            kind: ::frap_rna::testing::BindingKind::Soa,
            check: None,
            item: Some({
                fn item() -> &'static ::frap_rna::StructDef {
                    &$item
                }
                item
            }),
            soa: Some({
                fn soa() -> (
                    &'static ::frap_rna::StructDef,
                    &'static [::frap_rna::testing::BindingDecl],
                ) {
                    static DECLS: &[::frap_rna::testing::BindingDecl] = $decls;
                    (&$item, DECLS)
                }
                soa
            }),
        }
    };
}

pub(crate) use {other, readonly, soa, value};
