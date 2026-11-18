// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.smooth_vert` (Blender 5.2.4 `bmo_smooth_vert`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.10 (Split, separate, duplicate, triangulate, misc).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.smooth_vert(bmesh, verts=[], factor=0.0, mirror_clip_x=False, mirror_clip_y=False, mirror_clip_z=False, clip_dist=0.0, use_axis_x=False, use_axis_y=False, use_axis_z=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `smooth_vert` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SmoothVertIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `factor`.
    pub factor: f32,
    /// `mirror_clip_x`.
    pub mirror_clip_x: bool,
    /// `mirror_clip_y`.
    pub mirror_clip_y: bool,
    /// `mirror_clip_z`.
    pub mirror_clip_z: bool,
    /// `clip_dist`.
    pub clip_dist: f32,
    /// `use_axis_x`.
    pub use_axis_x: bool,
    /// `use_axis_y`.
    pub use_axis_y: bool,
    /// `use_axis_z`.
    pub use_axis_z: bool,
}

/// Output slots of `smooth_vert` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SmoothVertOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `smooth_vert` on `bm`.
pub fn smooth_vert(bm: &mut BMesh, i: &SmoothVertIn) -> Result<SmoothVertOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.10: bmesh op smooth_vert")
}
