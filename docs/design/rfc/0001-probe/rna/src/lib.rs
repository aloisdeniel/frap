// SPDX-License-Identifier: GPL-3.0-or-later
//! Probe of the `frap-rna` contracts.
//!
//! Every item is transcribed from ARCHITECTURE.md (section given per module). Where the
//! transcribed contract does not work, the module keeps the contract form where it still
//! compiles and adds the amendment next to it, tagged `RFC 0001 F-<id>` (see
//! `docs/design/rfc/0001-m0-contract-findings.md`).
//!
//! # Contract failures demonstrated as compile_fail doctests
//!
//! F-ED-1 — `gen` is a reserved keyword in edition 2024 (the workspace edition), so neither
//! `IdHeader.gen` (§5.1) nor a module for `src/gen/` (§2.2.2, §4.2) can be declared as written:
//!
//! ```compile_fail
//! pub struct IdHeader { pub gen: u64 }
//! ```
//!
//! F-VAL-4 — `Value` has no ID variant, so the §10.4 line `cx.result("object", ob)` (which needs
//! `IdRef<ObjectId>: Into<Value>`) cannot be written; frap-rna cannot name the generated
//! `OBJECT` StructDef either (L2 is above L1), so it cannot build a `Value::Ptr` itself:
//!
//! ```compile_fail,E0277
//! use frap_probe_rna::{IdRef, ObjectId, Value};
//! fn result(_k: &str, _v: impl Into<Value>) {}
//! fn f(ob: IdRef<ObjectId>) { result("object", ob) }
//! ```
//!
//! F-TR-4 — an `RnaColl` item can only be a `StructRef`; there is no way to return an ID
//! (re-root) or `None` (`mesh.materials` may contain `None`, verified):
//!
//! ```compile_fail,E0308
//! use frap_probe_rna::{IdAny, StructRef};
//! fn item<'a>(id: Option<IdAny>) -> StructRef<'a> { id }
//! ```
#![forbid(unsafe_code)]

pub mod def;
pub mod error;
/// `gen` is a reserved keyword in edition 2024: `src/gen/` (§2.2.2, §4.2) cannot be a module
/// named `gen` (RFC 0001 F-ED-1). Generated code lives in `src/generated/`.
#[rustfmt::skip]
pub mod generated;
pub mod id;
pub mod op;
pub mod ptr;
pub mod traits;
pub mod value;

pub use def::*;
pub use error::*;
pub use id::*;
pub use op::*;
pub use ptr::*;
pub use traits::*;
pub use value::*;
