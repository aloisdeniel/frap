// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.edges_select_sharp(sharpness=0.523599)`, tier T1 (`tests/golden/cases/edit/
//! select/sharp_flat.py`). Port of `edbm_select_sharp_edges_exec`
//! (`editors/mesh/editmesh_select.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode, every visible edge with exactly two faces whose normals differ by
//! more than `sharpness` (`dot(n_a, n_b) < cos(sharpness)`) is added to the selection (the
//! default cube: all 12 edges). In face-only select mode the faces using them are selected
//! instead (`EDBM_selectmode_convert(EDGE, FACE)`); otherwise the selection is flushed.

use frap_bmesh::{BMesh, EdgeId, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;

/// `dot_v3v3` (single precision, left to right).
pub(crate) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `BM_edge_loop_pair`: the two face normals of an edge with exactly two faces.
fn loop_pair_normals(bm: &BMesh, e: EdgeId) -> Option<([f32; 3], [f32; 3])> {
    let la = bm.edge(e).l?;
    let lb = bm.lp(la).radial_next;
    if la == lb || bm.lp(lb).radial_next != la {
        return None;
    }
    Some((bm.face(bm.lp(la).f).no, bm.face(bm.lp(lb).f).no))
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::EdgesSelectSharp) -> OpResult {
    let angle_limit_cos = p.sharpness.value.cos();
    for (_, me) in common::edit_meshes(cx) {
        let bm = cx.bmesh_mut(me)?;
        for e in bm.edge_ids() {
            if common::hidden(bm, e) {
                continue;
            }
            if let Some((a, b)) = loop_pair_normals(bm, e)
                && dot(a, b) < angle_limit_cos
            {
                bm.edge_select_set(e, true);
            }
        }
        if !bm
            .select_mode
            .intersects(SelectMode::VERT | SelectMode::EDGE)
        {
            // Edges cannot be selected on their own: select the faces using them.
            bm.selectmode_convert(SelectMode::EDGE, SelectMode::FACE);
        } else {
            common::selectmode_flush(bm);
        }
    }
    Ok(OpStatus::Finished)
}
