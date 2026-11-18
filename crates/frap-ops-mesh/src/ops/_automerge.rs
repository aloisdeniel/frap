// SPDX-License-Identifier: GPL-3.0-or-later
//! (Not an operator: the leading `_` keeps it out of the registry test.) Auto Merge after the
//! edit-mode tools that apply it (`mesh.bevel`, `mesh.symmetrize`): `EDBM_automerge_connected`,
//! run when `tool_settings.use_mesh_automerge` is on with `tool_settings.double_threshold`.
//! Closes the M2 leftover of D-79 (M4 W0, decision log M34W0).
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_automerge.cc
//! (`edbm_automerge_impl(…, BM_ELEM_SELECT, dist, use_connected = true, use_centroid = false)`),
//! Blender 5.2.4, GPL-2.0-or-later.

use frap_bmesh::ops::find_doubles::{FindDoublesIn, find_doubles};
use frap_bmesh::ops::weld_verts::{WeldVertsIn, weld_verts};
use frap_bmesh::{BMesh, ElemFlag, VertId};
use frap_op::prelude::*;

/// `tool_settings.use_mesh_automerge` and `double_threshold` of the context scene, when Auto
/// Merge is on.
pub(crate) fn threshold(cx: &OpCtx) -> Option<f32> {
    let ctx = cx.ctx();
    let ts = ctx.tool_settings();
    ts.use_mesh_automerge.then_some(ts.double_threshold)
}

/// `EDBM_automerge_connected(obedit, false, BM_ELEM_SELECT, dist)`: every vertex is searched,
/// selected vertices merge into connected unselected (visible) ones within `dist`. Returns
/// whether vertices were removed.
pub(crate) fn automerge_connected(bm: &mut BMesh, dist: f32) -> Result<bool, OpError> {
    let before = bm.verts.iter().count();
    let verts: Vec<VertId> = bm.vert_ids();
    let keep_verts: Vec<VertId> = bm
        .verts
        .iter()
        .filter(|(_, v)| !v.flag.contains(ElemFlag::SELECT) && !v.flag.contains(ElemFlag::HIDDEN))
        .map(|(i, _)| VertId(i))
        .collect();
    let err = |e: frap_bmesh::BmError| OpError::Runtime(e.to_string());
    let found = find_doubles(
        bm,
        &FindDoublesIn {
            verts,
            keep_verts,
            use_connected: true,
            dist,
        },
    )
    .map_err(err)?;
    weld_verts(
        bm,
        &WeldVertsIn {
            targetmap: found.targetmap,
            use_centroid: false,
            average_vert_data: false,
        },
    )
    .map_err(err)?;
    Ok(bm.verts.iter().count() != before)
}
