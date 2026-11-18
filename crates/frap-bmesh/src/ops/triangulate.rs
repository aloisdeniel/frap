// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.triangulate` (Blender 5.2.4 `bmo_triangulate`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.triangulate(bmesh, faces=[], quad_method='BEAUTY', ngon_method='BEAUTY')`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `triangulate` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct TriangulateIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `quad_method` (enum identifier).
    pub quad_method: &'static str,
    /// `ngon_method` (enum identifier).
    pub ngon_method: &'static str,
}

impl Default for TriangulateIn {
    fn default() -> Self {
        TriangulateIn {
            faces: Vec::new(),
            quad_method: "BEAUTY",
            ngon_method: "BEAUTY",
        }
    }
}

/// Output slots of `triangulate` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TriangulateOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `triangulate` on `bm`.
pub fn triangulate(bm: &mut BMesh, i: &TriangulateIn) -> Result<TriangulateOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op triangulate")
}
