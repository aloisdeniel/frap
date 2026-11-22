// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.uvs_rotate(use_ccw=False)`, tier T0
//! (`tests/golden/cases/edit/normals_m8/uvs_colors*.py`). Port of `edbm_rotate_uvs_exec`
//! (`editors/mesh/editmesh_tools.cc`) and `bmo_rotate_uvs_exec` (`bmesh/operators/
//! bmo_utils.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data) with a selected face, the active UV map's values
//! move one corner along each selected face: clockwise every corner takes the UV of the corner
//! before it (the first that of the last), with `use_ccw` the UV of the corner after it. A mesh
//! without a UV map is left alone. The operator always finishes.
//!
//! [`cycle`] holds the corner cycling the four `uvs_*`/`colors_*` operators share.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::{edit_meshes_where, sel};

pub(crate) mod cycle;

#[operator(poll = poll::EDITMESH, tier = T0)]
fn exec(cx: &mut OpCtx, p: mesh::UvsRotate) -> OpResult {
    let op = cycle::Cycle::Rotate {
        use_ccw: p.use_ccw.value,
    };
    for me in edit_meshes_where(cx, |bm| sel(bm).2 > 0) {
        let bm = cx.bmesh_mut(me)?;
        if let Some(uv) = cycle::active_uv(bm) {
            cycle::apply(bm, uv, op);
        }
    }
    Ok(OpStatus::Finished)
}
