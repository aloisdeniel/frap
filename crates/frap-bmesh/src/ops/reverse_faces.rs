// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.reverse_faces` (Blender 5.2.4 `bmo_reverse_faces`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.9 (Normals, marks, attributes).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.reverse_faces(bmesh, faces=[], flip_multires=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `reverse_faces` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReverseFacesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `flip_multires`.
    pub flip_multires: bool,
}

/// Output slots of `reverse_faces` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ReverseFacesOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `reverse_faces` on `bm`.
pub fn reverse_faces(bm: &mut BMesh, i: &ReverseFacesIn) -> Result<ReverseFacesOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.9: bmesh op reverse_faces")
}
