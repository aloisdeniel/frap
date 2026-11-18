// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-uv` (layer L10): `uv.*` operators (ARCHITECTURE §3.2). Primary namespace: `uv`.
//!
//! One file per operator in `src/ops/` (`ops/<op>.rs`, §21.1), included with `automod` and
//! registered in `frap_op::OPERATORS`: adding an operator needs no `mod` edit. The UV solvers
//! live in `frap-uv` (WP 3.3); WP 3.8 owns this crate after M3 W0. Only pure unit tests live
//! here; behaviour tests go to `frap-golden` cases (`tests/golden/cases/uv/**`).
#![deny(unsafe_code)]

#[allow(non_snake_case)]
mod ops {
    automod::dir!("src/ops");
}

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
