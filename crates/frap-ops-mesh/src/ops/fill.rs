// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.fill(use_beauty=True)`, tier T1 (`tests/golden/cases/edit/create/fill_*.py`).
//! Port of `edbm_fill_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `bmesh.ops.triangle_fill`.
//!
//! Every mesh in Edit Mode with selected edges fills them with triangles (scan-fill, then
//! beautified unless `use_beauty=False`); the new faces and edges are selected. No selected
//! edge anywhere: `RuntimeError: Error: No edges selected`; nothing filled: WARNING `No faces
//! filled` and `CANCELLED`.

use frap_bmesh::BMElem;
use frap_bmesh::ops::triangle_fill::{TriangleFillIn, triangle_fill};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Fill) -> OpResult {
    let use_beauty = p.use_beauty.value;
    let mut has_selected_edges = false;
    let mut has_faces_filled = false;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().1) == Some(0) {
            continue;
        }
        has_selected_edges = true;
        let bm = cx.bmesh_mut(me)?;
        let totface_orig = bm.faces_num();
        let edges = edbm::sel_edges(bm);
        let r = triangle_fill(
            bm,
            &TriangleFillIn {
                use_beauty,
                edges,
                ..Default::default()
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        if totface_orig == bm.faces_num() {
            continue;
        }
        has_faces_filled = true;
        let sel: Vec<BMElem> = out
            .geom
            .iter()
            .copied()
            .filter(|g| matches!(g, BMElem::Face(_) | BMElem::Edge(_)))
            .collect();
        edbm::select_enable(bm, &sel, true);
    }
    if !has_selected_edges {
        return Err(OpError::Runtime("No edges selected".into()));
    }
    if !has_faces_filled {
        cx.report(ReportType::Warning, "No faces filled");
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
