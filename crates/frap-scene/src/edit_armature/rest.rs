// SPDX-License-Identifier: GPL-3.0-or-later
//! The rest pose of an armature (WP 6.2): the per-bone values Blender derives in
//! `armature_finalize_restpose` (`Bone.head`/`tail` in the parent's space, the converted bone
//! roll, `bone_mat`, `arm_mat`, `length`) from the armature-space head, tail and edit roll
//! that frap stores (`Bone::head_local`/`tail_local`/`roll`).
//!
//! Ported from blender/source/blender/editors/armature/armature_utils.cc
//! (`armature_finalize_restpose`, `ED_armature_ebone_to_mat3`) and
//! blender/source/blender/blenkernel/intern/armature.cc (`BKE_armature_where_is_bone`),
//! Blender 5.2.4, GPL-2.0-or-later.

use glam::{Mat3, Mat4, Vec3};

use super::math::{
    invert_m3, invert_m4, m3, m4_from_m3, mul_mat3_m4_v3, normalize, offset_matrix,
    vec_roll_to_mat3, vec_roll_to_mat3_normalized,
};
use crate::id::Armature;

/// The rest data of one bone.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RestBone {
    /// `bone->head` (parent space; armature space for a root bone)
    pub head: Vec3,
    /// `bone->tail`
    pub tail: Vec3,
    /// `bone->roll` (the converted roll, not the edit roll)
    pub roll: f32,
    /// `bone->length`
    pub length: f32,
    /// `bone->bone_mat` (`Bone.matrix`)
    pub bone_mat: Mat3,
    /// `bone->arm_mat` (`Bone.matrix_local`)
    pub arm_mat: Mat4,
    /// `bone->arm_head` (`Bone.head_local`)
    pub arm_head: Vec3,
    /// `bone->arm_tail` (`Bone.tail_local`)
    pub arm_tail: Vec3,
}

/// `BKE_armature_where_is_bone` without recursion.
fn where_is(head: Vec3, tail: Vec3, roll: f32, parent: Option<&RestBone>) -> (f32, Mat3, Mat4) {
    let vec = tail - head;
    let length = vec.length();
    let bone_mat = vec_roll_to_mat3(vec, roll);
    let arm_mat = match parent {
        Some(p) => p.arm_mat * offset_matrix(&bone_mat, head, p.length),
        None => m4_from_m3(&bone_mat, head),
    };
    (length, bone_mat, arm_mat)
}

/// `ED_armature_ebone_to_mat3` for an edit bone given by its armature-space head, tail and
/// roll (a zero-length bone takes its parent's direction and roll).
pub fn ebone_mat3(head: Vec3, tail: Vec3, roll: f32, parent: Option<(Vec3, Vec3, f32)>) -> Mat3 {
    let (mut delta, len) = normalize(tail - head);
    let mut roll = roll;
    if len == 0.0
        && let Some((ph, pt, pr)) = parent
    {
        delta = normalize(pt - ph).0;
        roll = pr;
    }
    vec_roll_to_mat3_normalized(delta, roll)
}

/// One bone of `armature_finalize_restpose`, its parent already done.
pub fn finalize_bone(
    arm_head: Vec3,
    arm_tail: Vec3,
    arm_roll: f32,
    parent: Option<&RestBone>,
) -> RestBone {
    let (head, tail) = match parent {
        Some(p) => {
            let parmat_inv = invert_m4(&p.arm_mat);
            (
                mul_mat3_m4_v3(&parmat_inv, arm_head - p.arm_tail),
                mul_mat3_m4_v3(&parmat_inv, arm_tail - p.arm_tail),
            )
        }
        None => (arm_head, arm_tail),
    };
    let (_, _, arm_mat0) = where_is(head, tail, 0.0, parent);
    let premat = ebone_mat3(arm_head, arm_tail, arm_roll, None);
    let imat = invert_m3(&premat);
    let postmat = m3(&arm_mat0);
    let difmat = imat * postmat;
    let roll = -difmat.z_axis.x.atan2(difmat.z_axis.z);
    let (length, bone_mat, arm_mat) = where_is(head, tail, roll, parent);
    RestBone {
        head,
        tail,
        roll,
        length,
        bone_mat,
        arm_mat,
        arm_head,
        arm_tail,
    }
}

/// The rest pose of every bone of `arm` (index = bone index). Parents are processed before
/// their children whatever the storage order.
pub fn rest_pose(arm: &Armature) -> Vec<RestBone> {
    let n = arm.bones.len();
    let mut out: Vec<Option<RestBone>> = vec![None; n];
    fn visit(arm: &Armature, i: usize, out: &mut Vec<Option<RestBone>>, depth: usize) {
        if out[i].is_some() {
            return;
        }
        let b = &arm.bones[i];
        let parent = match b
            .parent
            .filter(|&p| p < arm.bones.len() && p != i && depth < 1024)
        {
            Some(p) => {
                visit(arm, p, out, depth + 1);
                out[p]
            }
            None => None,
        };
        out[i] = Some(finalize_bone(
            Vec3::from_array(b.head_local),
            Vec3::from_array(b.tail_local),
            b.roll,
            parent.as_ref(),
        ));
    }
    for i in 0..n {
        visit(arm, i, &mut out, 0);
    }
    out.into_iter()
        .map(|r| r.expect("invariant: every bone visited"))
        .collect()
}

/// A glam matrix as the RNA rows `Bone::matrix_local` stores.
pub fn rows4(m: &Mat4) -> [[f32; 4]; 4] {
    m.transpose().to_cols_array_2d()
}

/// A 3×3 glam matrix as RNA rows.
pub fn rows3(m: &Mat3) -> [[f32; 3]; 3] {
    m.transpose().to_cols_array_2d()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bone_rest_matrix() {
        let r = finalize_bone(Vec3::ZERO, Vec3::Z, 0.0, None);
        assert_eq!(r.length, 1.0);
        assert!(r.roll.abs() < 1e-7);
        let rows = rows4(&r.arm_mat);
        let want = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        for (a, b) in rows.iter().zip(want) {
            for (x, y) in a.iter().zip(b) {
                assert!((x - y).abs() < 1e-7, "{rows:?}");
            }
        }
    }

    #[test]
    fn child_head_is_parent_relative() {
        let p = finalize_bone(Vec3::ZERO, Vec3::Z, 0.0, None);
        let c = finalize_bone(Vec3::Z, Vec3::new(0.0, 1.0, 1.0), 0.0, Some(&p));
        assert!(c.head.abs_diff_eq(Vec3::ZERO, 1e-6), "{:?}", c.head);
        // The child points along -Z of the parent's space (parent Y = world Z).
        assert!(
            (c.arm_mat.w_axis.truncate() - Vec3::Z).length() < 1e-6,
            "{:?}",
            c.arm_mat
        );
        assert!(c.arm_mat.y_axis.truncate().abs_diff_eq(Vec3::Y, 1e-6));
    }
}
