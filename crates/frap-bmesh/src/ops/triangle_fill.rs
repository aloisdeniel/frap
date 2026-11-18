// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.triangle_fill` (Blender 5.2.4 `bmo_triangle_fill`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.triangle_fill(bmesh, use_beauty=False, use_dissolve=False, edges=[], normal=Vector())`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `triangle_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TriangleFillIn {
    /// `use_beauty`.
    pub use_beauty: bool,
    /// `use_dissolve`.
    pub use_dissolve: bool,
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `normal`.
    pub normal: [f32; 3],
}

/// Output slots of `triangle_fill` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TriangleFillOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `triangle_fill` on `bm`.
pub fn triangle_fill(bm: &mut BMesh, i: &TriangleFillIn) -> Result<TriangleFillOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op triangle_fill")
}
