// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_boundary_loop_multi(extend=True, delimit_edge_loop=set())`, tier T1
//! (`tests/golden/cases/edit/select_m8/boundary_loop_multi.py`). Port of
//! `edbm_boundary_loop_multiselect_exec` and `walker_select` (`editors/mesh/editmesh_select.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! The selected boundary edges of every mesh in Edit Mode (taken before anything changes, in
//! edge order) each select their whole boundary loop (`BMW_EDGELOOP` with
//! `BMW_FLAG_TEST_HIDDEN` and the `delimit_edge_loop` delimiters); an edge already walked is
//! not walked again. Without `extend` each mesh is deselected first. Without a selected
//! boundary edge on any mesh the operator is `CANCELLED` with Blender's error `At least one
//! boundary edge is needed to make a boundary loop selection`; when nothing changed it reports
//! the INFO `The selection has not changed. The full delimited loop was already selected`.

use frap_bmesh::walkers::WalkOpts;
use frap_bmesh::{BMElem, EdgeId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::select_more::common::{self, Ngons};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectBoundaryLoopMulti) -> OpResult {
    let objects = common::edit_meshes(cx);
    let mut object_edges: Vec<Vec<EdgeId>> = Vec::with_capacity(objects.len());
    let mut has_selected_boundary = false;
    for &(_, me) in &objects {
        let edges = common::read(cx, me, |bm| {
            bm.edge_ids()
                .into_iter()
                .filter(|&e| common::selected(bm, e) && bm.edge_is_boundary(e))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
        has_selected_boundary |= !edges.is_empty();
        object_edges.push(edges);
    }
    if !has_selected_boundary {
        return Err(OpError::Runtime(
            "At least one boundary edge is needed to make a boundary loop selection".into(),
        ));
    }
    let delimit = common::walk_delimit(&p.delimit_edge_loop.value, Ngons::Loop);
    let mut changed_multi = false;
    for (i, &(_, me)) in objects.iter().enumerate() {
        let mut changed = false;
        let mut changed_boundary = false;
        let bm = cx.bmesh_mut(me)?;
        if !p.extend.value {
            common::deselect_all(bm);
            changed = true;
        }
        let mut tagged = vec![false; bm.edges.capacity()];
        for &e in &object_edges[i] {
            if tagged[e.slot()] {
                continue;
            }
            let o = WalkOpts {
                delimit,
                ..WalkOpts::default()
            };
            let walked = bm.walk_edge_loop_ex(e, &o);
            for &x in &walked {
                tagged[x.slot()] = true;
            }
            let elems: Vec<BMElem> = walked.into_iter().map(BMElem::Edge).collect();
            changed_boundary |= common::walker_select(bm, &elems, true);
            changed |= changed_boundary;
        }
        if changed {
            if changed_boundary {
                common::selectmode_flush(bm);
            }
            changed_multi = true;
        }
    }
    if !changed_multi {
        cx.report(
            ReportType::Info,
            "The selection has not changed. The full delimited loop was already selected",
        );
    }
    Ok(OpStatus::Finished)
}
