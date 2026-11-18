// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_region_shrink_fatten(MESH_OT_extrude_region={…}, TRANSFORM_OT_shrink_fatten={…})` (Extrude Region and Shrink/Fatten), a macro
//! (`MESH_OT_extrude_region_shrink_fatten`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later): `mesh.extrude_region` then `transform.shrink_fatten` (along the vertex normals),
//! each with its nested arguments. The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_region_shrink_fatten(TRANSFORM_OT_shrink_fatten={"value": 0.5})`
//! - short form: `mesh.extrude_region_shrink_fatten TRANSFORM_OT_shrink_fatten.value=0.5`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_shrink_fatten` step runs with `use_proportional_edit=False`
//! and `mirror=False` forced (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: the cube's top face with `value=0.5` ends at z = 1.5.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    kind = Macro,
    support = Partial(
        "the TRANSFORM_OT_shrink_fatten step needs `transform.shrink_fatten` in Edit Mode (edit-mode transforms, WP 2.12): \
         until then the macro raises OP_NOT_IMPLEMENTED and nothing changes"
    ),
)]
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeRegionShrinkFatten) -> OpResult {
    Ok(OpStatus::Finished)
}
