// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.extrude_edge_only` (Blender 5.2.4 `bmo_extrude_edge_only`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.extrude_edge_only(bmesh, edges=[], use_normal_flip=False, use_select_history=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `extrude_edge_only` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtrudeEdgeOnlyIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_normal_flip`.
    pub use_normal_flip: bool,
    /// `use_select_history`.
    pub use_select_history: bool,
}

/// Output slots of `extrude_edge_only` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ExtrudeEdgeOnlyOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `extrude_edge_only` on `bm`.
pub fn extrude_edge_only(bm: &mut BMesh, i: &ExtrudeEdgeOnlyIn) -> Result<ExtrudeEdgeOnlyOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.5: bmesh op extrude_edge_only")
}
