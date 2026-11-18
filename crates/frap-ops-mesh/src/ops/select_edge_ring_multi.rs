// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_edge_ring_multi(delimit_edge_ring={'NGONS'})`, tier T1
//! (`tests/golden/cases/edit/select/loop_multi*.py`). Port of
//! `edbm_edge_ring_multiselect_exec` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with selected edges, each selected edge (taken before anything
//! changes, in edge order) adds its edge ring (`BMW_EDGERING` with the delimiters; e.g. edge 0
//! of the default cube gives edges 0, 2, 7 and 8). The selection is then flushed.

use frap_bmesh::walkers::WalkOpts;
use frap_bmesh::{BMElem, EdgeId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common::{self, Ngons};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectEdgeRingMulti) -> OpResult {
    let delimit = common::walk_delimit(&p.delimit_edge_ring.value, Ngons::Ring);
    let o = WalkOpts {
        delimit,
        ..WalkOpts::default()
    };
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
            let elems: Vec<BMElem> = bm
                .walk_edge_ring_ex(e, &o)
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
