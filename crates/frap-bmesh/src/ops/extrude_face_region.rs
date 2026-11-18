// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.extrude_face_region` (Blender 5.2.4 `bmo_extrude_face_region`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.extrude_face_region(bmesh, geom=[], edges_exclude={}, use_keep_orig=False, use_normal_flip=False, use_normal_from_adjacent=False, use_dissolve_ortho_edges=False, use_select_history=False, skip_input_flip=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `extrude_face_region` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtrudeFaceRegionIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `edges_exclude` (a set of edges).
    pub edges_exclude: Vec<EdgeId>,
    /// `use_keep_orig`.
    pub use_keep_orig: bool,
    /// `use_normal_flip`.
    pub use_normal_flip: bool,
    /// `use_normal_from_adjacent`.
    pub use_normal_from_adjacent: bool,
    /// `use_dissolve_ortho_edges`.
    pub use_dissolve_ortho_edges: bool,
    /// `use_select_history`.
    pub use_select_history: bool,
    /// `skip_input_flip`.
    pub skip_input_flip: bool,
}

/// Output slots of `extrude_face_region` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ExtrudeFaceRegionOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `extrude_face_region` on `bm`.
pub fn extrude_face_region(bm: &mut BMesh, i: &ExtrudeFaceRegionIn) -> Result<ExtrudeFaceRegionOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.5: bmesh op extrude_face_region")
}
