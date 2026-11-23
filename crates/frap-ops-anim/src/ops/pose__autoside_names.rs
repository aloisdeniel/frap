// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.autoside_names(axis='XAXIS')`, tier T0 (WP 8.3.11). Port of
//! `pose_autoside_names_exec` (blender/source/blender/editors/armature/pose_edit.cc) and
//! `bone_autoside_name` (blenkernel/intern/armature.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Each selected visible pose bone (`context.selected_pose_bones`) is renamed with the side
//! extension of the axis it lies on: `.L`/`.R` (X), `.Fr`/`.Bk` (Y), `.Top`/`.Bot` (Z), after
//! stripping existing extensions. Pose Mode reads the rest bone's `head`/`tail` **relative to
//! its parent** (`Bone.head`, `Bone.tail`; armature space for a root bone), not the
//! armature-space ends Edit Mode uses (Blender's code, kept). Renames go through
//! `ED_armature_bone_rename`. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{ArmatureId, IdRef, ObjectId};
use frap_scene::edit_armature::names::bone_autoside_name;
use frap_scene::edit_armature::rest::rest_pose;
use frap_spec_ops_anim::pose::{self, autoside_names::Axis};

use super::armature__flip_names::rename_cx;
use super::pose__select_hierarchy::chans::load_all;

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_AUTOSIDE_NAMES)]
fn exec(cx: &mut OpCtx, p: pose::PoseAutosideNames) -> OpResult {
    let axis = match p.axis.value {
        Axis::Xaxis => 0,
        Axis::Yaxis => 1,
        Axis::Zaxis => 2,
    };
    // `CTX_DATA_BEGIN (selected_pose_bones)`: the list is taken before any rename.
    let targets: Vec<(IdRef<ObjectId>, IdRef<ArmatureId>, usize)> = load_all(cx, false)
        .iter()
        .flat_map(|ch| {
            ch.selected_chans()
                .into_iter()
                .map(|c| (ch.ob, ch.arm_ref, c))
        })
        .collect();
    for (ob, arm, c) in targets {
        let found = {
            let main = cx.main();
            let name = main
                .objects
                .get(ob)
                .and_then(|o| o.pose.as_ref())
                .and_then(|p| p.bones.get(c))
                .map(|pb| pb.name.clone());
            let a = main.armatures.get(arm);
            match (name, a) {
                (Some(name), Some(a)) => a.bone_index(&name).map(|b| {
                    let r = rest_pose(a)[b];
                    (name, r.head[axis], r.tail[axis])
                }),
                _ => None,
            }
        };
        let Some((name, head, tail)) = found else {
            continue;
        };
        if let Some(new) = bone_autoside_name(&name, axis, head, tail) {
            rename_cx::rename(cx, arm, &name, &new);
        }
    }
    Ok(OpStatus::Finished)
}
