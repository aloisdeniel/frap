// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.quaternions_flip()`, tier T0 (WP 8.3.11). Port of `pose_flip_quats_exec`
//! (blender/source/blender/editors/armature/pose_edit.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every armature in Pose Mode, each selected visible bone in `QUATERNION` rotation mode
//! gets its quaternion negated (the same orientation, the other half of the 720° range).
//! `CANCELLED` when no selected bone uses quaternions. Auto-keying is not done (deviations §2,
//! WP 8.3.11 row).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::load_all;

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: pose::QuaternionsFlip) -> OpResult {
    let mut changed_multi = false;
    for mut ch in load_all(cx, false) {
        let mut changed = false;
        for c in ch.selected_chans() {
            let pb = ch.pb_mut(c);
            if pb.rotation_mode == "QUATERNION" {
                for v in &mut pb.rotation_quaternion {
                    *v = -*v;
                }
                changed = true;
            }
        }
        if changed {
            changed_multi = true;
            ch.store(cx)?;
        }
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
