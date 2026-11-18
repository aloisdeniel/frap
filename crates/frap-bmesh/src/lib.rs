// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-bmesh` (layer L5): `BMesh` with `BLI_mempool`-identical LIFO pools, kernel Euler ops
//! with Blender names, CustomData, select flush/history, walkers, `from_mesh`/`to_mesh`, one
//! file per `bmesh.ops` operator (ARCHITECTURE §6.2).
//!
//! The public API is a CONTRACT frozen by **M2 W0** (WP 2.0, §3.4): the element types and
//! `BMesh` (`types.rs`), `Pool` (`pool.rs`), the kernel Euler operators (`kernel.rs`), queries
//! (`query.rs`), CustomData (`customdata.rs`), selection (`select.rs`), walkers (`walkers.rs`),
//! conversion (`convert.rs`) and validation (`validate.rs`) are implemented by WP 2.1; the
//! `ops/<bmesh_op>.rs` files by the WPs of M2 W2 (§22.4). Additive changes need no RFC
//! (rule 2.2.4).
//!
//! **Long loops call `frap_math::budget::check()`** (the `--stmt-timeout` cancellation point,
//! §12): a cancelled statement is rolled back by the session, so an op may unwind half-way.
#![deny(unsafe_code)]

pub mod convert;
pub mod customdata;
pub mod kernel;
pub mod pool;
pub mod query;
pub mod select;
pub mod types;
pub mod validate;
pub mod walkers;

/// One file per `bmesh.ops` operator (`ops/<bmesh_op>.rs`, §6.2, §21.1), included with
/// `automod`: a WP adds an op file without a `mod` edit. Each op is
/// `pub fn <op>(bm: &mut BMesh, i: &<Op>In) -> Result<<Op>Out, BmError>`.
pub mod ops {
    automod::dir!(pub "src/ops");
}

pub use convert::{FromMeshParams, ToMeshParams};
pub use customdata::{CdLayer, CustomData};
pub use pool::Pool;
pub use types::{
    BMEdge, BMElem, BMFace, BMLoop, BMVert, BMesh, BmError, CreateFlag, DiskLink, EdgeId, ElemFlag,
    ElemTypes, FaceId, LoopId, SelectMode, ShapeLayer, VertId,
};

/// The identity matrix in Blender's `float[4][4]` layout (`m[col][row]`), the default of
/// `bmesh.ops` matrix slots.
pub const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];
