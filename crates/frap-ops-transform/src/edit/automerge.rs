// SPDX-License-Identifier: GPL-3.0-or-later
//! Auto Merge with split after edit-mode vertex transforms (`T_AUTOSPLIT`:
//! `EDBM_automerge_and_split`, which welds like `EDBM_automerge` and then splits the edges and
//! faces the moved vertices cross through `BM_mesh_intersect_edges`). It enables
//! `mesh.extrude_manifold` (forced `use_automerge_and_split=True`) and
//! `tool_settings.use_mesh_automerge_and_split`.
//!
//! M2 leftover carried into M4 by W0 (decision log M34W0): WP 4.6 owns this file,
//! `frap-bmesh/src/ops/intersect_edges.rs` and `frap-ops-mesh/src/ops/extrude_manifold.rs`;
//! it sets [`IMPLEMENTED`] once the port lands. Port target:
//! blender/source/blender/editors/mesh/editmesh_automerge.cc (`EDBM_automerge_and_split`).

use frap_bmesh::BMesh;
use frap_op::OpError;

/// Whether `use_automerge_and_split=True` is implemented (the edit-mode transforms refuse it
/// up front with `OP_NOT_IMPLEMENTED` while it is not).
pub const IMPLEMENTED: bool = false;

/// `EDBM_automerge_and_split(obedit, split_edges = true, split_faces = true, update = true,
/// BM_ELEM_SELECT, dist)`.
pub fn automerge_and_split(bm: &mut BMesh, dist: f32) -> Result<(), OpError> {
    let _ = (bm, dist);
    Err(super::automerge_and_split_not_implemented())
}
