// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.anim.keyframe_clear_v3d(confirm=True)`, tier T0 (WP 6.4). Port of
//! `clear_anim_v3d_exec` with `can_delete_fcurve` (blender/source/blender/editors/animation/
//! keyframing.cc), Blender 5.2.4, GPL-2.0-or-later: removes every F-curve of the assigned slot
//! of each selected object (in Pose Mode only the channels of selected pose bones). The action
//! and its slot stay. Nothing removed: `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::anim;

use super::anim__keyframe_delete_v3d::v3d;
use crate::keying::remove_fcurves;

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, _p: anim::KeyframeClearV3d) -> OpResult {
    let mut changed = false;
    for o in cx.ctx().selected_objects() {
        let Some(ob) = cx.main().objects.get(o).cloned() else {
            continue;
        };
        let Some(adt) = ob.animation_data.clone() else {
            continue;
        };
        let Some(a) = adt.action else {
            continue;
        };
        let (main, log) = cx.main_mut();
        let Some(act) = main.actions.get_mut(a, log) else {
            continue;
        };
        let n = remove_fcurves(act, adt.action_slot_handle, |fc| {
            v3d::bone_allows(&ob, fc, false)
        });
        changed |= n > 0;
    }
    if !changed {
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
