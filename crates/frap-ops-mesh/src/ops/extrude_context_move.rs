// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_context_move(MESH_OT_extrude_context={…}, TRANSFORM_OT_translate={…})`
//! (Extrude Region and Move (context)), a macro (`MESH_OT_extrude_context_move`,
//! `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later): `mesh.extrude_context` then
//! `transform.translate`, each with its nested arguments. The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_context_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})`
//! - short form: `mesh.extrude_context_move TRANSFORM_OT_translate.value=0,0,1`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_translate` step runs with
//! `use_proportional_edit=False` and `mirror=False` forced
//! (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: two adjacent faces give the same result as `extrude_region_move` (V14 E24
//! F12).
//!
//! Golden: `tests/golden/cases/edit/extrude/macros/context_move.py.pending` (blessed; enabled by
//! renaming it to `.py` once the transform step works in Edit Mode).

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
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeContextMove) -> OpResult {
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use frap_op::pipeline::macro_step_forced;

    #[test]
    fn macro_definition_forces_step_properties() {
        let mut forced = macro_step_forced("MESH_OT_extrude_context_move", "TRANSFORM_OT_translate");
        forced.sort();
        assert_eq!(forced, vec![("mirror", false), ("use_proportional_edit", false)]);
        assert!(macro_step_forced("MESH_OT_extrude_context_move", "MESH_OT_extrude_context").is_empty());
    }
}
