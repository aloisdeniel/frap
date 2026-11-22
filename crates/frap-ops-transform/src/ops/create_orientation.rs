// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.create_orientation(name='', use_view=False, use=False, overwrite=False)`,
//! tier T1 (`tests/golden/cases/{object,edit}/transform_m8/orientation_*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/transform/transform_ops.cc
//! (`create_orientation_exec`) and transform_orientations.cc (`BIF_createTransformOrientation`,
//! `createViewSpace`, `createObjectSpace`, `createMeshSpace`, `addMatrixSpace`,
//! `uniqueOrientationName`, `BIF_selectTransformOrientation`), Blender 5.2.4,
//! GPL-2.0-or-later. Poll `ED_operator_areaactive` (frap's virtual 3D View, D-10).
//!
//! The new custom orientation (a scene's `transform_spaces` entry) is
//! - with `use_view`: the 3D View's axes (normalized inverse view matrix), named
//!   `Custom View`;
//! - in mesh Edit Mode: made from the selection of the edit object (`Vertex`: the normal;
//!   `Edge`: the normal and the edge; `Face`: the faces' normal and tangent);
//! - otherwise: the active object's normalized world axes, named after the object.
//!
//! A given `name` replaces the default name; it is made unique (`Cube.001`) unless `overwrite`
//! is set, which updates the orientation of that name. `use` selects it as the scene
//! orientation (`transform.select_orientation`). Nothing to make it from (no active object, no
//! selection) is Blender's `RuntimeError: Error: Unable to create orientation`. Armature Edit
//! and Pose Mode (bone orientations) answer `NotImplementedError`. frap does not bind
//! `scene.transform_orientation_slots`; transforms read the orientations by name
//! (`orient_type`) and the selected one when `orient_type` is unset.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::ContextMode;
use frap_scene::id::Scene;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_transform::transform;
use glam::Mat3;

use crate::edit::mathx::normalize_m3;

#[operator(
    poll = poll::ALWAYS,
    tier = T1,
    support = Partial(
        "the bone orientations of armature Edit and Pose Mode are not implemented; \
         scene.transform_orientation_slots is not bound (use orient_type='<name>')"
    ),
    notes = [
        "Use the orientation by name: transform.rotate(value=…, orient_type='Face'), or pass \
         use=True and leave orient_type unset."
    ]
)]
fn exec(cx: &mut OpCtx, p: transform::CreateOrientation) -> OpResult {
    let name = p.name.value.to_string();
    let mode = cx.ctx().mode();
    let made: Option<(Mat3, String)> = if p.use_view.value {
        // `createViewSpace`: `copy_m3_m4(mat, rv3d->viewinv); normalize_m3(mat)`.
        let viewinv = cx.ctx().win.view3d.view_matrix.inverse();
        Some((
            normalize_m3(&Mat3::from_mat4(viewinv)),
            "Custom View".into(),
        ))
    } else {
        match mode {
            ContextMode::EditMesh => {
                let (c, edit) = cx.split_edit();
                c.active_object().and_then(|a| {
                    let me = c.main.objects.get(a)?.data?.typed::<frap_rna::MeshId>()?;
                    let em = edit.meshes.get(&me.key())?;
                    crate::edit::orient::mesh_space(&em.bm, &world_matrix_in(c.main, a))
                        .map(|(m, n)| (m, n.to_string()))
                })
            }
            ContextMode::EditArmature | ContextMode::Pose => {
                return Err(OpError::NotImplemented(
                    "frap: transform.create_orientation from bones (armature Edit or Pose Mode) \
                     is not implemented yet; create it in Object Mode from the armature or with \
                     use_view=True"
                        .into(),
                ));
            }
            _ => {
                // `createObjectSpace`: the active base, selected or not.
                let c = cx.ctx();
                c.active_object().and_then(|a| {
                    let o = c.main.objects.get(a)?;
                    let m = normalize_m3(&Mat3::from_mat4(world_matrix_in(c.main, a)));
                    Some((m, o.header.name.to_string()))
                })
            }
        }
    };
    let Some((matrix, default_name)) = made else {
        return Err(OpError::Runtime("Unable to create orientation".into()));
    };
    let name = if name.is_empty() { default_name } else { name };
    let use_it = p.r#use.value;
    let overwrite = p.overwrite.value;
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    let s = main
        .scenes
        .get_mut(scene, log)
        .ok_or_else(|| OpError::Runtime("Unable to create orientation".into()))?;
    let index = s.transform_orientation_add(&name, matrix, overwrite);
    if use_it {
        // `BIF_selectTransformOrientation`.
        s.orientation_slot_set_index(
            Scene::ORIENT_SLOT_DEFAULT,
            Scene::ORIENT_CUSTOM + index as i32,
        );
    }
    Ok(OpStatus::Finished)
}
