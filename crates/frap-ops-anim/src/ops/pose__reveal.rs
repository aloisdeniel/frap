// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.reveal(select=True)`, tier T0 (WP 8.3.11). Port of `pose_reveal_exec`
//! (blender/source/blender/editors/armature/pose_edit.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every armature in Pose Mode (unique data): every bone hidden in Pose Mode
//! (`PoseBone.hide`) that is in a visible bone collection is shown again and, unless it is
//! `hide_select`, gets `PoseBone.select = select` (only the body flag `POSE_SELECTED`, head and
//! tail are left as they are). `CANCELLED` when nothing was hidden.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::load_all;

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_REVEAL)]
fn exec(cx: &mut OpCtx, p: pose::PoseReveal) -> OpResult {
    let select = p.select.value;
    let mut changed_multi = false;
    for mut ch in load_all(cx, true) {
        let mut changed = false;
        for c in 0..ch.len() {
            if ch.bone(c).is_none() || !ch.in_visible_collection(c) {
                continue;
            }
            if !ch.pb(c).hide {
                continue;
            }
            let unselectable = ch.unselectable(c);
            let pb = ch.pb_mut(c);
            if !unselectable {
                pb.select = select;
            }
            pb.hide = false;
            changed = true;
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
