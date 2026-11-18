// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.offset_edgeloops` (Blender 5.2.4 `bmo_offset_edgeloops`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.6 (Subdivide and loops).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.offset_edgeloops(bmesh, edges=[], use_cap_endpoint=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `offset_edgeloops` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OffsetEdgeloopsIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_cap_endpoint`.
    pub use_cap_endpoint: bool,
}

/// Output slots of `offset_edgeloops` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct OffsetEdgeloopsOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `offset_edgeloops` on `bm`.
pub fn offset_edgeloops(bm: &mut BMesh, i: &OffsetEdgeloopsIn) -> Result<OffsetEdgeloopsOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.6: bmesh op offset_edgeloops")
}
