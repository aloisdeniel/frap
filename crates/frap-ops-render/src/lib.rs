// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-render` (layer L10): `render.render` with Blender's output path rules and `frap.render_contact_sheet`; reaches the GPU only through `OpCtx::services().render()` (ARCHITECTURE §19.4, §19.6). No primary namespace.
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

/// Output path rules (`//`, `#` padding, extensions) and image writing (WP 1.24).
pub mod output;

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
