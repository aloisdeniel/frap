// SPDX-License-Identifier: GPL-3.0-or-later
//! Auto Merge with split after edit-mode vertex transforms (`T_AUTOSPLIT`:
//! `EDBM_automerge_and_split`, which welds like `EDBM_automerge` and then splits the edges and
//! faces the moved vertices cross through `BM_mesh_intersect_edges`). It enables
//! `mesh.extrude_manifold` (forced `use_automerge_and_split=True`) and
//! `tool_settings.use_mesh_automerge_and_split`.
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_automerge.cc
//! (`EDBM_automerge_and_split`) and the Auto Merge part of
//! blender/source/blender/editors/transform/transform_convert_mesh.cc
//! (`special_aftertrans_update__mesh`: the face re-selection after the weld), Blender 5.2.4,
//! GPL-2.0-or-later. The intersection search is `frap_bmesh::ops::intersect_edges`
//! (`BM_mesh_intersect_edges`, WP 4.6).

use frap_bmesh::ops::intersect_edges::intersect_edges;
use frap_bmesh::ops::weld_verts::{WeldVertsIn, weld_verts};
use frap_bmesh::{BMesh, SelectMode};
use frap_op::OpError;

/// Whether `use_automerge_and_split=True` is implemented (the edit-mode transforms refuse it
/// up front with `OP_NOT_IMPLEMENTED` while it is not).
pub const IMPLEMENTED: bool = true;

/// `EDBM_automerge_and_split(obedit, split_edges = true, split_faces = true, update = true,
/// BM_ELEM_SELECT, dist)` followed by the transform's face re-selection: the selected
/// vertices and edges are intersected with the rest of the mesh within `dist`
/// (`BM_mesh_intersect_edges`), the pairs found are welded (`weld_verts`), and in face select
/// mode the selection is flushed from the edges again.
pub fn automerge_and_split(bm: &mut BMesh, dist: f32) -> Result<(), OpError> {
    let has_face_sel = bm.selected_counts().2 != 0;
    let mut targetmap = Vec::new();
    if intersect_edges(bm, true, dist, true, &mut targetmap) {
        weld_verts(
            bm,
            &WeldVertsIn {
                targetmap,
                use_centroid: false,
                average_vert_data: false,
            },
        )
        .map_err(|e| OpError::Runtime(e.to_string()))?;
    }
    if has_face_sel && bm.select_mode == SelectMode::FACE {
        bm.select_flush_mode_ex(SelectMode::EDGE, false);
    }
    Ok(())
}
