// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_faces_move(MESH_OT_extrude_faces_indiv={…},
//! TRANSFORM_OT_shrink_fatten={…})` (Extrude Individual Faces and Move), a macro
//! (`MESH_OT_extrude_faces_move`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! `mesh.extrude_faces_indiv` then `transform.shrink_fatten` (each face along its own normal: the
//! copies' vertex normals are their face normal), each with its nested arguments. The framework
//! runs the steps (§10.2, `frap_op::pipeline::execute`); this exec is the macro's post-step hook
//! and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_faces_move(TRANSFORM_OT_shrink_fatten={"value": 0.5})`
//! - short form: `mesh.extrude_faces_move TRANSFORM_OT_shrink_fatten.value=0.5`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_shrink_fatten` step runs with
//! `use_proportional_edit=False` and `mirror=False` forced
//! (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: two adjacent cube faces give V16 E28 F14, the copies selected, the history
//! cleared.
//!
//! Golden: `tests/golden/cases/edit/extrude/macros/faces_move.py.pending` (blessed; enabled by
//! renaming it to `.py` once the transform step works in Edit Mode).

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
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeFacesMove) -> OpResult {
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use frap_op::pipeline::macro_step_forced;

    #[test]
    fn macro_definition_forces_step_properties() {
        let mut forced = macro_step_forced("MESH_OT_extrude_faces_move", "TRANSFORM_OT_shrink_fatten");
        forced.sort();
        assert_eq!(forced, vec![("mirror", false), ("use_proportional_edit", false)]);
        assert!(macro_step_forced("MESH_OT_extrude_faces_move", "MESH_OT_extrude_faces_indiv").is_empty());
    }
}
