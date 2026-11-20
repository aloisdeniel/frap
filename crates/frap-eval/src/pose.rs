// SPDX-License-Identifier: GPL-3.0-or-later
//! Pose matrices (WP 6.2): `BKE_pose_where_is` for an armature object — each pose channel's
//! armature-space matrix from its rest matrix, its local transform (`location`,
//! `rotation_mode` rotation, `scale`) and its parent's, with `use_connect`,
//! `use_inherit_rotation`, `inherit_scale` and `use_local_location`; `pose_position='REST'`
//! gives the rest pose; Edit Mode keeps the pose of the stored (pre-edit) bones and the
//! current channels, as Blender's pose evaluation skips armatures with edit bones. The ARMATURE modifier (WP 6.5) and glTF skins (WP 6.8)
//! read the result through `EvaluatedObject::pose`.
//!
//! The maths are `frap_scene::edit_armature::{rest, posing}` (shared with the `PoseBone` RNA
//! hooks and the pose-mode transforms). A pose that was never built (an armature object made
//! with `bpy.data.objects.new`) evaluates as `BKE_pose_ensure` would build it: one default
//! channel per bone.

use std::sync::Arc;

use frap_rna::ArmatureId;
use frap_scene::edit_armature::posing::where_is;
use frap_scene::edit_armature::rest::rest_pose;
use frap_scene::id::Object;
use frap_scene::{EvaluatedPose, HostRead};

/// The evaluated pose of `ob` (`None` for objects that are not armatures).
pub(crate) fn evaluate(host: &dyn HostRead, ob: &Object) -> Option<Arc<EvaluatedPose>> {
    let arm_ref = ob.data?.typed::<ArmatureId>()?;
    let arm = host.main().armatures.get(arm_ref)?;
    let rest = rest_pose(arm);
    // Edit Mode keeps the pose of the stored bones (`BKE_pose_eval_*` return early while
    // `arm->edbo` is set, armature_update.cc), so only `pose_position` selects the rest pose.
    let rest_position = arm.pose_position == "REST";
    let mats = where_is(arm, &rest, ob.pose.as_ref(), rest_position);
    Some(Arc::new(EvaluatedPose {
        bones: arm
            .bones
            .iter()
            .zip(mats)
            .map(|(b, m)| (b.name.clone(), m.pose_mat))
            .collect(),
    }))
}
