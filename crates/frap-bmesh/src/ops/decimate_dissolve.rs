// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_decimate_dissolve` (the DECIMATE modifier's `DISSOLVE` mode, "Planar"): limited
//! dissolve of every vertex and edge of the mesh. M4 W0 wrote the wrapper (an exact port);
//! WP 4.5 owns this file.
//!
//! Ported from blender/source/blender/bmesh/tools/bmesh_decimate_dissolve.cc
//! (`BM_mesh_decimate_dissolve`), Blender 5.2.4, GPL-2.0-or-later.

use crate::BMesh;
use crate::ops::dissolve_limit::{Delimit, decimate_dissolve_ex};

/// `BM_mesh_decimate_dissolve(bm, angle_limit, do_dissolve_boundaries, delimit)`.
pub fn decimate_dissolve(
    bm: &mut BMesh,
    angle_limit: f32,
    do_dissolve_boundaries: bool,
    delimit: Delimit,
) {
    let verts = bm.vert_ids();
    let edges = bm.edge_ids();
    decimate_dissolve_ex(
        bm,
        angle_limit,
        do_dissolve_boundaries,
        delimit,
        &verts,
        &edges,
        0,
    );
}
