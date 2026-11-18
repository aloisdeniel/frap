// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.collapse` (Blender 5.2.4 `bmo_collapse`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.collapse(bmesh, edges=[], uvs=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `collapse` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollapseIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `uvs`.
    pub uvs: bool,
}

/// Output slots of `collapse` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct CollapseOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `collapse` on `bm`.
pub fn collapse(bm: &mut BMesh, i: &CollapseIn) -> Result<CollapseOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op collapse")
}
