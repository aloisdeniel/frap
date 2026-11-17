// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-object` (layer L10): `object.*`, `view3d.snap_*`, `view3d.camera_to_view_selected`, `collection.*`, `scene.*` operators and the object-mode `MODE_HANDLERS` (ARCHITECTURE §3.2, §8). Primary namespace: `object`.
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

/// `MODE_HANDLERS` entries, one file per mode (`modes/object.rs` M1, `edit_mesh.rs` M2,
/// `edit_armature.rs` and `pose.rs` M6). `object.mode_set` dispatches to them.
mod modes {
    automod::dir!("src/modes");
}
/// Shared helpers of the object operators (origin, apply, parenting maths; WP 1.21).
pub mod support;

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
