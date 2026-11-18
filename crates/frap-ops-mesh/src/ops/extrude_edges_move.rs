// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_edges_move(MESH_OT_extrude_edges_indiv={…}, TRANSFORM_OT_translate={…})`
//! (Extrude Only Edges and Move), a macro (`MESH_OT_extrude_edges_move`,
//! `editors/mesh/mesh_ops.cc`, Blender 5.2.4, GPL-2.0-or-later): `mesh.extrude_edges_indiv` then
//! `transform.translate`, each with its nested arguments. The framework runs the steps (§10.2,
//! `frap_op::pipeline::execute`); this exec is the macro's post-step hook and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_edges_move(TRANSFORM_OT_translate={"value": (0, 0, 1)})`
//! - short form: `mesh.extrude_edges_move TRANSFORM_OT_translate.value=0,0,1`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_translate` step runs with
//! `use_proportional_edit=False` and `mirror=False` forced
//! (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: one cube edge gives V10 E15 F7, the new edge selected and in the history.
//!
//! Golden: `tests/golden/cases/edit/extrude/macros/edges_move.py`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    kind = Macro,
)]
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeEdgesMove) -> OpResult {
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use frap_op::pipeline::macro_step_forced;

    #[test]
    fn macro_definition_forces_step_properties() {
        let mut forced = macro_step_forced("MESH_OT_extrude_edges_move", "TRANSFORM_OT_translate");
        forced.sort();
        assert_eq!(forced, vec![("mirror", false), ("use_proportional_edit", false)]);
        assert!(macro_step_forced("MESH_OT_extrude_edges_move", "MESH_OT_extrude_edges_indiv").is_empty());
    }
}
