// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.vert_connect_nonplanar(angle_limit=5°)` (Split Non-Planar Faces), tier T1
//! (`tests/golden/cases/edit/create/vert_connect_nonplanar.py`). Port of
//! `edbm_vert_connect_nonplaner_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `bmesh.ops.connect_verts_nonplanar`: selected faces bent more than
//! `angle_limit` are split along their flattest diagonal, recursively; the pieces are added to
//! the selection.

use frap_bmesh::ops::connect_verts::nonplanar::{
    ConnectVertsNonplanarIn, connect_verts_nonplanar,
};
use frap_bmesh::BMElem;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::edge_face_add::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::VertConnectNonplanar) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if edbm::with_bm(cx, me, |bm| bm.selected_counts().2) == Some(0) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let faces = edbm::sel_faces(bm);
        let r = connect_verts_nonplanar(
            bm,
            &ConnectVertsNonplanarIn {
                angle_limit: p.angle_limit.value,
                faces,
            },
        );
        if let Some(out) = edbm::finish(cx, r)? {
            let bm = cx.bmesh_mut(me)?;
            let faces: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
            edbm::select_enable(bm, &faces, true);
        }
    }
    Ok(OpStatus::Finished)
}
