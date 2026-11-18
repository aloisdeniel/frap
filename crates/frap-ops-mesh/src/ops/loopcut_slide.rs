// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.loopcut_slide(MESH_OT_loopcut={…}, TRANSFORM_OT_edge_slide={…})`, a macro
//! (`MESH_OT_loopcut_slide`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! `mesh.loopcut` then `transform.edge_slide` with their nested arguments (bpy form
//! `MESH_OT_loopcut={'edge_index': 0, 'object_index': 0}`, short form
//! `MESH_OT_loopcut.edge_index=0`). The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//!
//! As a macro step, `mesh.loopcut` with several cuts in vertex select mode switches every edit
//! mesh to edge select mode (edge slide cannot slide several loops in vertex mode); headless,
//! the loop is picked with `MESH_OT_loopcut={'edge_index': …, 'object_index': …}`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    kind = Macro,
    notes = ["headless: pick the ring with MESH_OT_loopcut={'edge_index': …, 'object_index': …}"]
)]
fn exec(_cx: &mut OpCtx, _p: mesh::LoopcutSlide) -> OpResult {
    Ok(OpStatus::Finished)
}
