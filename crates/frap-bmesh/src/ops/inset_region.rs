// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.inset_region` (Blender 5.2.4 `bmo_inset_region`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.inset_region(bmesh, faces=[], faces_exclude=[], use_boundary=False, use_even_offset=False, use_interpolate=False, use_relative_offset=False, use_edge_rail=False, thickness=0.0, depth=0.0, use_outset=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `inset_region` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InsetRegionIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `faces_exclude`.
    pub faces_exclude: Vec<FaceId>,
    /// `use_boundary`.
    pub use_boundary: bool,
    /// `use_even_offset`.
    pub use_even_offset: bool,
    /// `use_interpolate`.
    pub use_interpolate: bool,
    /// `use_relative_offset`.
    pub use_relative_offset: bool,
    /// `use_edge_rail`.
    pub use_edge_rail: bool,
    /// `thickness`.
    pub thickness: f32,
    /// `depth`.
    pub depth: f32,
    /// `use_outset`.
    pub use_outset: bool,
}

/// Output slots of `inset_region` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct InsetRegionOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `inset_region` on `bm`.
pub fn inset_region(bm: &mut BMesh, i: &InsetRegionIn) -> Result<InsetRegionOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.5: bmesh op inset_region")
}
