// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_edges` (Blender 5.2.4 `bmo_dissolve_edges`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_edges(bmesh, edges=[], use_verts=False, use_face_split=False, angle_threshold=0.0, use_preserve_quads=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `dissolve_edges` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveEdgesIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_verts`.
    pub use_verts: bool,
    /// `use_face_split`.
    pub use_face_split: bool,
    /// `angle_threshold`.
    pub angle_threshold: f32,
    /// `use_preserve_quads`.
    pub use_preserve_quads: bool,
}

/// Output slots of `dissolve_edges` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveEdgesOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `dissolve_edges` on `bm`.
pub fn dissolve_edges(bm: &mut BMesh, i: &DissolveEdgesIn) -> Result<DissolveEdgesOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op dissolve_edges")
}
