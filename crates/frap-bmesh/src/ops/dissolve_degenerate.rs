// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_degenerate` (Blender 5.2.4 `bmo_dissolve_degenerate`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_degenerate(bmesh, dist=0.0, edges=[])`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `dissolve_degenerate` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveDegenerateIn {
    /// `dist`.
    pub dist: f32,
    /// `edges`.
    pub edges: Vec<EdgeId>,
}

/// Output slots of `dissolve_degenerate` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveDegenerateOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `dissolve_degenerate` on `bm`.
pub fn dissolve_degenerate(bm: &mut BMesh, i: &DissolveDegenerateIn) -> Result<DissolveDegenerateOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op dissolve_degenerate")
}
