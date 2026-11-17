// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-spec-rna` (layer L2): **generated** `StructDef` statics and per-property consts for
//! every dumped RNA struct (ARCHITECTURE §4.2). Data crates depend only on this one.
//!
//! Naming (RFC 0001 F-GEN-1): the struct static at the root (`frap_spec_rna::OBJECT`), its
//! property consts in a per-struct module (`frap_spec_rna::object::LOCATION`). Everything
//! under `src/generated/` is written by `cargo xtask codegen` (WP 1.1); until then it holds the
//! hand-written M0 stand-in.
#![deny(unsafe_code)]

#[rustfmt::skip]
#[allow(missing_docs, clippy::all)]
pub mod generated;

pub use generated::*;
