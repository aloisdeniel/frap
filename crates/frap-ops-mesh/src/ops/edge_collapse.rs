// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.edge_collapse()`, tier T1 (`tests/golden/cases/edit/delete/edge_collapse.py`).
//! Port of `edbm_collapse_edge_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode with selected edges, `bmesh.ops.collapse(uvs=True)` runs on the
//! selected (visible) edges: each connected group of them collapses to the mean of its edges'
//! endpoints, with UVs and colours snapped per island. Always `FINISHED`.

use frap_bmesh::ops::collapse::{CollapseIn, collapse};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, sel_edges, totals};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = ["each connected group of selected edges collapses to one vertex at the mean of its edges' endpoints"]
)]
fn exec(cx: &mut OpCtx, _p: mesh::EdgeCollapse) -> OpResult {
    let mut removed = Removed::default();
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().1 == 0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        let i = CollapseIn {
            edges: sel_edges(bm),
            uvs: true,
        };
        collapse(bm, &i).map_err(bm_err)?;
        removed.add(old, totals(bm));
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}
