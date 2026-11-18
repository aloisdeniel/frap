// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.weld_verts` (Blender 5.2.4 `bmo_weld_verts`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.weld_verts(bmesh, targetmap={}, use_centroid=False, average_vert_data=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `weld_verts` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeldVertsIn {
    /// `targetmap` (vertex -> target vertex).
    pub targetmap: Vec<(VertId, VertId)>,
    /// `use_centroid`.
    pub use_centroid: bool,
    /// `average_vert_data`.
    pub average_vert_data: bool,
}

/// Output slots of `weld_verts` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct WeldVertsOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `weld_verts` on `bm`.
pub fn weld_verts(bm: &mut BMesh, i: &WeldVertsIn) -> Result<WeldVertsOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op weld_verts")
}
