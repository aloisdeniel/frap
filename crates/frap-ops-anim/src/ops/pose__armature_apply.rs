// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.armature_apply(selected=False)`, tier T0 (WP 6.2). Port of
//! `apply_armature_pose2bones_exec`, `applyarmature_set_edit_position`,
//! `applyarmature_transfer_properties`, `applyarmature_adjust_edit_position` and
//! `applyarmature_process_selected_recursive`
//! (blender/source/blender/editors/armature/pose_transform.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! The evaluated pose of the active pose object becomes the rest pose: each bone's head, tail
//! and roll are set from its pose matrix (`mat3_vec_to_roll`), its channels reset to the
//! identity, the rest matrices rebuilt (`ED_armature_from_edit`) and every user's pose
//! rebuilt. With `selected=True` only the selected bones are applied, their unselected
//! descendants keep their final position (the applied parent effect is baked into their rest
//! pose and their location channel). An armature with an action reports Blender's WARNING;
//! `selected=True` without selected bones is `CANCELLED`; a linked armature reports
//! `Cannot apply pose to lib-linked armature`. Objects parented to bones keep their parent
//! inverse (frap does not evaluate bone parenting before WP 6.5).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::ArmatureId;
use frap_scene::edit_armature::EditArmature;
use frap_scene::edit_armature::math::{
    BoneParentTransform, invert_m4, loc, m3, m4_from_m3, mat3_vec_to_roll,
    mul_mat3_m4_v3, offset_matrix, vec_roll_to_mat3,
};
use frap_scene::edit_armature::posing::{PoseMats, where_is};
use frap_scene::edit_armature::rest::{RestBone, rest_pose};
use frap_scene::id::{Armature, Pose};
use frap_spec_ops_anim::pose;
use glam::{Mat4, Vec3};

/// `applyarmature_set_edit_position`: head, tail and roll from a pose-space matrix; the new
/// rest matrix (without scale) when asked.
fn set_edit_position(ea: &mut EditArmature, i: usize, pose_mat: &Mat4, new_tail: Vec3) -> Mat4 {
    let b = &mut ea.edit_bones[i];
    b.head = loc(pose_mat).to_array();
    b.tail = new_tail.to_array();
    let delta = new_tail - loc(pose_mat);
    b.roll = mat3_vec_to_roll(&m3(pose_mat), delta);
    let premat = vec_roll_to_mat3(delta, b.roll);
    m4_from_m3(&premat, loc(pose_mat))
}

/// `applyarmature_transfer_properties`: the channel's transform is cleared.
fn reset_channel(pose: &mut Pose, name: &str) {
    if let Some(pb) = pose.bone_mut(name) {
        pb.location = [0.0; 3];
        pb.rotation_euler = [0.0; 3];
        pb.rotation_quaternion = [1.0, 0.0, 0.0, 0.0];
        pb.rotation_axis_angle = [0.0, 0.0, 1.0, 0.0];
        pb.scale = [1.0; 3];
    }
}

/// Parent state of the selected-only walk (`ApplyArmature_ParentState`).
#[derive(Copy, Clone)]
struct ParentState {
    bone: usize,
    new_rest_mat: Mat4,
    new_arm_mat: Mat4,
}

struct Walk<'a> {
    arm: &'a Armature,
    rest: &'a [RestBone],
    mats: &'a [PoseMats],
    selected: &'a [bool],
    ea: EditArmature,
    pose: Pose,
}

impl Walk<'_> {
    /// `applyarmature_process_selected_recursive`.
    fn process(&mut self, i: usize, pstate: Option<ParentState>) {
        let b = &self.arm.bones[i];
        let Some(ei) = self.ea.find(&b.name) else {
            return;
        };
        if self.pose.bone(&b.name).is_none() {
            return;
        }
        let flags = b.parent_flags();
        let mut new = ParentState {
            bone: i,
            new_rest_mat: Mat4::IDENTITY,
            new_arm_mat: Mat4::IDENTITY,
        };
        let next;
        if self.selected[i] {
            let invparent = match b.parent {
                Some(p) => {
                    let offs = offset_matrix(&self.rest[i].bone_mat, self.rest[i].head, self.rest[p].length);
                    let old = BoneParentTransform::from_matrices(
                        flags,
                        &offs,
                        Some((&self.rest[p].arm_mat, &self.mats[p].pose_mat)),
                    );
                    let new_parent_pose = pstate.map_or(self.rest[p].arm_mat, |s| s.new_rest_mat);
                    let newb = BoneParentTransform::from_matrices(
                        flags,
                        &offs,
                        Some((&self.rest[p].arm_mat, &new_parent_pose)),
                    );
                    newb.combine(&old.inverted())
                }
                None => BoneParentTransform::IDENTITY,
            };
            new.new_rest_mat = invparent.apply(&self.mats[i].pose_mat);
            let new_tail = new.new_rest_mat.transform_point3(Vec3::new(0.0, self.rest[i].length, 0.0));
            new.new_arm_mat = set_edit_position(&mut self.ea, ei, &new.new_rest_mat, new_tail);
            reset_channel(&mut self.pose, &b.name);
            next = Some(new);
        } else if let Some(ps) = pstate {
            let offs = offset_matrix(&self.rest[i].bone_mat, self.rest[i].head, self.rest[ps.bone].length);
            let bpt = BoneParentTransform::from_matrices(
                flags,
                &offs,
                Some((&self.rest[ps.bone].arm_mat, &ps.new_rest_mat)),
            );
            new.new_rest_mat = bpt.apply(&Mat4::IDENTITY);
            let chan_loc = self
                .pose
                .bone(&b.name)
                .map_or(Vec3::ZERO, |pb| Vec3::from_array(pb.location));
            let old_chan_loc = mul_mat3_m4_v3(&bpt.loc, chan_loc);
            let delta = new.new_rest_mat * invert_m4(&self.rest[i].arm_mat);
            // `applyarmature_adjust_edit_position`.
            let eb = &self.ea.edit_bones[ei];
            let premat = vec_roll_to_mat3(eb.tail_v() - eb.head_v(), eb.roll);
            let mut new_pose = m4_from_m3(&(m3(&delta) * premat), Vec3::ZERO);
            new_pose.w_axis = delta.transform_point3(eb.head_v()).extend(1.0);
            let new_tail = delta.transform_point3(eb.tail_v());
            new.new_arm_mat = set_edit_position(&mut self.ea, ei, &new_pose, new_tail);
            if !b.use_connect && old_chan_loc != Vec3::ZERO {
                let inv_parent_arm = invert_m4(&ps.new_arm_mat);
                let offs = inv_parent_arm * new.new_arm_mat;
                let bpt = BoneParentTransform::from_matrices(
                    flags,
                    &offs,
                    Some((&ps.new_arm_mat, &ps.new_arm_mat)),
                );
                let inv_loc = invert_m4(&bpt.loc);
                if let Some(pb) = self.pose.bone_mut(&b.name) {
                    pb.location = mul_mat3_m4_v3(&inv_loc, old_chan_loc).to_array();
                }
            }
            next = Some(new);
        } else {
            next = None;
        }
        for c in self.arm.children_of(Some(i)) {
            self.process(c, next);
        }
    }
}

#[operator(poll = poll::POSEMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: pose::ArmatureApply) -> OpResult {
    let Some(ob) = cx.ctx().pose_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(arm_ref) = cx
        .main()
        .objects
        .get(ob)
        .and_then(|o| o.data)
        .and_then(|d| d.typed::<ArmatureId>())
    else {
        return Ok(OpStatus::Cancelled);
    };
    let (arm, pose, has_action, linked) = {
        let main = cx.main();
        let (Some(o), Some(a)) = (main.objects.get(ob), main.armatures.get(arm_ref)) else {
            return Ok(OpStatus::Cancelled);
        };
        let has_action = o.animation_data.as_ref().is_some_and(|ad| ad.action.is_some());
        (
            a.clone(),
            o.pose.clone().unwrap_or_default(),
            has_action,
            a.header.library.is_some(),
        )
    };
    if linked {
        return Err(OpError::Runtime(
            "Cannot apply pose to lib-linked armature".into(),
        ));
    }
    if has_action {
        cx.report(
            ReportType::Warning,
            "Actions on this armature will be destroyed by this new rest pose as the transforms stored are relative to the old rest pose",
        );
    }
    let selected: Vec<bool> = arm
        .bones
        .iter()
        .map(|b| {
            pose.bone(&b.name)
                .is_some_and(|pb| pb.select && !pb.hide && arm.bone_collections_visible(b))
        })
        .collect();
    if p.selected.value && !selected.iter().any(|s| *s) {
        return Ok(OpStatus::Cancelled);
    }
    let rest = rest_pose(&arm);
    let rest_position = arm.pose_position == "REST";
    let mats = where_is(&arm, &rest, Some(&pose), rest_position);
    let mut walk = Walk {
        arm: &arm,
        rest: &rest,
        mats: &mats,
        selected: &selected,
        ea: EditArmature::from_armature(&arm),
        pose,
    };
    if p.selected.value {
        for r in arm.children_of(None) {
            walk.process(r, None);
        }
    } else {
        let names: Vec<String> = walk.pose.bones.iter().map(|pb| pb.name.to_string()).collect();
        for n in names {
            let (Some(i), Some(ei)) = (arm.bone_index(&n), walk.ea.find(&n)) else {
                continue;
            };
            set_edit_position(&mut walk.ea, ei, &mats[i].pose_mat, mats[i].tail);
            reset_channel(&mut walk.pose, &n);
        }
    }
    let Walk { ea, pose, .. } = walk;
    let new_arm = {
        let (main, log) = cx.main_mut();
        let Some(a) = main.armatures.get_mut(arm_ref, log) else {
            return Ok(OpStatus::Cancelled);
        };
        ea.apply_to(a);
        a.clone()
    };
    let users: Vec<_> = cx
        .main()
        .objects
        .iter()
        .filter(|(_, o)| o.data == Some(arm_ref.any()))
        .map(|(r, _)| r)
        .collect();
    for u in users {
        let o = cx.object_mut(u)?;
        if u == ob {
            o.pose = Some(pose.clone());
        }
        Pose::rebuild(&mut o.pose, &new_arm);
    }
    Ok(OpStatus::Finished)
}
