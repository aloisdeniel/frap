// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_region_move(MESH_OT_extrude_region={…}, TRANSFORM_OT_translate={…})` (Extrude Region and Move), a macro
//! (`MESH_OT_extrude_region_move`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later): `mesh.extrude_region` then `transform.translate`,
//! each with its nested arguments. The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})`
//! - short form: `mesh.extrude_region_move TRANSFORM_OT_translate.value=0,0,1`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_translate` step runs with `use_proportional_edit=False`
//! and `mirror=False` forced (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: the cube's top face moved up by 1 gives V12 E20 F10, the cap (face 6) selected and in the history; `exec` translates in `GLOBAL` space unless `orient_type` is given (the along-the-normal move is the interactive invoke).

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
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeRegionMove) -> OpResult {
    Ok(OpStatus::Finished)
}
