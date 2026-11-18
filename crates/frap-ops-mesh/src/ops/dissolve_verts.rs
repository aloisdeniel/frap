// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.dissolve_verts(use_face_split=False, use_boundary_tear=False)`, tier T1
//! (`tests/golden/cases/edit/delete/dissolve_verts*.py`). Port of `edbm_dissolve_verts_exec`
//! (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! On every mesh in Edit Mode with selected vertices, `bmesh.ops.dissolve_verts` runs on the
//! selected (visible) vertices: the faces around each merge into one, and vertices left
//! between two edges join them. The selection is flushed by the bmesh operator. Always
//! `FINISHED`.

use frap_bmesh::ops::dissolve_verts::{DissolveVertsIn, dissolve_verts};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::delete::common::{Removed, bm_err, edit_bm, sel_verts, totals};

/// The body of `edbm_dissolve_verts_exec` (shared with `mesh.dissolve_mode`).
pub(crate) fn run(cx: &mut OpCtx, use_face_split: bool, use_boundary_tear: bool) -> OpResult {
    let mut removed = Removed::default();
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().0 == 0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let old = totals(bm);
        let i = DissolveVertsIn {
            verts: sel_verts(bm),
            use_face_split,
            use_boundary_tear,
        };
        dissolve_verts(bm, &i).map_err(bm_err)?;
        removed.add(old, totals(bm));
    }
    removed.write_result(cx);
    Ok(OpStatus::Finished)
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = ["result.removed / removed_edges / removed_faces: net element counts removed"]
)]
fn exec(cx: &mut OpCtx, p: mesh::DissolveVerts) -> OpResult {
    run(cx, p.use_face_split.value, p.use_boundary_tear.value)
}
