// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.connect_vert_pair` (Blender 5.2.4 `bmo_connect_vert_pair`): **stub** frozen by M2 W0 (WP 2.0),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.connect_vert_pair(bmesh, verts=[], verts_exclude=[], faces_exclude=[])`.
//!
//! The input struct mirrors Blender's input slots one to one (same names, Blender's slot
//! defaults in `Default`), so behaviour can be cross-checked against `bmesh.ops.*`. The
//! owning WP adds Blender's output slots to the output struct and starts the file with the
//! create/kill order note of ARCHITECTURE §6.2.

use crate::{BMElem, BMesh, BmError, FaceId, VertId};

/// Input slots of `connect_vert_pair` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectVertPairIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `verts_exclude`.
    pub verts_exclude: Vec<VertId>,
    /// `faces_exclude`.
    pub faces_exclude: Vec<FaceId>,
}

/// Output slots of `connect_vert_pair` (the owning WP adds Blender's `*.out` slots).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConnectVertPairOut {
    /// Every element the op created or kept as its result (`geom.out`-like).
    pub geom: Vec<BMElem>,
}

/// Runs `connect_vert_pair` on `bm`.
pub fn connect_vert_pair(bm: &mut BMesh, i: &ConnectVertPairIn) -> Result<ConnectVertPairOut, BmError> {
    let _ = (bm, i);
    todo!("WP 2.8: bmesh op connect_vert_pair")
}
