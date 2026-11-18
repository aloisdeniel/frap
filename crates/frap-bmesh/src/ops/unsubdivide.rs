// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.unsubdivide` (Blender 5.2.4 `bmo_unsubdivide`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.6 (Subdivide and loops).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.unsubdivide(bmesh, verts=[], iterations=0)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `unsubdivide` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnsubdivideIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `iterations`.
    pub iterations: i32,
}

/// Output slots of `unsubdivide` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct UnsubdivideOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `unsubdivide` on `bm`.
pub fn unsubdivide(bm: &mut BMesh, i: &UnsubdivideIn) -> Result<UnsubdivideOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.6: bmesh op unsubdivide")
}
