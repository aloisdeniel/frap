// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.select_parent()`, tier T0 (WP 8.3.11). Port of `pose_select_parent_exec`
//! (blender/source/blender/editors/armature/pose_select.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On the pose object only: the parent of the active pose bone (`context.active_pose_bone`)
//! gets selected (with head and tail) and the active bone stays where it is, as in Blender
//! (`arm->act_bone = bone`, the active bone itself). `CANCELLED` without an active pose bone,
//! without a parent, with a parent hidden in Pose Mode, or when the **active** bone is
//! `hide_select` (Blender tests the active bone's flag, not the parent's).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::ArmatureId;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::Chans;

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: pose::SelectParent) -> OpResult {
    let found = {
        let c = cx.ctx();
        let active = c.active_pose_bone();
        let arm = c
            .pose_object()
            .and_then(|ob| c.main.objects.get(ob))
            .and_then(|o| o.data?.typed::<ArmatureId>());
        match (c.pose_object(), arm, active) {
            (Some(ob), Some(arm), Some(b)) => Some((ob, arm, b.bone)),
            _ => None,
        }
    };
    let Some((ob, arm, name)) = found else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(mut ch) = Chans::load(cx, ob, arm) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(c) = ch.find(&name) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(parent) = ch.parent(c) else {
        return Ok(OpStatus::Cancelled);
    };
    if ch.pb(parent).hide || ch.unselectable(c) {
        return Ok(OpStatus::Cancelled);
    }
    ch.select(parent);
    ch.set_active(Some(c));
    ch.store(cx)?;
    Ok(OpStatus::Finished)
}
