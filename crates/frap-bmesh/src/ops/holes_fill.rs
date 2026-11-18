// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.holes_fill` (Blender 5.2.4 `bmo_holes_fill`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.holes_fill(bmesh, edges=[], sides=0)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId};

/// Input slots of `holes_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HolesFillIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `sides`.
    pub sides: i32,
}

/// Output slots of `holes_fill` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct HolesFillOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `holes_fill` on `bm`.
pub fn holes_fill(bm: &mut BMesh, i: &HolesFillIn) -> Result<HolesFillOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op holes_fill")
}
