// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.wireframe` (Blender 5.2.4 `bmo_wireframe`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.wireframe(bmesh, faces=[], thickness=0.0, offset=0.0, use_replace=False, use_boundary=False, use_even_offset=False, use_crease=False, crease_weight=0.0, use_relative_offset=False, material_offset=0)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `wireframe` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WireframeIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `thickness`.
    pub thickness: f32,
    /// `offset`.
    pub offset: f32,
    /// `use_replace`.
    pub use_replace: bool,
    /// `use_boundary`.
    pub use_boundary: bool,
    /// `use_even_offset`.
    pub use_even_offset: bool,
    /// `use_crease`.
    pub use_crease: bool,
    /// `crease_weight`.
    pub crease_weight: f32,
    /// `use_relative_offset`.
    pub use_relative_offset: bool,
    /// `material_offset`.
    pub material_offset: i32,
}

/// Output slots of `wireframe` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct WireframeOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `wireframe` on `bm`.
pub fn wireframe(bm: &mut BMesh, i: &WireframeIn) -> Result<WireframeOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op wireframe")
}
