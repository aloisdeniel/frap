// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_verts_indiv()`, tier T1 (`tests/golden/cases/edit/extrude/verts_*.py`).
//! Port of `edbm_extrude_verts_exec` (`editors/mesh/editmesh_extrude.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): every selected vertex of every mesh in Edit Mode gets a copy joined to
//! it by a wire edge (`bmesh.ops.extrude_vert_indiv`); the originals are deselected (the
//! selection is flushed) and the copies selected; the selection history follows the copies.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::extrude_region::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::ExtrudeVertsIndiv) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if !common::has_selected_vert(cx, me) {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        common::extrude_verts_indiv(bm)?;
    }
    Ok(OpStatus::Finished)
}
