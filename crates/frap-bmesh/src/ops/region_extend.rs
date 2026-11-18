// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.region_extend` (Blender 5.2.4 `bmo_region_extend`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.4 (Selection).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.region_extend(bmesh, geom=[], use_contract=False, use_faces=False, use_face_step=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `region_extend` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionExtendIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `use_contract`.
    pub use_contract: bool,
    /// `use_faces`.
    pub use_faces: bool,
    /// `use_face_step`.
    pub use_face_step: bool,
}

/// Output slots of `region_extend` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RegionExtendOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `region_extend` on `bm`.
pub fn region_extend(bm: &mut BMesh, i: &RegionExtendIn) -> Result<RegionExtendOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.4: bmesh op region_extend")
}
