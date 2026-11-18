// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.pointmerge` (Blender 5.2.4 `bmo_pointmerge`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.pointmerge(bmesh, verts=[], merge_co=Vector(), vert_target=None)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `pointmerge` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointmergeIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `merge_co`.
    pub merge_co: [f32; 3],
    /// `vert_target`.
    pub vert_target: Option<VertId>,
}

/// Output slots of `pointmerge` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct PointmergeOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `pointmerge` on `bm`.
pub fn pointmerge(bm: &mut BMesh, i: &PointmergeIn) -> Result<PointmergeOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op pointmerge")
}
