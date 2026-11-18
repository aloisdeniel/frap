// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.bisect_plane` (Blender 5.2.4 `bmo_bisect_plane`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.bisect_plane(bmesh, geom=[], dist=0.0, plane_co=Vector(), plane_no=Vector(), use_snap_center=False, clear_outer=False, clear_inner=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError};

/// Input slots of `bisect_plane` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BisectPlaneIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `dist`.
    pub dist: f32,
    /// `plane_co`.
    pub plane_co: [f32; 3],
    /// `plane_no`.
    pub plane_no: [f32; 3],
    /// `use_snap_center`.
    pub use_snap_center: bool,
    /// `clear_outer`.
    pub clear_outer: bool,
    /// `clear_inner`.
    pub clear_inner: bool,
}

/// Output slots of `bisect_plane` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BisectPlaneOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `bisect_plane` on `bm`.
pub fn bisect_plane(bm: &mut BMesh, i: &BisectPlaneIn) -> Result<BisectPlaneOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op bisect_plane")
}
