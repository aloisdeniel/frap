// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-transform` (layer L10): `transform.*` for every mode: the shared orientation/pivot/proportional core (`core/`) and one directory of `TRANSFORM_MODE_HANDLERS` per mode (`object/` M1, `edit/` M2, `pose/` M6) (ARCHITECTURE §10.3). Primary namespace: `transform`.
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

/// Shared orientation / pivot / proportional-editing core (WP 1.21).
pub mod core;
/// Object-mode `TRANSFORM_MODE_HANDLERS` entry (WP 1.21).
pub mod object;

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
