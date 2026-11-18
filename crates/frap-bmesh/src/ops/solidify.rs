// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.solidify` (Blender 5.2.4 `bmo_solidify`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.solidify(bmesh, geom=[], thickness=0.0)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `solidify` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidifyIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `thickness`.
    pub thickness: f32,
}

/// Output slots of `solidify` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SolidifyOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `solidify` on `bm`.
pub fn solidify(bm: &mut BMesh, i: &SolidifyIn) -> Result<SolidifyOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op solidify")
}
