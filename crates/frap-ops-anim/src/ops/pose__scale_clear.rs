// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.scale_clear()`, tier T0 (WP 6.2). Port of `pose_clear_transform_generic_exec` and
//! `pchan_clear_*` (blender/source/blender/editors/armature/pose_transform.cc), Blender 5.2.4,
//! GPL-2.0-or-later: resets the scale of the selected, visible pose bones of every armature
//! in Pose Mode to their defaults on the unlocked components (`pose__loc_clear.rs`, `clear`).
//! `CANCELLED` when no bone is selected.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__loc_clear::clear;

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: pose::ScaleClear) -> OpResult {
    clear::exec(
        cx,
        clear::What {
            loc: false,
            rot: false,
            scale: true,
        },
    )
}
