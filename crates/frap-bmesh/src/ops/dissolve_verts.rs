// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_verts` (Blender 5.2.4 `bmo_dissolve_verts`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_verts(bmesh, verts=[], use_face_split=False, use_boundary_tear=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `dissolve_verts` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveVertsIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `use_face_split`.
    pub use_face_split: bool,
    /// `use_boundary_tear`.
    pub use_boundary_tear: bool,
}

/// Output slots of `dissolve_verts` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveVertsOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `dissolve_verts` on `bm`.
pub fn dissolve_verts(bm: &mut BMesh, i: &DissolveVertsIn) -> Result<DissolveVertsOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op dissolve_verts")
}
