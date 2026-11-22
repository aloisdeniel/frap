// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.separate()`, tier T0 (WP 8.3.10). Port of `separate_armature_exec`
//! (blender/source/blender/editors/armature/armature_relations.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`frap_scene::edit_armature::separate`).
//!
//! For every armature object in Edit Mode with a selected visible bone: the selected bones move
//! to a new armature object (a duplicate, `Armature.001` with data `Armature.001` and copied
//! actions, in the original's collections, with its transform, base selection and the moved
//! bones' pose channels, in Object Mode); objects bone-parented to a moved bone follow it; the
//! original keeps the other bones and re-enters Edit Mode with nothing selected. An armature
//! with only joints selected is just deselected. Reports INFO `Separated bones` when anything
//! moved; always `FINISHED`.

use std::sync::Arc;

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::separate;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SEPARATE
)]
fn exec(cx: &mut OpCtx, _p: armature::Separate) -> OpResult {
    let scene = cx.ctx().win.scene;
    let mut ok = false;
    for (ob, arm) in common::edit_armatures(cx) {
        let Some((a, mut ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        let (has_bone, has_any) = separate::selection_state(&ea, &a);
        if !has_bone {
            if has_any {
                ea.deselect_all();
                common::store(cx, arm, ea);
            }
            continue;
        }
        cx.split_edit().1.armatures.remove(&arm.key());
        let back = {
            let (main, log) = cx.main_mut();
            separate::separate(main, log, scene, ob, arm, &ea)
        };
        let back = back.map(|(_, b)| b).unwrap_or(ea);
        cx.split_edit().1.armatures.insert(arm.key(), Arc::new(back));
        cx.main_mut().1.modified.insert(arm.any());
        ok = true;
    }
    if ok {
        cx.report(ReportType::Info, "Separated bones");
    }
    Ok(OpStatus::Finished)
}
