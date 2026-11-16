// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-io` (layer L10): `import_scene.gltf`, `export_scene.gltf` and the `wm.*` file operators (ARCHITECTURE §15, §17.2). No primary namespace.
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

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
