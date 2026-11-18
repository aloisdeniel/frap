// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.duplicate_move(MESH_OT_duplicate={…}, TRANSFORM_OT_translate={…})`, a macro
//! (`MESH_OT_duplicate_move`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! `mesh.duplicate` then `transform.translate` with their nested arguments (bpy form
//! `TRANSFORM_OT_translate={'value': (0, 0, 1)}`, short form
//! `TRANSFORM_OT_translate.value=0,0,1`). The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//! The copies are appended and selected, the originals deselected, then the selection is
//! moved ([MESH §4.11]). Without a selection `mesh.duplicate` cancels, which cancels the
//! macro. The translation in Edit Mode is the edit-mode transform handler (WP 2.12).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(poll = poll::EDITMESH, tier = T1, kind = Macro)]
fn exec(_cx: &mut OpCtx, _p: mesh::DuplicateMove) -> OpResult {
    Ok(OpStatus::Finished)
}
