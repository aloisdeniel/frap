// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.dissolve_faces(use_verts=False)`, tier T1
//! (`tests/golden/cases/edit/delete/dissolve_faces*.py`). Port of `edbm_dissolve_faces_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode with selected faces, `bmesh.ops.dissolve_faces` runs on the
//! selected (visible) faces: each connected region of two or more faces becomes one n-gon,
//! which is selected (the selection is extended, `EDBM_op_call_and_selectf`). A single face
//! stays as it is. Always `FINISHED`.

use frap_bmesh::BMElem;
use frap_bmesh::ops::dissolve_faces::{DissolveFacesIn, dissolve_faces};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, sel_faces, select_elems, totals};

/// The body of `edbm_dissolve_faces_exec` (shared with `mesh.dissolve_mode`).
pub(crate) fn run(cx: &mut OpCtx, use_verts: bool) -> OpResult {
    let mut removed = Removed::default();
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().2 == 0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        let i = DissolveFacesIn {
            faces: sel_faces(bm),
            use_verts,
        };
        let out = dissolve_faces(bm, &i).map_err(bm_err)?;
        let region: Vec<BMElem> = out.region.iter().map(|&f| BMElem::Face(f)).collect();
        select_elems(bm, &region);
        removed.add(old, totals(bm));
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = ["each connected region of selected faces becomes one selected n-gon; a single face is left alone"]
)]
fn exec(cx: &mut OpCtx, p: mesh::DissolveFaces) -> OpResult {
    run(cx, p.use_verts.value)
}
