// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.rotate_edges` (Blender 5.2.4 `bmo_rotate_edges`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.rotate_edges(bmesh, edges=[], use_ccw=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `rotate_edges` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RotateEdgesIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_ccw`.
    pub use_ccw: bool,
}

/// Output slots of `rotate_edges` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RotateEdgesOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `rotate_edges` on `bm`.
pub fn rotate_edges(bm: &mut BMesh, i: &RotateEdgesIn) -> Result<RotateEdgesOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op rotate_edges")
}
