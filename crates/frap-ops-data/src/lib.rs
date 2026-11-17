// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-data` (layer L10): `material.*`, `image.*`, `world.*` operators and the data-API RNA functions (`bpy.data.*.new/remove`, slots, nodes, links) (ARCHITECTURE §3.2). No primary namespace.
//!
//! One file per operator in `src/ops/` (`ops/<op>.rs` for the primary namespace,
//! `ops/<namespace>__<op>.rs` otherwise, §21.1), included with `automod` and registered in
//! `frap_op::OPERATORS`: adding an operator needs no `mod` edit. Only pure unit tests live
//! here; behaviour tests go to `frap-golden` cases or `crates/frap-ops-<x>-tests` (rule 2.2.11).
#![deny(unsafe_code)]

#[allow(non_snake_case)]
mod ops {
    automod::dir!("src/ops");
}

/// RNA function implementations, one file each (`funcs/<struct>_<func>.rs`), registered in
/// `frap_scene::RNA_FUNCS`.
#[allow(non_snake_case)]
mod funcs {
    automod::dir!("src/funcs");
}

/// RNA get/set hooks that need the evaluator (`Object.dimensions`, `Object.bound_box`), one
/// file each (`hooks/<struct>_<prop>.rs`), registered in `frap_scene::RNA_HOOKS`.
mod hooks {
    automod::dir!("src/hooks");
}

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
