// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.faces_shade_smooth()`, tier T1
//! (`tests/golden/cases/edit/normals/shade_*.py`). Port of `edbm_faces_shade_smooth_exec` and
//! `mesh_set_smooth_faces` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Every mesh in Edit Mode (unique data) with a selected face marks its selected faces smooth
//! (`BM_ELEM_SMOOTH`). Leaving Edit Mode writes `sharp_face` only while some face is still
//! flat, so making every face smooth removes the attribute ([MESH §2.5]). The operator always
//! finishes.

use frap_bmesh::{BMesh, ElemFlag};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{edit_meshes_where, sel};

/// `mesh_set_smooth_faces`: the smooth flag of every selected face.
pub(crate) fn set_smooth_faces(bm: &mut BMesh, smooth: bool) {
    for f in bm.face_ids() {
        if bm.face(f).flag.contains(ElemFlag::SELECT) {
            bm.face_mut(f).flag.set(ElemFlag::SMOOTH, smooth);
        }
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::FacesShadeSmooth) -> OpResult {
    for me in edit_meshes_where(cx, |bm| sel(bm).2 > 0) {
        set_smooth_faces(cx.bmesh_mut(me)?, true);
    }
    Ok(OpStatus::Finished)
}
