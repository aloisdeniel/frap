// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.rotation_mode_set(type='QUATERNION')`, tier T0 (WP 6.2). Port of
//! `pose_bone_rotmode_exec` (blender/source/blender/editors/armature/pose_edit.cc), Blender
//! 5.2.4, GPL-2.0-or-later: every bone of `context.selected_pose_bones` switches rotation mode,
//! converting its stored rotation (`BKE_rotMode_change_values`, as the `PoseBone.rotation_mode`
//! setter does). Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__select_all::common;

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: pose::RotationModeSet) -> OpResult {
    let mode = p.r#type.value.identifier();
    for (ob, arm) in common::pose_objects(cx) {
        let names = common::channels(cx, ob, arm, true);
        if names.is_empty() {
            continue;
        }
        let o = cx.object_mut(ob)?;
        if let Some(pose) = o.pose.as_mut() {
            for n in &names {
                if let Some(pb) = pose.bone_mut(n) {
                    pb.set_rotation_mode(mode);
                }
            }
        }
    }
    Ok(OpStatus::Finished)
}
