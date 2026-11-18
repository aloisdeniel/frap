// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_faces` (Blender 5.2.4 `bmo_dissolve_faces`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_faces(bmesh, faces=[], use_verts=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `dissolve_faces` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveFacesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `use_verts`.
    pub use_verts: bool,
}

/// Output slots of `dissolve_faces` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveFacesOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `dissolve_faces` on `bm`.
pub fn dissolve_faces(bm: &mut BMesh, i: &DissolveFacesIn) -> Result<DissolveFacesOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op dissolve_faces")
}
