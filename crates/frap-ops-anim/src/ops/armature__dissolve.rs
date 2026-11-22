// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.dissolve()`, tier T0 (WP 8.3.10). Port of
//! `armature_dissolve_selected_exec` and `armature_dissolve_ebone_cb`
//! (blender/source/blender/editors/armature/armature_edit.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::dissolve`).
//!
//! On every armature in Edit Mode, a visible bone with a selected tail (or body) whose only
//! connected child has a selected head (or body) absorbs that child: it takes the child's tail,
//! tail radius and tail selection, the child is removed with its pose channel, and the child's
//! children move up (staying connected, their head radius following). With
//! `Armature.use_mirror_x` the X mirrors are dissolved too. `CANCELLED` when nothing was
//! dissolved.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_DISSOLVE
)]
fn exec(cx: &mut OpCtx, _p: armature::Dissolve) -> OpResult {
    let mut changed_multi = false;
    for (ob, arm) in common::edit_armatures(cx) {
        let Some((a, mut ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        let (names, changed) = ea.dissolve(&a);
        common::store(cx, arm, ea);
        // `BKE_pose_channels_remove(obedit, armature_dissolve_ebone_cb, arm)`.
        if !names.is_empty() && cx.main().objects.get(ob).is_some_and(|o| o.pose.is_some()) {
            let o = cx.object_mut(ob)?;
            if let Some(pose) = o.pose.as_mut() {
                pose.bones.retain(|pb| !names.contains(&pb.name));
            }
        }
        changed_multi |= changed;
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
