// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.dissolve_degenerate(threshold=1e-4)`, tier T1
//! (`tests/golden/cases/edit/delete/dissolve_degenerate.py`). Port of
//! `edbm_dissolve_degenerate_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode, `bmesh.ops.dissolve_degenerate` runs on the selected (visible)
//! edges: edges shorter than `threshold` collapse and degenerate face corners are clipped.
//! Where the mesh changed, the selection is flushed up from the vertices. Reports `No
//! degenerate geometry found` (INFO) when nothing changed, else `Removed: N vertices, N
//! edges, N faces` over the changed meshes. Always `FINISHED`.

use frap_bmesh::ops::dissolve_degenerate::{DissolveDegenerateIn, dissolve_degenerate};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, sel_edges, totals};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = ["INFO 'No degenerate geometry found' or 'Removed: N vertices, N edges, N faces'"]
)]
fn exec(cx: &mut OpCtx, p: mesh::DissolveDegenerate) -> OpResult {
    let mut removed = Removed::default();
    let mut changed_multi = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        // Without selected edges the bmesh operator has no input and changes nothing.
        if edit_bm(cx, me).is_none_or(|bm| sel_edges(bm).is_empty()) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        let i = DissolveDegenerateIn {
            dist: p.threshold.value,
            edges: sel_edges(bm),
        };
        dissolve_degenerate(bm, &i).map_err(bm_err)?;
        let new = totals(bm);
        if old == new {
            continue;
        }
        // Tricky to maintain correct selection here, so just flush up from verts.
        bm.select_flush();
        changed_multi = true;
        removed.add(old, new);
    }
    if changed_multi {
        cx.report(ReportType::Info, removed.delete_info());
    } else {
        cx.report(ReportType::Info, "No degenerate geometry found");
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}
