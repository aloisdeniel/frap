// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_faces_indiv()`, tier T1 (`tests/golden/cases/edit/extrude/faces_*.py`).
//! Port of `edbm_extrude_faces_exec` and `edbm_extrude_discrete_faces`
//! (`editors/mesh/editmesh_extrude.cc`, Blender 5.2.4, GPL-2.0-or-later): every selected face
//! of every mesh in Edit Mode is extruded on its own (`bmesh.ops.extrude_discrete_faces`); the
//! copies become the selection, the history is cleared, and the copies' vertex normals are set
//! to their face normal (what `mesh.extrude_faces_move`'s shrink/fatten step moves along).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::extrude_region::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::ExtrudeFacesIndiv) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if common::selected_counts_of(cx, me).2 == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        common::extrude_discrete(bm)?;
    }
    Ok(OpStatus::Finished)
}
