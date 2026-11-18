// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.grid_fill` (Blender 5.2.4 `bmo_grid_fill`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.grid_fill(bmesh, edges=[], mat_nr=0, use_smooth=False, use_interp_simple=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `grid_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridFillIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `mat_nr`.
    pub mat_nr: i32,
    /// `use_smooth`.
    pub use_smooth: bool,
    /// `use_interp_simple`.
    pub use_interp_simple: bool,
}

/// Output slots of `grid_fill` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct GridFillOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `grid_fill` on `bm`.
pub fn grid_fill(bm: &mut BMesh, i: &GridFillIn) -> Result<GridFillOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op grid_fill")
}
