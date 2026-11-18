// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_edges_indiv(use_normal_flip=False)`, tier T1
//! (`tests/golden/cases/edit/extrude/edges_*.py`). Port of `edbm_extrude_edges_exec` and
//! `EDBM_extrude_edges_indiv` (`editors/mesh/editmesh_extrude.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): every selected edge of every mesh in Edit Mode is copied and joined to
//! its copy by a quad (`bmesh.ops.extrude_edge_only`); the new edges and their vertices become
//! the selection (the side quads are not selected); the history follows the copies.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::extrude_region::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::ExtrudeEdgesIndiv) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        if common::selected_counts_of(cx, me).1 == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        common::extrude_edges_indiv(bm, p.use_normal_flip.value)?;
    }
    Ok(OpStatus::Finished)
}
