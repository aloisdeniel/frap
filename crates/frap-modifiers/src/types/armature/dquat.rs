// SPDX-License-Identifier: GPL-3.0-or-later
//! Dual quaternions for the ARMATURE modifier's `use_deform_preserve_volume`
//! (`ARM_DEF_QUATERNION`): `mat4_to_dquat` (the per-bone deform quaternion Blender stores in
//! `pchan->runtime.deform_dual_quat`), `add_weighted_dq_dq_pivot`, `normalize_dq` and
//! `mul_v3m3_dq`.
//!
//! Ported from blender/source/blender/blenlib/intern/math_rotation_c.cc and
//! blender/source/blender/blenlib/intern/math_matrix_c.cc (`orthogonalize_m4`,
//! `is_orthonormal_m3`, `determinant_m4`), Blender 5.2.4, GPL-2.0-or-later. The matrices are
//! glam column-major (`m.col(i)` is Blender's `m[i]`).

use frap_math::quat::{QuatWxyz, dot_qtqt, mat4_to_quat, quat_to_mat4};
use glam::{Mat4, Vec3};

use super::mul_m4_m4m4;

/// `DualQuat`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct DualQuat {
    /// Non-dual part (w, x, y, z).
    pub quat: QuatWxyz,
    /// Dual part.
    pub trans: [f32; 4],
    /// Scale/shear part.
    pub scale: Mat4,
    /// Weight of the scale part (0: no scale).
    pub scale_weight: f32,
}

impl Default for DualQuat {
    /// `DualQuat dq = {}`: everything zero.
    fn default() -> Self {
        DualQuat {
            quat: [0.0; 4],
            trans: [0.0; 4],
            scale: Mat4::ZERO,
            scale_weight: 0.0,
        }
    }
}

/// `mat4_to_size`.
fn mat4_to_size(m: &Mat4) -> Vec3 {
    Vec3::new(
        m.x_axis.truncate().length(),
        m.y_axis.truncate().length(),
        m.z_axis.truncate().length(),
    )
}

/// `normalize_v3` in place (a zero vector stays zero).
fn normalize(v: Vec3) -> Vec3 {
    let d = v.dot(v);
    if d > 1.0e-35 { v * (1.0 / d.sqrt()) } else { Vec3::ZERO }
}

/// `is_orthonormal_m3` on the upper 3×3 of `m`.
fn is_orthonormal_m3(m: &Mat4) -> bool {
    let c = [
        m.x_axis.truncate(),
        m.y_axis.truncate(),
        m.z_axis.truncate(),
    ];
    for i in 0..3 {
        for j in 0..i {
            if c[i].dot(c[j]).abs() > 1e-5 {
                return false;
            }
        }
    }
    c.iter().all(|v| (v.dot(*v) - 1.0).abs() <= 1e-5)
}

/// `determinant_m3(a1, a2, a3, b1, b2, b3, c1, c2, c3)`.
#[allow(clippy::too_many_arguments)]
fn det3(a1: f32, a2: f32, a3: f32, b1: f32, b2: f32, b3: f32, c1: f32, c2: f32, c3: f32) -> f32 {
    a1 * (b2 * c3 - b3 * c2) - b1 * (a2 * c3 - a3 * c2) + c1 * (a2 * b3 - a3 * b2)
}

/// `determinant_m4`.
fn determinant_m4(m: &Mat4) -> f32 {
    let r = |i: usize| m.col(i);
    let (a1, b1, c1, d1) = (r(0).x, r(0).y, r(0).z, r(0).w);
    let (a2, b2, c2, d2) = (r(1).x, r(1).y, r(1).z, r(1).w);
    let (a3, b3, c3, d3) = (r(2).x, r(2).y, r(2).z, r(2).w);
    let (a4, b4, c4, d4) = (r(3).x, r(3).y, r(3).z, r(3).w);
    a1 * det3(b2, b3, b4, c2, c3, c4, d2, d3, d4) - b1 * det3(a2, a3, a4, c2, c3, c4, d2, d3, d4)
        + c1 * det3(a2, a3, a4, b2, b3, b4, d2, d3, d4)
        - d1 * det3(a2, a3, a4, b2, b3, b4, c2, c3, c4)
}

/// `orthogonalize_m4(R, 1)`: the Y axis is kept (normalized), X and Z are rebuilt, and the
/// axes keep their former lengths.
fn orthogonalize_m4_y(m: &Mat4) -> Mat4 {
    let size = mat4_to_size(m);
    let mut r0 = m.x_axis.truncate();
    let r1 = normalize(m.y_axis.truncate());
    let mut r2 = m.z_axis.truncate();
    if r1.dot(r0) < 1.0 {
        r2 = normalize(r0.cross(r1));
        r0 = r1.cross(r2);
    } else if r0.dot(r2) < 1.0 {
        r0 = normalize(r1.cross(r2));
        r2 = r0.cross(r1);
    } else {
        let vec = Vec3::new(r1.y, r1.z, r1.x);
        r0 = normalize(r1.cross(vec));
        r2 = r0.cross(r1);
    }
    Mat4::from_cols(
        (r0 * size.x).extend(m.x_axis.w),
        (r1 * size.y).extend(m.y_axis.w),
        (r2 * size.z).extend(m.z_axis.w),
        m.w_axis,
    )
}

/// `invert_m4_m4` (a singular matrix gives zero, as Eigen's inverse does in Blender).
fn invert_m4(m: &Mat4) -> Mat4 {
    if m.determinant() == 0.0 {
        Mat4::ZERO
    } else {
        m.inverse()
    }
}

/// `mat4_to_dquat(dq, basemat, mat)`: the deform quaternion of a bone whose rest matrix is
/// `basemat` and whose deform matrix (`chan_mat`) is `mat`.
pub(crate) fn mat4_to_dquat(basemat: &Mat4, mat: &Mat4) -> DualQuat {
    let mut dq = DualQuat::default();
    let base_rs = mul_m4_m4m4(mat, basemat);
    let scale = mat4_to_size(&base_rs);
    let dscale = scale - Vec3::ONE;
    let r = if !is_orthonormal_m3(mat)
        || determinant_m4(mat) < 0.0
        || dscale.length_squared() > 1e-4f32 * 1e-4f32
    {
        // Extract R and S (extra orthogonalize to avoid flipping with stretched bones).
        let tmp = orthogonalize_m4_y(&base_rs);
        let basequat = mat4_to_quat(&tmp);
        let mut base_r = quat_to_mat4(basequat);
        base_r.w_axis = base_rs.w_axis.truncate().extend(base_r.w_axis.w);
        let baseinv = invert_m4(basemat);
        let r = mul_m4_m4m4(&base_r, &baseinv);
        let base_rinv = invert_m4(&base_r);
        let s = mul_m4_m4m4(&base_rinv, &base_rs);
        // mul_m4_series(dq->scale, basemat, S, baseinv)
        dq.scale = mul_m4_m4m4(&mul_m4_m4m4(basemat, &s), &baseinv);
        dq.scale_weight = 1.0;
        r
    } else {
        dq.scale_weight = 0.0;
        *mat
    };
    dq.quat = mat4_to_quat(&r);
    let t = r.w_axis;
    let q = dq.quat;
    dq.trans = [
        -0.5 * (t.x * q[1] + t.y * q[2] + t.z * q[3]),
        0.5 * (t.x * q[0] + t.y * q[3] - t.z * q[2]),
        0.5 * (-t.x * q[3] + t.y * q[0] + t.z * q[1]),
        0.5 * (t.x * q[2] - t.y * q[1] + t.z * q[0]),
    ];
    dq
}

/// `add_weighted_dq_dq`.
fn add_weighted_dq_dq(sum: &mut DualQuat, dq: &DualQuat, weight: f32) {
    let mut weight = weight;
    let mut flipped = false;
    if dot_qtqt(dq.quat, sum.quat) < 0.0 {
        flipped = true;
        weight = -weight;
    }
    for i in 0..4 {
        sum.quat[i] += weight * dq.quat[i];
    }
    for i in 0..4 {
        sum.trans[i] += weight * dq.trans[i];
    }
    if dq.scale_weight != 0.0 {
        if flipped {
            weight = -weight;
        }
        sum.scale += dq.scale * weight;
        sum.scale_weight += weight;
    }
}

/// `mul_v3_m4v3`.
fn mul_v3_m4v3(m: &Mat4, v: Vec3) -> Vec3 {
    let c = |i: usize| m.col(i);
    Vec3::new(
        v.x * c(0).x + v.y * c(1).x + c(2).x * v.z + c(3).x,
        v.x * c(0).y + v.y * c(1).y + c(2).y * v.z + c(3).y,
        v.x * c(0).z + v.y * c(1).z + c(2).z * v.z + c(3).z,
    )
}

/// `add_weighted_dq_dq_pivot(dq_sum, dq, pivot, weight, compute_scale_matrix = false)` (the
/// modifier never asks for deform matrices).
pub(crate) fn add_weighted_dq_dq_pivot(sum: &mut DualQuat, dq: &DualQuat, pivot: Vec3, weight: f32) {
    if dq.scale_weight != 0.0 {
        let mut mdq = *dq;
        let dst = mul_v3_m4v3(&mdq.scale, pivot) - pivot;
        let q = mdq.quat;
        mdq.trans[0] -= 0.5 * (q[1] * dst.x + q[2] * dst.y + q[3] * dst.z);
        mdq.trans[1] += 0.5 * (q[0] * dst.x + q[2] * dst.z - q[3] * dst.y);
        mdq.trans[2] += 0.5 * (q[0] * dst.y + q[3] * dst.x - q[1] * dst.z);
        mdq.trans[3] += 0.5 * (q[0] * dst.z + q[1] * dst.y - q[2] * dst.x);
        // `compute_scale_matrix == false`: the scale matrix is discarded.
        mdq.scale_weight = 0.0;
        add_weighted_dq_dq(sum, &mdq, weight);
    } else {
        add_weighted_dq_dq(sum, dq, weight);
    }
}

/// `normalize_dq`.
pub(crate) fn normalize_dq(dq: &mut DualQuat, totweight: f32) {
    let scale = 1.0 / totweight;
    for v in dq.quat.iter_mut() {
        *v *= scale;
    }
    for v in dq.trans.iter_mut() {
        *v *= scale;
    }
    if dq.scale_weight != 0.0 {
        let addweight = totweight - dq.scale_weight;
        if addweight != 0.0 {
            dq.scale.x_axis.x += addweight;
            dq.scale.y_axis.y += addweight;
            dq.scale.z_axis.z += addweight;
            dq.scale.w_axis.w += addweight;
        }
        dq.scale *= scale;
        dq.scale_weight = 1.0;
    }
}

/// `mul_v3m3_dq(r, nullptr, dq)`: transforms `r`.
pub(crate) fn mul_v3m3_dq(r: Vec3, dq: &DualQuat) -> Vec3 {
    let [w, x, y, z] = dq.quat;
    let [t0, t1, t2, t3] = dq.trans;
    // Rows of Blender's M as written (M[col][row]).
    let m00 = w * w + x * x - y * y - z * z;
    let m10 = 2.0 * (x * y - w * z);
    let m20 = 2.0 * (x * z + w * y);
    let m01 = 2.0 * (x * y + w * z);
    let m11 = w * w + y * y - x * x - z * z;
    let m21 = 2.0 * (y * z - w * x);
    let m02 = 2.0 * (x * z - w * y);
    let m12 = 2.0 * (y * z + w * x);
    let m22 = w * w + z * z - x * x - y * y;
    let mut len2 = dot_qtqt(dq.quat, dq.quat);
    if len2 > 0.0 {
        len2 = 1.0 / len2;
    }
    let t = Vec3::new(
        2.0 * (-t0 * x + w * t1 - t2 * z + y * t3),
        2.0 * (-t0 * y + t1 * z - x * t3 + w * t2),
        2.0 * (-t0 * z + x * t2 + w * t3 - t1 * y),
    );
    let mut r = r;
    if dq.scale_weight != 0.0 {
        // mul_m4_v3
        r = mul_v3_m4v3(&dq.scale, r);
    }
    // mul_m3_v3(M, r)
    let rr = Vec3::new(
        m00 * r.x + m10 * r.y + m20 * r.z,
        m01 * r.x + m11 * r.y + m21 * r.z,
        m02 * r.x + m12 * r.y + m22 * r.z,
    );
    (rr + t) * len2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).abs().max_element() < 1e-5
    }

    #[test]
    fn rigid_bone_transforms_like_its_matrix() {
        let base = Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let chan = Mat4::from_translation(Vec3::new(0.5, -1.0, 2.0)) * Mat4::from_rotation_z(0.7);
        let dq = mat4_to_dquat(&base, &chan);
        assert_eq!(dq.scale_weight, 0.0);
        let mut sum = DualQuat::default();
        let p = Vec3::new(0.3, 0.2, -0.4);
        add_weighted_dq_dq_pivot(&mut sum, &dq, p, 0.5);
        normalize_dq(&mut sum, 0.5);
        assert!(close(mul_v3m3_dq(p, &sum), chan.transform_point3(p)));
    }

    #[test]
    fn scaled_bone_keeps_the_scale_at_the_pivot() {
        let base = Mat4::IDENTITY;
        let chan = Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
        let dq = mat4_to_dquat(&base, &chan);
        assert_eq!(dq.scale_weight, 1.0);
        let mut sum = DualQuat::default();
        let p = Vec3::new(1.0, 2.0, 3.0);
        add_weighted_dq_dq_pivot(&mut sum, &dq, p, 1.0);
        normalize_dq(&mut sum, 1.0);
        // The scale's translation at the pivot moves the point; the scale itself is dropped.
        assert!(close(mul_v3m3_dq(p, &sum), Vec3::new(2.0, 2.0, 3.0)));
    }

    #[test]
    fn determinant_matches_glam() {
        let m = Mat4::from_cols_array(&[
            1.0, 2.0, 0.5, 0.0, 0.0, 1.0, 3.0, 0.0, 4.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0,
        ]);
        assert!((determinant_m4(&m) - m.determinant()).abs() < 1e-4);
    }
}
