// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_intersect_edges` (Auto Merge with split: splits the edges and faces that the
//! selected vertices and edges cross, then welds through a target map). WP 4.6 (M2 leftover,
//! decision log M34W0); stub created by M4 W0.
//!
//! Port target: blender/source/blender/bmesh/tools/bmesh_intersect_edges.cc, Blender 5.2.4.

use crate::{BMesh, VertId};

/// `BM_mesh_intersect_edges(bm, hflag, dist, split_faces, r_targetmap)`: returns whether
/// anything was split; `targetmap` receives the vertex pairs to weld (`weld_verts`' map
/// format).
pub fn intersect_edges(
    bm: &mut BMesh,
    select_only: bool,
    dist: f32,
    split_faces: bool,
    targetmap: &mut Vec<(VertId, VertId)>,
) -> bool {
    let _ = (bm, select_only, dist, split_faces, targetmap);
    todo!("WP 4.6: BM_mesh_intersect_edges")
}
