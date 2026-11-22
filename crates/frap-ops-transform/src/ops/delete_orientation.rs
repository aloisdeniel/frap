// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.delete_orientation()`, tier T1
//! (`tests/golden/cases/{object,edit}/transform_m8/orientation_*.py`, WP 8.3.5).
//!
//! Ported from blender/source/blender/editors/transform/transform_ops.cc
//! (`delete_orientation_exec`, `delete_orientation_poll`), transform_orientations.cc
//! (`BIF_removeTransformOrientationIndex`) and blenkernel/intern/scene.cc
//! (`BKE_scene_transform_orientation_remove`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Deletes the custom orientation selected as the scene orientation; the slots that used it
//! fall back to GLOBAL and those using a later custom orientation follow it. The poll fails
//! (`Operator bpy.ops.transform.delete_orientation.poll() failed, context is incorrect`)
//! unless a custom orientation is selected (`transform.select_orientation` or
//! `create_orientation(use=True)`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail};
use frap_scene::Context;
use frap_scene::id::Scene;
use frap_spec_ops_transform::transform;

/// `delete_orientation_poll`: the scene orientation is a custom one.
fn custom_selected(c: &Context) -> Result<(), PollFail> {
    let slot = &c.scene().orientation_slots[Scene::ORIENT_SLOT_DEFAULT];
    if slot.kind >= Scene::ORIENT_CUSTOM && slot.index_custom != -1 {
        return Ok(());
    }
    let hint = match c.scene().transform_orientations.first() {
        Some(t) => format!(
            "select a custom orientation first: bpy.ops.transform.select_orientation(\
             orientation={})",
            poll::py_name(&t.name)
        ),
        None => "create and select a custom orientation first: \
                 bpy.ops.transform.create_orientation(use=True)"
            .into(),
    };
    Err(PollFail {
        message: None,
        unmet: Default::default(),
        hint: Some(hint),
    })
}

/// `delete_orientation_poll`.
const CUSTOM_SELECTED: Poll = Poll {
    requires: &[],
    check: custom_selected,
};

#[operator(poll = CUSTOM_SELECTED, tier = T1)]
fn exec(cx: &mut OpCtx, _p: transform::DeleteOrientation) -> OpResult {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        let index = s.orientation_slots[Scene::ORIENT_SLOT_DEFAULT].index_custom;
        if let Ok(i) = usize::try_from(index) {
            s.transform_orientation_remove(i);
        }
    }
    Ok(OpStatus::Finished)
}
