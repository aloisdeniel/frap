// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.rip_edge_move(MESH_OT_rip_edge={…}, TRANSFORM_OT_translate={…})`, a macro
//! (`MESH_OT_rip_edge_move`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! `mesh.rip_edge` then `transform.translate` with their nested arguments, tier T1
//! (`tests/golden/cases/edit/tools_m8/rip_edge.py`). The framework runs the steps (§10.2);
//! this exec is the macro's post-step hook and changes nothing. Called with `exec`, the ray of
//! `rip_edge` comes from its `location` and `direction` arguments (the mouse ray only
//! `invoke` fills in), and the extended vertices are moved by the translation.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(poll = poll::EDITMESH_VIEW3D, tier = T1, kind = Macro)]
fn exec(_cx: &mut OpCtx, _p: mesh::RipEdgeMove) -> OpResult {
    Ok(OpStatus::Finished)
}
