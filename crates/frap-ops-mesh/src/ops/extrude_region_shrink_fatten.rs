// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_region_shrink_fatten(MESH_OT_extrude_region={…},
//! TRANSFORM_OT_shrink_fatten={…})` (Extrude Region and Shrink/Fatten), a macro
//! (`MESH_OT_extrude_region_shrink_fatten`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `mesh.extrude_region` then `transform.shrink_fatten` (along the vertex
//! normals), each with its nested arguments. The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_region_shrink_fatten(TRANSFORM_OT_shrink_fatten={"value": 0.5})`
//! - short form: `mesh.extrude_region_shrink_fatten TRANSFORM_OT_shrink_fatten.value=0.5`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_shrink_fatten` step runs with
//! `use_proportional_edit=False` and `mirror=False` forced
//! (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: the cube's top face with `value=0.5` ends at z = 1.5.
//!
//! Golden: `tests/golden/cases/edit/extrude/macros/region_shrink_fatten.py`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    kind = Macro,
)]
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeRegionShrinkFatten) -> OpResult {
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use frap_op::pipeline::macro_step_forced;

    #[test]
    fn macro_definition_forces_step_properties() {
        let mut forced = macro_step_forced("MESH_OT_extrude_region_shrink_fatten", "TRANSFORM_OT_shrink_fatten");
        forced.sort();
        assert_eq!(forced, vec![("mirror", false), ("use_proportional_edit", false)]);
        assert!(macro_step_forced("MESH_OT_extrude_region_shrink_fatten", "MESH_OT_extrude_region").is_empty());
    }
}
