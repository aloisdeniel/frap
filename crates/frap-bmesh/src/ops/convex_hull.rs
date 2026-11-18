// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.convex_hull` (Blender 5.2.4 `bmo_convex_hull`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.convex_hull(bmesh, input=[], use_existing_faces=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `convex_hull` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConvexHullIn {
    /// `input`.
    pub input: Vec<BMElem>,
    /// `use_existing_faces`.
    pub use_existing_faces: bool,
}

/// Output slots of `convex_hull` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConvexHullOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `convex_hull` on `bm`.
pub fn convex_hull(bm: &mut BMesh, i: &ConvexHullIn) -> Result<ConvexHullOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op convex_hull")
}
