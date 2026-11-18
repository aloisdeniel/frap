// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.delete_edgeloop(use_face_split=True)`, tier T1
//! (`tests/golden/cases/edit/delete/delete_edgeloop.py`). Port of `edbm_delete_edgeloop_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode with selected edges: the faces around the selected edges are
//! tagged, `bmesh.ops.dissolve_edges` runs on the selected edges with `use_verts=True`,
//! `angle_threshold=π` (and `use_preserve_quads=False`), so the faces on both sides of the
//! loop merge and the loop's vertices dissolve; the faces that inherited the tag (the merged
//! faces) are selected and the selection is flushed from the vertices. Always `FINISHED`.

use frap_bmesh::ElemFlag;
use frap_bmesh::ops::dissolve_edges::{DissolveEdgesIn, dissolve_edges};
use frap_bmesh::{BMElem, SelectMode};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, sel_edges, totals};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "dissolve_edges(use_verts=True, angle_threshold=pi) on the selected edges; the merged faces are selected",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::DeleteEdgeloop) -> OpResult {
    let mut removed = Removed::default();
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().1 == 0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        // Deal with selection.
        for f in bm.faces.slots.iter_mut().flatten() {
            f.flag.remove(ElemFlag::TAG);
        }
        for e in bm.edge_ids() {
            if bm.edge(e).flag.contains(ElemFlag::SELECT) {
                for f in bm.edge_faces(e) {
                    bm.face_mut(f).flag.insert(ElemFlag::TAG);
                }
            }
        }
        let i = DissolveEdgesIn {
            edges: sel_edges(bm),
            use_verts: true,
            use_face_split: p.use_face_split.value,
            angle_threshold: core::f64::consts::PI as f32,
            use_preserve_quads: false,
        };
        dissolve_edges(bm, &i).map_err(bm_err)?;
        // `BM_mesh_elem_hflag_enable_test(bm, BM_FACE, BM_ELEM_SELECT, true, false, BM_ELEM_TAG)`.
        for f in bm.face_ids() {
            let fl = bm.face(f).flag;
            if !fl.contains(ElemFlag::HIDDEN) && fl.contains(ElemFlag::TAG) {
                bm.select_set(BMElem::Face(f), true);
            }
        }
        bm.select_flush_mode_ex(SelectMode::VERT, false);
        // `EDBM_uvselect_clear` only invalidates Blender's UV-selection sync, which frap does
        // not keep.
        removed.add(old, totals(bm));
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}
