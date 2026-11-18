// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_vertices_move(MESH_OT_extrude_verts_indiv={…}, TRANSFORM_OT_translate={…})` (Extrude Only Vertices and Move), a macro
//! (`MESH_OT_extrude_vertices_move`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later): `mesh.extrude_verts_indiv` then `transform.translate`,
//! each with its nested arguments. The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_vertices_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})`
//! - short form: `mesh.extrude_vertices_move TRANSFORM_OT_translate.value=0,0,1`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_translate` step runs with `use_proportional_edit=False`
//! and `mirror=False` forced (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: one cube vertex gives V9 E13 F6, the new vertex selected and in the history.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    kind = Macro,
    support = Partial(
        "the TRANSFORM_OT_translate step needs `transform.translate` in Edit Mode (edit-mode transforms, WP 2.12): \
         until then the macro raises OP_NOT_IMPLEMENTED and nothing changes"
    ),
)]
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeVerticesMove) -> OpResult {
    Ok(OpStatus::Finished)
}
