// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.vert_connect_concave()` (Split Concave Faces), tier T2 (`tests/golden/cases/edit/create/vert_connect_concave.py`
//! covers convex input; concave faces need WP 2.10's `triangulate`). Port of
//! `edbm_vert_connect_concave_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `bmesh.ops.connect_verts_concave` (which triangulates through WP
//! 2.10's `triangulate`): selected concave faces are cut into convex pieces, which are added to
//! the selection.

use frap_bmesh::ops::connect_verts::concave::{ConnectVertsConcaveIn, connect_verts_concave};
use frap_bmesh::BMElem;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, _p: mesh::VertConnectConcave) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().2) == Some(0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::sel_faces(bm);
        let r = connect_verts_concave(bm, &ConnectVertsConcaveIn { faces });
        if let Some(out) = edbm::finish(cx, r)? {
            let bm = cx.bmesh_mut(me)?;
            let faces: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
            edbm::select_enable(bm, &faces, true);
        }
    }
    Ok(OpStatus::Finished)
}
