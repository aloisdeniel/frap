// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.uvs_reverse()`, tier T0
//! (`tests/golden/cases/edit/normals_m8/uvs_colors*.py`). Port of `edbm_reverse_uvs_exec`
//! (`editors/mesh/editmesh_tools.cc`) and `bm_face_reverse_uvs` / `bmo_reverse_uvs_exec`
//! (`bmesh/operators/bmo_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data) with a selected face, the active UV map's values of
//! each selected face are reversed: corner `i` takes the UV of corner `len - 1 - i` (the first
//! corner takes the last one's). A mesh without a UV map is left alone. The operator always
//! finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{edit_meshes_where, sel};
use super::uvs_rotate::cycle;

#[operator(poll = poll::EDITMESH, tier = T0)]
fn exec(cx: &mut OpCtx, _p: mesh::UvsReverse) -> OpResult {
    for me in edit_meshes_where(cx, |bm| sel(bm).2 > 0) {
        let bm = cx.bmesh_mut(me)?;
        if let Some(uv) = cycle::active_uv(bm) {
            cycle::apply(bm, uv, cycle::Cycle::Reverse);
        }
    }
    Ok(OpStatus::Finished)
}
