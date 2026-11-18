// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_loose(extend=False)`, tier T1 (`tests/golden/cases/edit/select/
//! loose_sides.py`). Port of `edbm_select_loose_exec` (`editors/mesh/editmesh_select.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode (after deselecting all unless `extend`), for each enabled select mode:
//! vertex mode selects visible vertices without edges, edge mode visible wire edges, face mode
//! visible faces whose edges are all boundary edges (faces connected to nothing). The selection
//! is then flushed.

use frap_bmesh::SelectMode;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectLoose) -> OpResult {
    for (_, me) in common::edit_meshes(cx) {
        let bm = cx.bmesh_mut(me)?;
        let mut changed = false;
        if !p.extend.value {
            common::deselect_all(bm);
            changed = true;
        }
        let mode = bm.select_mode;
        if mode.contains(SelectMode::VERT) {
            for v in bm.vert_ids() {
                if !common::hidden(bm, v) && bm.vert(v).e.is_none() {
                    bm.vert_select_set(v, true);
                    changed = true;
                }
            }
        }
        if mode.contains(SelectMode::EDGE) {
            for e in bm.edge_ids() {
                if !common::hidden(bm, e) && bm.edge_is_wire(e) {
                    bm.edge_select_set(e, true);
                    changed = true;
                }
            }
        }
        if mode.contains(SelectMode::FACE) {
            for f in bm.face_ids() {
                if common::hidden(bm, f) {
                    continue;
                }
                let is_loose = bm
                    .face_loops(f)
                    .into_iter()
                    .all(|l| bm.edge_is_boundary(bm.lp(l).e));
                if is_loose {
                    bm.face_select_set(f, true);
                    changed = true;
                }
            }
        }
        if changed {
            common::selectmode_flush(bm);
        }
    }
    Ok(OpStatus::Finished)
}
