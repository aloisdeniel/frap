// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.delete` (Blender 5.2.4 `bmo_delete`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.1 (BMesh core).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.delete(bmesh, geom=[], context='VERTS')`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `delete` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `context` (enum identifier).
    pub context: &'static str,
}

impl Default for DeleteIn {
    fn default() -> Self {
        DeleteIn {
            geom: Vec::new(),
            context: "VERTS",
        }
    }
}

/// Output slots of `delete` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DeleteOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `delete` on `bm`.
pub fn delete(bm: &mut BMesh, i: &DeleteIn) -> Result<DeleteOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.1: bmesh op delete")
}
