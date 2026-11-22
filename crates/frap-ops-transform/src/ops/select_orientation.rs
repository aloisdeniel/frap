// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.select_orientation(orientation='GLOBAL')`, tier T1
//! (`tests/golden/cases/{object,edit}/transform_m8/orientation_*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/transform/transform_ops.cc
//! (`select_orientation_exec`) and blenkernel/intern/scene.cc
//! (`BKE_scene_orientation_slot_set_index`), Blender 5.2.4, GPL-2.0-or-later. Poll
//! `ED_operator_view3d_active` (frap's virtual 3D View, D-10).
//!
//! Sets the scene's transform orientation (`scene.transform_orientation_slots[0]`): one of the
//! seven built-in orientations or a custom orientation by name
//! (`rna_TransformOrientation_itemf`; an unknown name is the conversion `TypeError` listing
//! them). Transforms whose `orient_type` is unset then use it.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::Scene;
use frap_spec_ops_transform::transform;

use crate::core::orient::{OrientType, SceneOrientations};

#[operator(
    poll = poll::ALWAYS,
    tier = T1,
    support = Partial(
        "scene.transform_orientation_slots is not bound: the selected orientation is seen by \
         the transforms (orient_type unset) only"
    ),
    dyn_enums = [("orientation", crate::core::orient::orient_type_items)]
)]
fn exec(cx: &mut OpCtx, p: transform::SelectOrientation) -> OpResult {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    let Some(s) = main.scenes.get_mut(scene, log) else {
        return Ok(OpStatus::Cancelled);
    };
    let value = OrientType::resolve(&p.orientation.value, &SceneOrientations::of(s)).value();
    s.orientation_slot_set_index(Scene::ORIENT_SLOT_DEFAULT, value);
    Ok(OpStatus::Finished)
}
