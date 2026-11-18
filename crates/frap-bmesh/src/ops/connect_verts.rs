// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.connect_verts` (Blender 5.2.4 `bmo_connect_verts`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.connect_verts(bmesh, verts=[], faces_exclude=[], check_degenerate=False)`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId, VertId};

/// Input slots of `connect_verts` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectVertsIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `faces_exclude`.
    pub faces_exclude: Vec<FaceId>,
    /// `check_degenerate`.
    pub check_degenerate: bool,
}

/// Output slots of `connect_verts` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConnectVertsOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `connect_verts` on `bm`.
pub fn connect_verts(bm: &mut BMesh, i: &ConnectVertsIn) -> Result<ConnectVertsOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op connect_verts")
}
