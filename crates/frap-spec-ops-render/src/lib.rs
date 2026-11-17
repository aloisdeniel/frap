// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-spec-ops-render` (layer L2): **generated** `OperatorSpec` statics and one typed props struct per
//! operator of the `render` ops group whose OPERATORS.md milestone is ≤ `spec/current_milestone`
//! (ARCHITECTURE §4.2, §10.4). One module per Blender namespace (`frap-spec-ops-render::<ns>::<Op>`).
//! Everything under `src/generated/` is written by `cargo xtask codegen` (WP 1.1).
#![deny(unsafe_code)]

#[rustfmt::skip]
#[allow(missing_docs, clippy::all)]
pub mod generated;

#[allow(unused_imports)] // empty until codegen fills the group
pub use generated::*;
