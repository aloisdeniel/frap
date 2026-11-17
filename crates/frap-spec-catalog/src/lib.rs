// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-spec-catalog` (layer L2): every dumped operator that has no typed props struct in the
//! current milestone, as one deflate-compressed JSON blob (`src/generated/catalog.json.deflate`,
//! written by `cargo xtask codegen`), decoded on first use into leaked `&'static OperatorSpec`
//! tables in a `OnceLock`: pointer identity holds for the process. Serves `KnownNotImplemented`
//! and `--describe` (ARCHITECTURE §4.2, §10.1). Owned by WP 1.1 except this file.
#![deny(unsafe_code)]

mod decode;
/// Generated blob(s) and their metadata (`cargo xtask codegen`, WP 1.1).
#[rustfmt::skip]
#[allow(missing_docs, clippy::all)]
pub mod generated;

pub use decode::catalog;
