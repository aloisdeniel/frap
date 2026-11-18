// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.edge_rotate(use_ccw=False)`, tier T2
//! (`tests/golden/cases/edit/create/edge_rotate.py`). Port of
//! `edbm_edge_rotate_selected_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `bmesh.ops.rotate_edges`.
//!
//! Selected edges between two faces that are both selected or both unselected rotate (to the
//! next corners, clockwise unless `use_ccw`); the rotated edges are selected instead.
//! Errors: `Select edges or face pairs for edge loops to rotate about` (no selected edge),
//! `Could not find any selected edges that can be rotated`; WARNING `Unable to rotate N
//! edge(s)`.

use frap_bmesh::ops::rotate_edges::{RotateEdgesIn, edge_face_pair, rotate_edges};
use frap_bmesh::{BMElem, ElemFlag};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::EdgeRotate) -> OpResult {
    let use_ccw = p.use_ccw.value;
    let mut tot_failed_all = 0usize;
    let mut no_selected_edges = true;
    let mut invalid_selected_edges = true;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().1) == Some(0) {
            continue;
        }
        no_selected_edges = false;
        let tot = edbm::with_bm(cx, me, |bm| {
            bm.edge_ids()
                .into_iter()
                .filter(|&e| {
                    bm.edge(e).flag.contains(ElemFlag::SELECT)
                        && edge_face_pair(bm, e).is_some_and(|(fa, fb)| {
                            bm.face(fa).flag.contains(ElemFlag::SELECT)
                                == bm.face(fb).flag.contains(ElemFlag::SELECT)
                        })
                })
                .count()
        })
        .unwrap_or(0);
        if tot == 0 {
            continue;
        }
        invalid_selected_edges = false;
        let bm = cx.bmesh_mut(me)?;
        for e in bm.edge_ids() {
            let t = bm.edge(e).flag.contains(ElemFlag::SELECT)
                && edge_face_pair(bm, e).is_some_and(|(fa, fb)| {
                    bm.face(fa).flag.contains(ElemFlag::SELECT)
                        == bm.face(fb).flag.contains(ElemFlag::SELECT)
                });
            bm.edge_mut(e).flag.set(ElemFlag::TAG, t);
        }
        let edges = edbm::edges_with(bm, ElemFlag::TAG);
        let el: Vec<BMElem> = edges.iter().map(|&e| BMElem::Edge(e)).collect();
        edbm::select_disable(bm, &el, true);
        let r = rotate_edges(bm, &RotateEdgesIn { edges, use_ccw });
        let out = edbm::finish(cx, r)?;
        let bm = cx.bmesh_mut(me)?;
        let rotated = out.map(|o| o.edges).unwrap_or_default();
        let el: Vec<BMElem> = rotated.iter().map(|&e| BMElem::Edge(e)).collect();
        edbm::hidden_disable(bm, &el, true);
        edbm::select_enable(bm, &el, true);
        let tot_failed = tot - rotated.len();
        tot_failed_all += tot_failed;
        if tot_failed != 0 {
            // `BM_mesh_elem_hflag_enable_test(BM_EDGE, SELECT, respecthide, TAG)`.
            for e in bm.edge_ids() {
                let f = bm.edge(e).flag;
                if f.contains(ElemFlag::TAG) && !f.contains(ElemFlag::HIDDEN) {
                    bm.edge_select_set(e, true);
                }
            }
        }
        bm.select_flush_mode();
    }
    if no_selected_edges {
        return Err(OpError::Runtime(
            "Select edges or face pairs for edge loops to rotate about".into(),
        ));
    }
    if invalid_selected_edges {
        return Err(OpError::Runtime(
            "Could not find any selected edges that can be rotated".into(),
        ));
    }
    if tot_failed_all != 0 {
        cx.report(
            ReportType::Warning,
            format!("Unable to rotate {tot_failed_all} edge(s)"),
        );
    }
    Ok(OpStatus::Finished)
}
