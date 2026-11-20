// SPDX-License-Identifier: GPL-3.0-or-later
//! Bone maths (WP 6.2): bone-vector/roll matrices, the bone parent transform and the small
//! matrix helpers they need.
//!
//! Ported from blender/source/blender/blenkernel/intern/armature.cc (`vec_roll_to_mat3_normalized`,
//! `mat3_vec_to_roll`, `BKE_bone_offset_matrix_get`, `BKE_bone_parent_transform_*`,
//! `BKE_pchan_to_mat4`, `BKE_pchan_rot_to_mat3`, `BKE_pchan_mat3_to_rot`,
//! `BKE_pchan_apply_mat4`), blender/source/blender/blenlib/intern/math_matrix_c.cc
//! (`orthogonalize_m4_stable`, `normalize_m4_ex`, `mat4_to_size_fix_shear`) and
//! blender/source/blender/blenlib/intern/math_rotation_c.cc (`quat_split_swing_and_twist`,
//! `axis_angle_normalized_to_mat3`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Matrices are glam's (column-major): Blender's `mat[i]` is column `i`, `mul_m4_m4m4(r, a, b)`
//! is `a * b`.

use frap_math::EulerOrder;
use frap_math::euler::{
    eul_o_to_mat3, mat3_normalized_to_compatible_eul_o, mat3_normalized_to_eul_o,
};
use frap_math::matrix::RotationMode;
use frap_math::quat::{
    axis_angle_to_mat3, mat3_normalized_to_quat, normalize_qt, quat_to_axis_angle, quat_to_mat3,
};
use glam::{Mat3, Mat4, Vec3, Vec4};

/// `normalize_v3`: the unit vector and the length (zero vector and 0 below `1e-35` squared).
pub fn normalize(v: Vec3) -> (Vec3, f32) {
    let d = v.dot(v);
    if d > 1.0e-35 {
        let d = d.sqrt();
        (v * (1.0 / d), d)
    } else {
        (Vec3::ZERO, 0.0)
    }
}

/// `axis_angle_normalized_to_mat3`.
pub fn axis_angle_normalized_to_mat3(axis: Vec3, angle: f32) -> Mat3 {
    let (s, c) = (angle.sin(), angle.cos());
    let ico = 1.0 - c;
    let nsi = axis * s;
    let n00 = (axis.x * axis.x) * ico;
    let n01 = (axis.x * axis.y) * ico;
    let n11 = (axis.y * axis.y) * ico;
    let n02 = (axis.x * axis.z) * ico;
    let n12 = (axis.y * axis.z) * ico;
    let n22 = (axis.z * axis.z) * ico;
    Mat3::from_cols(
        Vec3::new(n00 + c, n01 + nsi.z, n02 - nsi.y),
        Vec3::new(n01 - nsi.z, n11 + c, n12 + nsi.x),
        Vec3::new(n02 + nsi.y, n12 - nsi.x, n22 + c),
    )
}

/// `vec_roll_to_mat3_normalized`: the rotation taking +Y to the unit bone vector `nor`, then
/// rolled by `roll` about it.
pub fn vec_roll_to_mat3_normalized(nor: Vec3, roll: f32) -> Mat3 {
    const SAFE_THRESHOLD: f32 = 6.1e-3;
    const CRITICAL_THRESHOLD: f32 = 2.5e-4;
    const THRESHOLD_SQUARED: f32 = CRITICAL_THRESHOLD * CRITICAL_THRESHOLD;
    let (x, y, z) = (nor.x, nor.y, nor.z);
    let mut theta = 1.0 + y;
    let theta_alt = x * x + z * z;
    // b[col][row] as in Blender.
    let mut b = [[0.0f32; 3]; 3];
    if theta > SAFE_THRESHOLD || theta_alt > THRESHOLD_SQUARED {
        b[0][1] = -x;
        b[1][0] = x;
        b[1][1] = y;
        b[1][2] = z;
        b[2][1] = -z;
        if theta <= SAFE_THRESHOLD {
            theta = theta_alt * 0.5 + theta_alt * theta_alt * 0.125;
        }
        b[0][0] = 1.0 - x * x / theta;
        b[2][2] = 1.0 - z * z / theta;
        let v = -x * z / theta;
        b[2][0] = v;
        b[0][2] = v;
    } else {
        b = [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let bmat = Mat3::from_cols_array_2d(&b);
    let rmat = axis_angle_normalized_to_mat3(nor, roll);
    rmat * bmat
}

/// `vec_roll_to_mat3`: `vec` normalized first.
pub fn vec_roll_to_mat3(vec: Vec3, roll: f32) -> Mat3 {
    vec_roll_to_mat3_normalized(normalize(vec).0, roll)
}

/// `quat_split_swing_and_twist` (twist angle only).
pub fn quat_twist_angle(q_in: [f32; 4], axis: usize) -> f32 {
    let q = if q_in[0] < 0.0 {
        [-q_in[0], -q_in[1], -q_in[2], -q_in[3]]
    } else {
        q_in
    };
    let t = q[axis + 1].atan2(q[0]);
    2.0 * t
}

/// `mat3_vec_to_roll`: the roll of `mat` about `vec`.
pub fn mat3_vec_to_roll(mat: &Mat3, vec: Vec3) -> f32 {
    let vecmat = vec_roll_to_mat3(vec, 0.0);
    let rollmat = vecmat.inverse() * *mat;
    let q = frap_math::quat::mat3_to_quat(&rollmat);
    quat_twist_angle(q, 1)
}

/// `mat3_to_vec_roll`: the bone direction (the Y axis) and roll of `mat`.
pub fn mat3_to_vec_roll(mat: &Mat3) -> (Vec3, f32) {
    (mat.y_axis, mat3_vec_to_roll(mat, mat.y_axis))
}

/// `copy_m4_m3` + translation.
pub fn m4_from_m3(m: &Mat3, loc: Vec3) -> Mat4 {
    Mat4::from_cols(
        m.x_axis.extend(0.0),
        m.y_axis.extend(0.0),
        m.z_axis.extend(0.0),
        loc.extend(1.0),
    )
}

/// `copy_m3_m4`.
pub fn m3(m: &Mat4) -> Mat3 {
    Mat3::from_mat4(*m)
}

/// The translation column of a 4×4.
pub fn loc(m: &Mat4) -> Vec3 {
    m.w_axis.truncate()
}

/// `mul_v3_m4v3`.
pub fn mul_m4_v3(m: &Mat4, v: Vec3) -> Vec3 {
    m.transform_point3(v)
}

/// `mul_mat3_m4_v3`: the 3×3 part of `m` applied to `v`.
pub fn mul_mat3_m4_v3(m: &Mat4, v: Vec3) -> Vec3 {
    m.transform_vector3(v)
}

/// `invert_m4_m4` (identity when singular, as Blender leaves the result undefined; frap picks
/// the identity).
pub fn invert_m4(m: &Mat4) -> Mat4 {
    if m.determinant() == 0.0 {
        Mat4::IDENTITY
    } else {
        m.inverse()
    }
}

/// `invert_m3_m3` (identity when singular).
pub fn invert_m3(m: &Mat3) -> Mat3 {
    if m.determinant() == 0.0 {
        Mat3::IDENTITY
    } else {
        m.inverse()
    }
}

/// `orthogonalize_stable(v1, v2, v3, normalize)`.
fn orthogonalize_stable(v1: &mut Vec3, v2: &mut Vec3, v3: &mut Vec3, normalize_axes: bool) {
    let len_sq_v1 = v1.length_squared();
    if len_sq_v1 > 0.0 {
        *v2 += *v1 * (-v2.dot(*v1) / len_sq_v1);
        *v3 += *v1 * (-v3.dot(*v1) / len_sq_v1);
        if normalize_axes {
            *v1 *= 1.0 / len_sq_v1.sqrt();
        }
    }
    let (mut norm_v2, length_v2) = normalize(*v2);
    let (mut norm_v3, length_v3) = normalize(*v3);
    let cos_angle = norm_v2.dot(norm_v3);
    let abs_cos_angle = cos_angle.abs();
    if abs_cos_angle > 1e-4 && abs_cos_angle < 1.0 - f32::EPSILON {
        let angle = cos_angle.acos();
        let target_angle = angle + (std::f32::consts::FRAC_PI_2 - angle) / 2.0;
        norm_v2 += norm_v3 * -cos_angle;
        norm_v2 *= target_angle.sin() / norm_v2.length();
        norm_v2 += norm_v3 * target_angle.cos();
        let tmp = norm_v2.cross(norm_v3);
        norm_v3 = normalize(tmp.cross(norm_v2)).0;
        if !normalize_axes {
            let scale_fac = angle.sin().sqrt();
            *v2 = norm_v2 * (length_v2 * scale_fac);
            *v3 = norm_v3 * (length_v3 * scale_fac);
        }
    }
    if normalize_axes {
        *v2 = norm_v2;
        *v3 = norm_v3;
    }
}

/// `orthogonalize_m4_stable(R, 1, normalize)` (the Y axis is the primary one).
pub fn orthogonalize_m4_stable_y(m: &Mat4, normalize_axes: bool) -> Mat4 {
    let (mut x, mut y, mut z) = (
        m.x_axis.truncate(),
        m.y_axis.truncate(),
        m.z_axis.truncate(),
    );
    orthogonalize_stable(&mut y, &mut x, &mut z, normalize_axes);
    Mat4::from_cols(
        x.extend(m.x_axis.w),
        y.extend(m.y_axis.w),
        z.extend(m.z_axis.w),
        m.w_axis,
    )
}

/// `normalize_m4_ex`: unit axes and their former lengths.
pub fn normalize_m4_ex(m: &Mat4) -> (Mat4, Vec3) {
    let mut cols = [m.x_axis, m.y_axis, m.z_axis];
    let mut scale = Vec3::ZERO;
    for (i, c) in cols.iter_mut().enumerate() {
        let (n, len) = normalize(c.truncate());
        scale[i] = len;
        let w = if len != 0.0 { c.w / len } else { c.w };
        *c = n.extend(w);
    }
    (Mat4::from_cols(cols[0], cols[1], cols[2], m.w_axis), scale)
}

/// `normalize_m4` (the lengths dropped).
pub fn normalize_m4(m: &Mat4) -> Mat4 {
    normalize_m4_ex(m).0
}

/// `mat4_to_size`.
pub fn mat4_to_size(m: &Mat4) -> Vec3 {
    Vec3::new(
        m.x_axis.truncate().length(),
        m.y_axis.truncate().length(),
        m.z_axis.truncate().length(),
    )
}

/// `mat4_to_volume_scale` (`determinant_m4_mat3_array`).
pub fn mat4_to_volume_scale(m: &Mat4) -> f32 {
    let c = |i: usize, j: usize| m.col(i)[j];
    c(0, 0) * (c(1, 1) * c(2, 2) - c(1, 2) * c(2, 1))
        - c(1, 0) * (c(0, 1) * c(2, 2) - c(0, 2) * c(2, 1))
        + c(2, 0) * (c(0, 1) * c(1, 2) - c(0, 2) * c(1, 1))
}

/// `mat4_to_size_fix_shear`.
pub fn mat4_to_size_fix_shear(m: &Mat4) -> Vec3 {
    let size = mat4_to_size(m);
    let volume = size.x * size.y * size.z;
    if volume != 0.0 {
        size * (mat4_to_volume_scale(m) / volume).abs().cbrt()
    } else {
        size
    }
}

/// `rescale_m4`.
pub fn rescale_m4(m: &Mat4, scale: Vec3) -> Mat4 {
    Mat4::from_cols(
        (m.x_axis.truncate() * scale.x).extend(m.x_axis.w),
        (m.y_axis.truncate() * scale.y).extend(m.y_axis.w),
        (m.z_axis.truncate() * scale.z).extend(m.z_axis.w),
        m.w_axis,
    )
}

/// `mul_mat3_m4_fl`: scales the 3×3 part.
fn mul_mat3_m4_fl(m: &Mat4, f: f32) -> Mat4 {
    Mat4::from_cols(
        m.x_axis * Vec4::new(f, f, f, 1.0),
        m.y_axis * Vec4::new(f, f, f, 1.0),
        m.z_axis * Vec4::new(f, f, f, 1.0),
        m.w_axis,
    )
}

/// `Bone.inherit_scale` (`eBone_InheritScaleMode`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum InheritScale {
    /// `FULL`
    #[default]
    Full,
    /// `FIX_SHEAR`
    FixShear,
    /// `AVERAGE`
    Average,
    /// `NONE`
    None,
    /// `NONE_LEGACY`
    NoneLegacy,
    /// `ALIGNED`
    Aligned,
}

impl InheritScale {
    /// From the RNA identifier (`FULL` for anything unknown).
    pub fn from_identifier(s: &str) -> InheritScale {
        match s {
            "FIX_SHEAR" => InheritScale::FixShear,
            "AVERAGE" => InheritScale::Average,
            "NONE" => InheritScale::None,
            "NONE_LEGACY" => InheritScale::NoneLegacy,
            "ALIGNED" => InheritScale::Aligned,
            _ => InheritScale::Full,
        }
    }
}

/// The bone flags the parent transform reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ParentFlags {
    /// `use_inherit_rotation` (`!BONE_HINGE`)
    pub inherit_rotation: bool,
    /// `use_local_location` (`!BONE_NO_LOCAL_LOCATION`)
    pub local_location: bool,
    /// `inherit_scale`
    pub inherit_scale: InheritScale,
}

/// `BoneParentTransform`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BoneParentTransform {
    /// `rotscale_mat`
    pub rotscale: Mat4,
    /// `loc_mat`
    pub loc: Mat4,
    /// `post_scale`
    pub post_scale: Vec3,
}

impl BoneParentTransform {
    /// `BKE_bone_parent_transform_clear`.
    pub const IDENTITY: BoneParentTransform = BoneParentTransform {
        rotscale: Mat4::IDENTITY,
        loc: Mat4::IDENTITY,
        post_scale: Vec3::ONE,
    };

    /// `BKE_bone_parent_transform_calc_from_matrices`. `parent` is `(parent_arm_mat,
    /// parent_pose_mat)` for a child bone, `None` for a root bone (then `offs` is its
    /// `arm_mat`).
    pub fn from_matrices(
        flags: ParentFlags,
        offs: &Mat4,
        parent: Option<(&Mat4, &Mat4)>,
    ) -> BoneParentTransform {
        let mut post_scale = Vec3::ONE;
        let Some((parent_arm, parent_pose)) = parent else {
            let rotscale = *offs;
            let loc = if !flags.local_location {
                Mat4::from_translation(loc(offs))
            } else {
                rotscale
            };
            return BoneParentTransform {
                rotscale,
                loc,
                post_scale,
            };
        };
        let use_rotation = flags.inherit_rotation;
        let mode = flags.inherit_scale;
        let full_transform = use_rotation && mode == InheritScale::Full;
        let rotscale = if full_transform {
            *parent_pose * *offs
        } else {
            let mut tmat;
            if use_rotation {
                tmat = *parent_pose;
                match mode {
                    InheritScale::Full | InheritScale::FixShear => {}
                    InheritScale::None | InheritScale::Average => {
                        tmat = orthogonalize_m4_stable_y(&tmat, true);
                    }
                    InheritScale::Aligned => {
                        tmat = orthogonalize_m4_stable_y(&tmat, false);
                        let (n, s) = normalize_m4_ex(&tmat);
                        tmat = n;
                        post_scale = s;
                    }
                    InheritScale::NoneLegacy => tmat = normalize_m4(&tmat),
                }
            } else {
                tmat = *parent_arm;
                match mode {
                    InheritScale::Full => tmat = rescale_m4(&tmat, mat4_to_size(parent_pose)),
                    InheritScale::FixShear => {
                        tmat = rescale_m4(&tmat, mat4_to_size_fix_shear(parent_pose));
                    }
                    InheritScale::Aligned => post_scale = mat4_to_size_fix_shear(parent_pose),
                    InheritScale::None | InheritScale::Average | InheritScale::NoneLegacy => {}
                }
            }
            if mode == InheritScale::Average {
                tmat = mul_mat3_m4_fl(&tmat, mat4_to_volume_scale(parent_pose).abs().cbrt());
            }
            let mut r = tmat * *offs;
            if mode == InheritScale::FixShear {
                r = orthogonalize_m4_stable_y(&r, false);
            }
            r
        };
        let loc_mat = if !flags.local_location {
            let bone_loc = Mat4::from_translation(mul_m4_v3(parent_pose, loc(offs)));
            let tmat4 = m4_from_m3(&m3(parent_pose), Vec3::ZERO);
            bone_loc * tmat4
        } else if !full_transform {
            *parent_pose * *offs
        } else {
            rotscale
        };
        BoneParentTransform {
            rotscale,
            loc: loc_mat,
            post_scale,
        }
    }

    /// `BKE_bone_parent_transform_invert`.
    pub fn inverted(&self) -> BoneParentTransform {
        let inv = |v: f32| if v != 0.0 { 1.0 / v } else { 0.0 };
        BoneParentTransform {
            rotscale: invert_m4(&self.rotscale),
            loc: invert_m4(&self.loc),
            post_scale: Vec3::new(
                inv(self.post_scale.x),
                inv(self.post_scale.y),
                inv(self.post_scale.z),
            ),
        }
    }

    /// `BKE_bone_parent_transform_combine(self, other)`.
    pub fn combine(&self, other: &BoneParentTransform) -> BoneParentTransform {
        BoneParentTransform {
            rotscale: self.rotscale * other.rotscale,
            loc: self.loc * other.loc,
            post_scale: self.post_scale * other.post_scale,
        }
    }

    /// `BKE_bone_parent_transform_apply`.
    pub fn apply(&self, inmat: &Mat4) -> Mat4 {
        let tmploc = loc(inmat);
        let mut out = self.rotscale * *inmat;
        out.w_axis = mul_m4_v3(&self.loc, tmploc).extend(1.0);
        rescale_m4(&out, self.post_scale)
    }
}

/// `BKE_bone_offset_matrix_get`: `bone_mat` placed at the bone's head (parent space) plus the
/// parent's length along Y.
pub fn offset_matrix(bone_mat: &Mat3, head: Vec3, parent_length: f32) -> Mat4 {
    let mut offs = m4_from_m3(bone_mat, head);
    offs.w_axis.y += parent_length;
    offs
}

/// A pose channel's local transform channels (`bPoseChannel` loc/rot/scale).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Channels {
    /// `location`
    pub location: Vec3,
    /// `rotation_mode`
    pub mode: RotationMode,
    /// `rotation_quaternion`
    pub quat: [f32; 4],
    /// `rotation_euler`
    pub euler: [f32; 3],
    /// `rotation_axis_angle` axis
    pub axis: [f32; 3],
    /// `rotation_axis_angle` angle
    pub angle: f32,
    /// `scale`
    pub scale: Vec3,
}

impl Channels {
    /// `BKE_pchan_rot_to_mat3`.
    pub fn rot_to_mat3(&self) -> Mat3 {
        match self.mode {
            RotationMode::Euler(order) => eul_o_to_mat3(self.euler, order),
            RotationMode::AxisAngle => axis_angle_to_mat3(self.axis, self.angle),
            RotationMode::Quaternion => quat_to_mat3(normalize_qt(self.quat).0),
        }
    }

    /// `BKE_pchan_to_mat4`: rotation times scale, plus the location unless the bone is
    /// connected (`BONE_CONNECTED`).
    pub fn to_mat4(&self, connected: bool) -> Mat4 {
        let smat = Mat3::from_diagonal(self.scale);
        let tmat = self.rot_to_mat3() * smat;
        let l = if connected { Vec3::ZERO } else { self.location };
        m4_from_m3(&tmat, l)
    }

    /// `BKE_pchan_mat3_to_rot` (a normalized rotation matrix).
    pub fn mat3_to_rot(&mut self, mat: &Mat3, use_compat: bool) {
        match self.mode {
            RotationMode::Quaternion => self.quat = mat3_normalized_to_quat(mat),
            RotationMode::AxisAngle => {
                let (axis, angle) = quat_to_axis_angle(mat3_normalized_to_quat(mat));
                self.axis = axis;
                self.angle = angle;
            }
            RotationMode::Euler(order) => {
                self.euler = if use_compat {
                    mat3_normalized_to_compatible_eul_o(mat, self.euler, order)
                } else {
                    mat3_normalized_to_eul_o(mat, order)
                };
            }
        }
    }

    /// `BKE_pchan_apply_mat4`: decomposes `mat` into location, rotation (current mode) and
    /// scale.
    pub fn apply_mat4(&mut self, mat: &Mat4, use_compat: bool) {
        let (l, rot, size) = frap_math::matrix::mat4_to_loc_rot_size(mat);
        self.location = l;
        self.scale = size;
        self.mat3_to_rot(&rot, use_compat);
    }
}

/// The Euler order of a rotation mode (XYZ for quaternion and axis-angle).
pub fn euler_order(mode: RotationMode) -> EulerOrder {
    match mode {
        RotationMode::Euler(o) => o,
        _ => EulerOrder::XYZ,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_roll_of_up_vector() {
        // armature_add's bone (0,0,0) → (0,0,1): matrix_local rows (1,0,0), (0,0,-1), (0,1,0).
        let m = vec_roll_to_mat3(Vec3::Z, 0.0);
        assert!(m.x_axis.abs_diff_eq(Vec3::X, 1e-7), "{m:?}");
        assert!(m.y_axis.abs_diff_eq(Vec3::Z, 1e-7), "{m:?}");
        assert!(m.z_axis.abs_diff_eq(-Vec3::Y, 1e-7), "{m:?}");
    }

    #[test]
    fn roll_round_trips() {
        let v = Vec3::new(0.3, -0.2, 0.9).normalize();
        for roll in [-2.0f32, -0.5, 0.0, 0.7, 3.0] {
            let m = vec_roll_to_mat3_normalized(v, roll);
            let r = mat3_vec_to_roll(&m, v);
            assert!((r - roll).abs() < 1e-5, "{roll} {r}");
        }
    }

    #[test]
    fn negative_y_singularity() {
        let m = vec_roll_to_mat3_normalized(-Vec3::Y, 0.0);
        assert!(m.y_axis.abs_diff_eq(-Vec3::Y, 1e-7), "{m:?}");
    }

    #[test]
    fn parent_transform_full_is_product() {
        let flags = ParentFlags {
            inherit_rotation: true,
            local_location: true,
            inherit_scale: InheritScale::Full,
        };
        let pa = Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0));
        let pp = Mat4::from_rotation_z(0.3) * pa;
        let offs = Mat4::from_translation(Vec3::Y);
        let bpt = BoneParentTransform::from_matrices(flags, &offs, Some((&pa, &pp)));
        assert!(bpt.rotscale.abs_diff_eq(pp * offs, 1e-6));
        let inv = bpt.inverted().apply(&bpt.apply(&Mat4::IDENTITY));
        assert!(inv.abs_diff_eq(Mat4::IDENTITY, 1e-5));
    }
}
