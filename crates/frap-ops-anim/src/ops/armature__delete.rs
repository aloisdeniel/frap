// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.delete(confirm=True)`, tier T0 (WP 6.2). Port of
//! `armature_delete_selected_exec` (blender/source/blender/editors/armature/armature_edit.cc),
//! Blender 5.2.4, GPL-2.0-or-later. `confirm` only matters for `invoke`.
//!
//! The selected, visible bones (plus their X mirrors with `use_mirror_x`) are removed with
//! their pose channels; their children move to the removed bone's parent, disconnected; the
//! selection is re-synced. `CANCELLED` without a selected bone.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0, spec = armature::OPSPEC_ARMATURE_OT_DELETE)]
fn exec(cx: &mut OpCtx, _p: armature::ArmatureDelete) -> OpResult {
    if common::selected_count(cx) == 0 {
        return Ok(OpStatus::Cancelled);
    }
    let mut changed_multi = false;
    for (ob, arm) in common::edit_armatures(cx) {
        let Some((a, mut ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        let doomed = ea.bones_to_delete(&a);
        let names: Vec<String> = doomed
            .iter()
            .map(|&i| ea.edit_bones[i].name.to_string())
            .collect();
        if !doomed.is_empty() {
            ea.delete_bones(&doomed);
            changed_multi = true;
        }
        common::store(cx, arm, ea);
        // `BKE_pose_channels_remove`: the channels of the deleted bones.
        if !names.is_empty() && cx.main().objects.get(ob).is_some_and(|o| o.pose.is_some()) {
            let o = cx.object_mut(ob)?;
            if let Some(pose) = o.pose.as_mut() {
                pose.bones
                    .retain(|pb| !names.iter().any(|n| pb.name == n.as_str()));
            }
        }
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
