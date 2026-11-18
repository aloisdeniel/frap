// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.recalc_face_normals` (Blender 5.2.4 `bmo_recalc_face_normals`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.9 (Normals, marks, attributes).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.recalc_face_normals(bmesh, faces=[])`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `recalc_face_normals` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecalcFaceNormalsIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
}

/// Output slots of `recalc_face_normals` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RecalcFaceNormalsOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `recalc_face_normals` on `bm`.
pub fn recalc_face_normals(bm: &mut BMesh, i: &RecalcFaceNormalsIn) -> Result<RecalcFaceNormalsOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.9: bmesh op recalc_face_normals")
}
