// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.offset_edge_loops(use_cap_endpoint=False)` (an INTERNAL operator, normally run
//! by `offset_edge_loops_slide`), tier T1 (`tests/golden/cases/edit/subdivide/offset_*.py`).
//! Port of `edbm_offset_edgeloop_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `frap_bmesh::ops::offset_edgeloops`.
//!
//! The selected edge loops are doubled: the faces beside them are cut by new loops placed on
//! the old vertices (the edge slide of the macro moves them apart), and only the new loops end
//! up selected. Every edit mesh with a selected edge takes part; nothing selected anywhere
//! answers `CANCELLED`. In face-only select mode every edit mesh switches to edge mode.

use frap_bmesh::ops::offset_edgeloops::{OffsetEdgeloopsIn, offset_edgeloops};
use frap_bmesh::{EdgeId, ElemFlag, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::loopcut::selectmode_disable_multi;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::OffsetEdgeLoops) -> OpResult {
    let use_cap_endpoint = p.use_cap_endpoint.value;
    let mut changed_multi = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let any = {
            let (_, edit) = cx.split_edit();
            edit.meshes
                .get(&me.key())
                .is_some_and(|em| em.bm.selected_counts().1 > 0)
        };
        if !any {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let edges: Vec<EdgeId> = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| bm.edge(e).flag.contains(ElemFlag::SELECT))
            .collect();
        let out = offset_edgeloops(
            bm,
            &OffsetEdgeloopsIn {
                edges,
                use_cap_endpoint,
            },
        )
        .map_err(|e| OpError::Runtime(e.to_string()))?;
        // Deselect everything, then select the new loops.
        bm.select_history.clear();
        for v in bm.vert_ids() {
            bm.vert_mut(v).flag.remove(ElemFlag::SELECT);
        }
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.remove(ElemFlag::SELECT);
        }
        for f in bm.face_ids() {
            bm.face_mut(f).flag.remove(ElemFlag::SELECT);
        }
        for e in out.edges {
            bm.edge_select_set(e, true);
            if !bm.edge(e).flag.contains(ElemFlag::HIDDEN) {
                bm.edge_mut(e).flag.insert(ElemFlag::SELECT);
            }
        }
        changed_multi = true;
    }
    if !changed_multi {
        return Ok(OpStatus::Cancelled);
    }
    // In face-only select mode, switch to edge select mode so an edge-only selection is not
    // an inconsistent state (every edit mesh, to keep them in sync).
    let ts_mode = cx.ctx().scene().tool_settings.mesh_select_mode;
    if ts_mode == [false, false, true] {
        selectmode_disable_multi(cx, SelectMode::FACE, SelectMode::EDGE)?;
    }
    Ok(OpStatus::Finished)
}
