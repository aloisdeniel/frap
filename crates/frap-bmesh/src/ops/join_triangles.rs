// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.join_triangles` (Blender 5.2.4 `bmo_join_triangles`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.join_triangles(bmesh, faces=[], cmp_seam=False, cmp_sharp=False, cmp_uvs=False, cmp_vcols=False, cmp_materials=False, angle_face_threshold=0.0, angle_shape_threshold=0.0, topology_influence=0.0, deselect_joined=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `join_triangles` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JoinTrianglesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `cmp_seam`.
    pub cmp_seam: bool,
    /// `cmp_sharp`.
    pub cmp_sharp: bool,
    /// `cmp_uvs`.
    pub cmp_uvs: bool,
    /// `cmp_vcols`.
    pub cmp_vcols: bool,
    /// `cmp_materials`.
    pub cmp_materials: bool,
    /// `angle_face_threshold`.
    pub angle_face_threshold: f32,
    /// `angle_shape_threshold`.
    pub angle_shape_threshold: f32,
    /// `topology_influence`.
    pub topology_influence: f32,
    /// `deselect_joined`.
    pub deselect_joined: bool,
}

/// Output slots of `join_triangles` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct JoinTrianglesOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `join_triangles` on `bm`.
pub fn join_triangles(bm: &mut BMesh, i: &JoinTrianglesIn) -> Result<JoinTrianglesOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op join_triangles")
}
