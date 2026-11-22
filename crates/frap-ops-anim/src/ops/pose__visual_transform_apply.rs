// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.visual_transform_apply()`, tier T1 (WP 8.3.11). Port of
//! `pose_visual_transform_apply_exec` (blender/source/blender/editors/armature/pose_transform.cc)
//! with `BKE_armature_mat_pose_to_bone` and `BKE_pchan_apply_mat4`
//! (blenkernel/intern/armature.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every armature object in Pose Mode, each selected visible bone gets the local transform
//! (location, rotation in its own rotation mode, Euler-compatible with the old value, scale)
//! that reproduces its evaluated armature-space matrix `PoseBone.matrix` under its parent's
//! evaluated matrix. All matrices are taken before any channel changes, so parents and children
//! applied together do not feed into each other. frap has no constraints or IK, so the visual
//! transform is the bone's own transform up to the float round trip (`Armature.pose_position
//! = 'REST'` gives the rest transform). Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::posing::{pose_to_bone, where_is};
use frap_scene::edit_armature::rest::rest_pose;
use frap_spec_ops_anim::pose;

use super::pose__select_hierarchy::chans::load_all;

#[operator(poll = poll::POSEMODE, tier = T1)]
fn exec(cx: &mut OpCtx, _p: pose::VisualTransformApply) -> OpResult {
    for mut ch in load_all(cx, false) {
        let rest = rest_pose(&ch.arm);
        let mats = where_is(
            &ch.arm,
            &rest,
            Some(&ch.pose),
            ch.arm.pose_position == "REST",
        );
        // `pchan_xform_array`: every matrix first, then the writes.
        let mut xforms = Vec::new();
        for c in 0..ch.len() {
            let Some(b) = ch.bone(c) else {
                continue;
            };
            if !ch.selected(c) {
                continue;
            }
            let parent = ch.arm.bones[b]
                .parent
                .and_then(|p| mats.get(p))
                .map(|m| m.pose_mat);
            let m = pose_to_bone(&ch.arm, &rest, b, parent.as_ref(), &mats[b].pose_mat);
            xforms.push((c, m));
        }
        if xforms.is_empty() {
            continue;
        }
        for (c, m) in xforms {
            let pb = ch.pb_mut(c);
            let mut chans = pb.channels();
            chans.apply_mat4(&m, true);
            pb.set_channels(&chans);
        }
        ch.store(cx)?;
    }
    Ok(OpStatus::Finished)
}
