// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_manifold(MESH_OT_extrude_region={…}, TRANSFORM_OT_translate={…})` (Extrude
//! Manifold), a macro (`MESH_OT_extrude_manifold`, `editors/mesh/mesh_ops.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `mesh.extrude_region` with `use_dissolve_ortho_edges=True` forced, then
//! `transform.translate` with `use_automerge_and_split=True` forced, each with its nested
//! arguments. The framework runs the steps (§10.2, `frap_op::pipeline::execute`); this exec is the
//! macro's post-step hook and changes nothing.
//!
//! Both spellings name the steps by their `bl_idname`:
//! - bpy: `bpy.ops.mesh.extrude_manifold(TRANSFORM_OT_translate={"value": (0, 0, -0.5)})`
//! - short form: `mesh.extrude_manifold TRANSFORM_OT_translate.value=0,0,-0.5`
//!
//! As Blender's macro definition does, the `TRANSFORM_OT_translate` step runs with
//! `use_proportional_edit=False` and `mirror=False` forced
//! (`frap_op::pipeline::MACRO_STEP_FORCED`).
//!
//! Verified on 5.2.4: pushing the cube's top face inward only moves it (V8 E12 F6): the side faces
//! are folded into the walls.
//!
//! Golden: `tests/golden/cases/edit/extrude/macros/manifold.py.pending` (blessed; enabled by
//! renaming it to `.py` once edit-mode transforms implement `use_automerge_and_split`,
//! `EDBM_automerge_and_split`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    kind = Macro,
    support = Partial(
        "the TRANSFORM_OT_translate step forces use_automerge_and_split=True, which edit-mode \
         transforms do not implement yet (EDBM_automerge_and_split): the macro raises \
         OP_NOT_IMPLEMENTED and nothing changes"
    ),
)]
fn exec(_cx: &mut OpCtx, _p: mesh::ExtrudeManifold) -> OpResult {
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use frap_op::pipeline::macro_step_forced;

    #[test]
    fn macro_definition_forces_step_properties() {
        let mut forced = macro_step_forced("MESH_OT_extrude_manifold", "TRANSFORM_OT_translate");
        forced.sort();
        assert_eq!(
            forced,
            vec![
                ("mirror", false),
                ("use_automerge_and_split", true),
                ("use_proportional_edit", false)
            ]
        );
        assert_eq!(
            macro_step_forced("MESH_OT_extrude_manifold", "MESH_OT_extrude_region"),
            vec![("use_dissolve_ortho_edges", true)]
        );
    }
}
