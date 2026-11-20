// SPDX-License-Identifier: GPL-3.0-or-later
//! Pose matrices (WP 6.2): `BKE_pose_where_is` without constraints and IK (frap has neither):
//! each channel's armature-space `pose_mat` from its rest matrix, its local transform and its
//! parent's pose, then `pose_head`/`pose_tail` and the deform matrix `chan_mat`.
//!
//! Ported from blender/source/blender/blenkernel/intern/armature.cc (`BKE_pose_where_is`,
//! `BKE_pose_where_is_bone`, `BKE_pose_where_is_bone_tail`,
//! `BKE_bone_parent_transform_calc_from_pchan`, `BKE_armature_mat_pose_to_bone_ex`),
//! Blender 5.2.4, GPL-2.0-or-later.

use glam::{Mat4, Vec3};

use super::math::{BoneParentTransform, Channels, invert_m4, loc, offset_matrix};
use super::rest::RestBone;
use crate::id::{Armature, Pose, PoseBone};

/// The evaluated matrices of one pose channel.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PoseMats {
    /// `pose_mat` (`PoseBone.matrix`, armature space)
    pub pose_mat: Mat4,
    /// `pose_head` (`PoseBone.head`)
    pub head: Vec3,
    /// `pose_tail` (`PoseBone.tail`)
    pub tail: Vec3,
    /// `chan_mat` after evaluation: `pose_mat · arm_mat⁻¹` (`PoseBone.matrix_channel`)
    pub chan_mat: Mat4,
}

/// `BKE_bone_parent_transform_calc_from_pchan` for bone `i`, with the parent's current pose
/// matrix (`None` when bone `i` has no parent).
pub fn parent_transform(
    arm: &Armature,
    rest: &[RestBone],
    i: usize,
    parent_pose: Option<&Mat4>,
) -> BoneParentTransform {
    let b = &arm.bones[i];
    let flags = b.parent_flags();
    match (b.parent, parent_pose) {
        (Some(p), Some(pp)) if p < rest.len() => {
            let offs = offset_matrix(&rest[i].bone_mat, rest[i].head, rest[p].length);
            BoneParentTransform::from_matrices(flags, &offs, Some((&rest[p].arm_mat, pp)))
        }
        _ => BoneParentTransform::from_matrices(flags, &rest[i].arm_mat, None),
    }
}

/// The channels of bone `i` (`BKE_pose_channel_ensure`'s values when the pose has none).
pub fn channel<'a>(pose: Option<&'a Pose>, arm: &Armature, i: usize) -> Option<&'a PoseBone> {
    pose.and_then(|p| p.bone(&arm.bones[i].name))
}

/// `BKE_pose_where_is_bone` (`do_extra = true`) for bone `i`.
pub fn where_is_bone(
    arm: &Armature,
    rest: &[RestBone],
    i: usize,
    ch: &Channels,
    parent_pose: Option<&Mat4>,
) -> PoseMats {
    let chan_mat = ch.to_mat4(arm.bones[i].use_connect);
    let bpt = parent_transform(arm, rest, i, parent_pose);
    let pose_mat = bpt.apply(&chan_mat);
    finish(pose_mat, &rest[i])
}

fn finish(pose_mat: Mat4, r: &RestBone) -> PoseMats {
    let head = loc(&pose_mat);
    let tail = head + pose_mat.y_axis.truncate() * r.length;
    PoseMats {
        pose_mat,
        head,
        tail,
        chan_mat: pose_mat * invert_m4(&r.arm_mat),
    }
}

/// `BKE_pose_where_is` for every bone of `arm` (index = bone index). `rest_position`
/// (`pose_position = 'REST'`) reads the rest bones.
pub fn where_is(
    arm: &Armature,
    rest: &[RestBone],
    pose: Option<&Pose>,
    rest_position: bool,
) -> Vec<PoseMats> {
    let n = arm.bones.len();
    let mut out: Vec<Option<PoseMats>> = vec![None; n];
    if rest_position {
        return rest
            .iter()
            .map(|r| PoseMats {
                pose_mat: r.arm_mat,
                head: r.arm_head,
                tail: r.arm_tail,
                chan_mat: Mat4::IDENTITY,
            })
            .collect();
    }
    let default = PoseBone::new("");
    fn visit(
        arm: &Armature,
        rest: &[RestBone],
        pose: Option<&Pose>,
        default: &PoseBone,
        i: usize,
        out: &mut Vec<Option<PoseMats>>,
        depth: usize,
    ) {
        if out[i].is_some() {
            return;
        }
        let parent = match arm.bones[i]
            .parent
            .filter(|&p| p < arm.bones.len() && p != i && depth < 1024)
        {
            Some(p) => {
                visit(arm, rest, pose, default, p, out, depth + 1);
                out[p].map(|m| m.pose_mat)
            }
            None => None,
        };
        let ch = channel(pose, arm, i).unwrap_or(default).channels();
        out[i] = Some(where_is_bone(arm, rest, i, &ch, parent.as_ref()));
    }
    for i in 0..n {
        visit(arm, rest, pose, &default, i, &mut out, 0);
    }
    out.into_iter()
        .map(|m| m.expect("invariant: every bone visited"))
        .collect()
}

/// `BKE_armature_mat_pose_to_bone_ex`: the local channel matrix that gives bone `i` the
/// armature-space pose matrix `inmat` (the setter of `PoseBone.matrix`).
pub fn pose_to_bone(
    arm: &Armature,
    rest: &[RestBone],
    i: usize,
    parent_pose: Option<&Mat4>,
    inmat: &Mat4,
) -> Mat4 {
    parent_transform(arm, rest, i, parent_pose)
        .inverted()
        .apply(inmat)
}

/// `BKE_armature_mat_bone_to_pose`.
pub fn bone_to_pose(
    arm: &Armature,
    rest: &[RestBone],
    i: usize,
    parent_pose: Option<&Mat4>,
    inmat: &Mat4,
) -> Mat4 {
    parent_transform(arm, rest, i, parent_pose).apply(inmat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit_armature::rest::rest_pose;
    use crate::id::Bone;

    fn bone(name: &str, parent: Option<usize>, h: [f32; 3], t: [f32; 3]) -> Bone {
        let mut b = Bone::new(name);
        b.parent = parent;
        b.head_local = h;
        b.tail_local = t;
        b
    }

    #[test]
    fn rest_pose_without_transforms() {
        let mut arm = Armature::new("A");
        arm.bones = vec![
            bone("Root", None, [0.0; 3], [0.0, 0.0, 1.0]),
            bone("Child", Some(0), [0.0, 0.0, 1.0], [0.0, 1.0, 1.0]),
        ];
        let rest = rest_pose(&arm);
        let m = where_is(&arm, &rest, None, false);
        for (r, p) in rest.iter().zip(&m) {
            assert!(r.arm_mat.abs_diff_eq(p.pose_mat, 1e-6));
            assert!(r.arm_tail.abs_diff_eq(p.tail, 1e-6));
        }
    }

    #[test]
    fn parent_rotation_carries_child() {
        let mut arm = Armature::new("A");
        arm.bones = vec![
            bone("Root", None, [0.0; 3], [0.0, 0.0, 1.0]),
            bone("Child", Some(0), [0.0, 0.0, 1.0], [0.0, 0.0, 2.0]),
        ];
        let rest = rest_pose(&arm);
        let mut pose = Pose {
            bones: vec![PoseBone::new("Root"), PoseBone::new("Child")],
            ..Pose::default()
        };
        // 90° about the bone's local X: the bone (along +Z) tips towards -Y... in bone space.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        pose.bones[0].rotation_quaternion = [h, h, 0.0, 0.0];
        let m = where_is(&arm, &rest, Some(&pose), false);
        // Child head follows the root's tail.
        assert!(m[1].head.abs_diff_eq(m[0].tail, 1e-6), "{m:?}");
        assert!((m[1].tail - m[1].head).length() - 1.0 < 1e-5);
        assert!(m[0].tail.z.abs() < 1e-6, "{:?}", m[0].tail);
    }
}
