// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_edge_loop_multi(delimit_edge_loop={'OUTER_CORNERS', 'NGONS'})`, tier
//! T1 (`tests/golden/cases/edit/select/loop_multi*.py`). Port of
//! `edbm_edge_loop_multiselect_exec` and `walker_select` (`editors/mesh/editmesh_select.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with selected edges, each selected edge (taken before anything
//! changes, in edge order) adds its edge loop (`BMW_EDGELOOP` with the delimiters; an edge with
//! more than two faces walks `BMW_EDGELOOP_NONMANIFOLD`, which ignores them). The selection is
//! then flushed. Replaces Blender's old `loop_multi_select(ring=False)`.

use frap_bmesh::walkers::WalkOpts;
use frap_bmesh::{BMElem, EdgeId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::loop_select::walk;
use super::select_more::common::{self, Ngons};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectEdgeLoopMulti) -> OpResult {
    let delimit = common::walk_delimit(&p.delimit_edge_loop.value, Ngons::Loop);
    for (_, me) in common::edit_meshes(cx) {
        let Some(edarray) = common::read(cx, me, |bm| -> Vec<EdgeId> {
            bm.edge_ids()
                .into_iter()
                .filter(|&e| common::selected(bm, e))
                .collect()
        }) else {
            continue;
        };
        if edarray.is_empty() {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let mut changed = false;
        for e in edarray {
            let elems: Vec<BMElem> = if bm.edge_face_count(e) > 2 {
                walk::edge_loop_nonmanifold(bm, e)
            } else {
                let o = WalkOpts {
                    delimit,
                    ..WalkOpts::default()
                };
                bm.walk_edge_loop_ex(e, &o)
            }
            .into_iter()
            .map(BMElem::Edge)
            .collect();
            changed |= common::walker_select(bm, &elems, true);
        }
        if changed {
            common::selectmode_flush(bm);
        }
    }
    Ok(OpStatus::Finished)
}
