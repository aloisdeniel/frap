// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.hide(unselected=False)`, tier T0 (WP 8.3.11). Port of `pose_hide_exec`
//! (blender/source/blender/editors/armature/pose_edit.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every armature in Pose Mode (unique data): the bones in a visible bone collection whose
//! `PoseBone.select` is set (`unselected=True`: not set) get `PoseBone.hide` and lose their
//! selection (with head and tail). An already hidden bone is matched by its flag too.
//! `CANCELLED` when no bone was hidden.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::load_all;

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_HIDE)]
fn exec(cx: &mut OpCtx, p: pose::PoseHide) -> OpResult {
    let hide_selected = !p.unselected.value;
    let mut changed_multi = false;
    for mut ch in load_all(cx, true) {
        let mut changed = false;
        for c in 0..ch.len() {
            if !ch.in_visible_collection(c) {
                continue;
            }
            if ch.pb(c).select != hide_selected {
                continue;
            }
            ch.pb_mut(c).hide = true;
            ch.deselect(c);
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
