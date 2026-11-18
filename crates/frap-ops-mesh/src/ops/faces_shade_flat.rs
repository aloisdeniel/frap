// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.faces_shade_flat()`, tier T1 (`tests/golden/cases/edit/normals/shade_*.py`).
//! Port of `edbm_faces_shade_flat_exec` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Every mesh in Edit Mode (unique data) with a selected face marks its selected faces flat;
//! leaving Edit Mode then writes `sharp_face` (true on those faces). The operator always
//! finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::faces_shade_smooth::set_smooth_faces;
use super::flip_normals::{edit_meshes_where, sel};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::FacesShadeFlat) -> OpResult {
    for me in edit_meshes_where(cx, |bm| sel(bm).2 > 0) {
        set_smooth_faces(cx.bmesh_mut(me)?, false);
    }
    Ok(OpStatus::Finished)
}
