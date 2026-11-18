// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.offset_edge_loops_slide(MESH_OT_offset_edge_loops={…},
//! TRANSFORM_OT_edge_slide={…})`, a macro (`MESH_OT_offset_edge_loops_slide`,
//! `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later): `mesh.offset_edge_loops` then
//! `transform.edge_slide`, whose `single_side` the macro forces to True
//! (`frap_op::pipeline::MACRO_STEP_FORCED`). The framework runs the steps (§10.2); this exec is
//! the macro's post-step hook and changes nothing.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(poll = poll::EDITMESH, tier = T1, kind = Macro)]
fn exec(_cx: &mut OpCtx, _p: mesh::OffsetEdgeLoopsSlide) -> OpResult {
    Ok(OpStatus::Finished)
}
