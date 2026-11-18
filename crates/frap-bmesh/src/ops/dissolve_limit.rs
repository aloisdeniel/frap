// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_limit` (Blender 5.2.4 `bmo_dissolve_limit`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_limit(bmesh, angle_limit=0.0, use_dissolve_boundaries=False, verts=[], edges=[], delimit={})`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, EdgeId, VertId};

/// Input slots of `dissolve_limit` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveLimitIn {
    /// `angle_limit`.
    pub angle_limit: f32,
    /// `use_dissolve_boundaries`.
    pub use_dissolve_boundaries: bool,
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `delimit` (enum-flag identifiers).
    pub delimit: Vec<&'static str>,
}

/// Output slots of `dissolve_limit` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveLimitOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `dissolve_limit` on `bm`.
pub fn dissolve_limit(bm: &mut BMesh, i: &DissolveLimitIn) -> Result<DissolveLimitOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.7: bmesh op dissolve_limit")
}
