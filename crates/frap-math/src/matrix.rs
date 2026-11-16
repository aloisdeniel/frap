// SPDX-License-Identifier: GPL-3.0-or-later
//! Matrix helpers with Blender's conventions: `mat4_to_loc_rot_size` (shear is lost on
//! decomposition, like Blender), `Matrix.decompose()`, object transforms
//! (`BKE_object_to_mat4`, `BKE_object_apply_mat4`, `BKE_object_mat3_to_rot`) and rotation-mode
//! conversion (`BKE_rotMode_change_values`).
//!
//! Provenance: Blender 5.2.4 (GPL-2.0-or-later): `blenlib/intern/math_matrix_c.cc`
//! (`normalize_m3`, `determinant_m3_array`, `mul_m3_m3m3`, `size_to_mat3`, `mat3_to_rot_size`,
//! `mat4_to_loc_rot_size`, `mat4_decompose`, `loc_eulO_size_to_mat4`, `loc_quat_size_to_mat4`),
//! `blenlib/intern/math_vector_inline.cc` (`normalize_v3_v3_length`),
//! `blenkernel/intern/object.cc` (`BKE_object_rot_to_mat3`, `BKE_object_to_mat3`,
//! `BKE_object_to_mat4`, `BKE_object_mat3_to_rot`, `BKE_object_apply_mat4_ex`) and
//! `blenkernel/intern/armature.cc` (`BKE_rotMode_change_values`). Operation order is kept so
//! results match Blender to the last bit wherever the platform's `libm` agrees with `libm`.
//!
//! Internally a 3×3 matrix is `M3 = [[f32; 3]; 3]` indexed `m[col][row]`, exactly Blender's
//! `float m[3][3]` and `glam::Mat3::to_cols_array_2d`.

use glam::{Mat3, Mat4, Quat, Vec3};

use crate::euler::{
    EulerOrder, eul_o_to_m3, eul_o_to_quat, m3_normalized_to_compatible_eul_o,
    m3_normalized_to_eul_o, quat_to_eul_o,
};
use crate::quat::{
    QUAT_IDENTITY, QuatWxyz, axis_angle_to_eul_o, axis_angle_to_m3, axis_angle_to_quat,
    conjugate_qt, eul_o_to_axis_angle, m3_normalized_to_quat, m3_normalized_to_quat_fast,
    mul_qt_qtqt, normalize_qt, quat_to_axis_angle, quat_to_m3, to_glam,
};

/// Blender's `float m[3][3]`, `m[col][row]`.
pub(crate) type M3 = [[f32; 3]; 3];

const UNIT_M3: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

pub(crate) fn to_m3(m: &Mat3) -> M3 {
    m.to_cols_array_2d()
}

pub(crate) fn from_m3(m: &M3) -> Mat3 {
    Mat3::from_cols_array_2d(m)
}

fn dot_v3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `len_v3`
pub fn len_v3(a: [f32; 3]) -> f32 {
    dot_v3(a, a).sqrt()
}

/// `normalize_v3_v3`: `(unit vector, original length)`; vectors with a squared length
/// ≤ 1e-35 (or NaN) become zero with length 0, as in Blender.
pub fn normalize_v3(a: [f32; 3]) -> ([f32; 3], f32) {
    let d = dot_v3(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let f = 1.0 / d;
        ([a[0] * f, a[1] * f, a[2] * f], d)
    } else {
        ([0.0; 3], 0.0)
    }
}

/// `normalize_m3_m3`: every column normalized.
pub(crate) fn normalize_m3(m: &M3) -> M3 {
    [
        normalize_v3(m[0]).0,
        normalize_v3(m[1]).0,
        normalize_v3(m[2]).0,
    ]
}

/// `determinant_m3_array`
pub(crate) fn determinant_m3(m: &M3) -> f32 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[1][0] * (m[0][1] * m[2][2] - m[0][2] * m[2][1])
        + m[2][0] * (m[0][1] * m[1][2] - m[0][2] * m[1][1])
}

/// `mul_m3_m3m3(R, A, B)`: `R = A · B`.
pub(crate) fn mul_m3_m3m3(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = b[i][0] * a[0][j] + b[i][1] * a[1][j] + b[i][2] * a[2][j];
        }
    }
    r
}

fn size_to_m3(size: [f32; 3]) -> M3 {
    [
        [size[0], 0.0, 0.0],
        [0.0, size[1], 0.0],
        [0.0, 0.0, size[2]],
    ]
}

fn m4_from_m3_loc(m: &M3, loc: [f32; 3]) -> Mat4 {
    Mat4::from_cols_array_2d(&[
        [m[0][0], m[0][1], m[0][2], 0.0],
        [m[1][0], m[1][1], m[1][2], 0.0],
        [m[2][0], m[2][1], m[2][2], 0.0],
        [loc[0], loc[1], loc[2], 1.0],
    ])
}

fn m3_of_m4(m: &Mat4) -> M3 {
    let c = m.to_cols_array_2d();
    [
        [c[0][0], c[0][1], c[0][2]],
        [c[1][0], c[1][1], c[1][2]],
        [c[2][0], c[2][1], c[2][2]],
    ]
}

/// `is_negative_m3`: determinant < 0 (a mirroring transform).
pub fn is_negative_m3(m: &Mat3) -> bool {
    determinant_m3(&to_m3(m)) < 0.0
}

/// `mat3_to_size`: column lengths (always positive).
pub fn mat3_to_size(m: &Mat3) -> Vec3 {
    let m = to_m3(m);
    Vec3::new(len_v3(m[0]), len_v3(m[1]), len_v3(m[2]))
}

/// `mat3_to_rot_size`: unit-column rotation and per-axis size; a mirrored matrix gets a
/// negated rotation and negative sizes, so `rot` is never mirrored.
pub fn mat3_to_rot_size(m: &Mat3) -> (Mat3, Vec3) {
    let (rot, size) = m3_to_rot_size(&to_m3(m));
    (from_m3(&rot), Vec3::from_array(size))
}

fn m3_to_rot_size(m: &M3) -> (M3, [f32; 3]) {
    let (r0, s0) = normalize_v3(m[0]);
    let (r1, s1) = normalize_v3(m[1]);
    let (r2, s2) = normalize_v3(m[2]);
    let mut rot = [r0, r1, r2];
    let mut size = [s0, s1, s2];
    if determinant_m3(&rot) < 0.0 {
        for c in rot.iter_mut() {
            for v in c.iter_mut() {
                *v = -*v;
            }
        }
        for s in size.iter_mut() {
            *s = -*s;
        }
    }
    (rot, size)
}

/// `mat4_to_loc_rot_size`: location, rotation matrix (unit columns) and size.
pub fn mat4_to_loc_rot_size(m: &Mat4) -> (Vec3, Mat3, Vec3) {
    let (rot, size) = m3_to_rot_size(&m3_of_m4(m));
    (m.w_axis.truncate(), from_m3(&rot), Vec3::from_array(size))
}

/// `Matrix.decompose()` / `mat4_decompose`: location, rotation and scale as Blender
/// decomposes a matrix (shear is lost). The quaternion is a `glam::Quat` (x, y, z, w); see
/// [`decompose_wxyz`] for Blender's w-first layout.
pub fn decompose(m: &Mat4) -> (Vec3, Quat, Vec3) {
    let (loc, q, size) = decompose_wxyz(m);
    (loc, to_glam(q), size)
}

/// [`decompose`] with a w-first quaternion.
pub fn decompose_wxyz(m: &Mat4) -> (Vec3, QuatWxyz, Vec3) {
    let (rot, size) = m3_to_rot_size(&m3_of_m4(m));
    (
        m.w_axis.truncate(),
        m3_normalized_to_quat_fast(&rot),
        Vec3::from_array(size),
    )
}

/// `loc_eulO_size_to_mat4`.
pub fn loc_eul_o_size_to_mat4(loc: Vec3, eul: [f32; 3], order: EulerOrder, size: Vec3) -> Mat4 {
    let t = mul_m3_m3m3(&eul_o_to_m3(eul, order), &size_to_m3(size.to_array()));
    m4_from_m3_loc(&t, loc.to_array())
}

/// `loc_quat_size_to_mat4` (w-first quaternion).
pub fn loc_quat_size_to_mat4(loc: Vec3, quat: QuatWxyz, size: Vec3) -> Mat4 {
    let t = mul_m3_m3m3(&quat_to_m3(quat), &size_to_m3(size.to_array()));
    m4_from_m3_loc(&t, loc.to_array())
}

/// `loc_axisangle_size_to_mat4` (`axis_angle_to_mat3`, then size).
pub fn loc_axis_angle_size_to_mat4(loc: Vec3, axis: [f32; 3], angle: f32, size: Vec3) -> Mat4 {
    let t = mul_m3_m3m3(&axis_angle_to_m3(axis, angle), &size_to_m3(size.to_array()));
    m4_from_m3_loc(&t, loc.to_array())
}

/// `Object.rotation_mode` (`eRotationModes`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RotationMode {
    /// One of the six Euler orders (`ROT_MODE_XYZ` … `ROT_MODE_ZYX`).
    Euler(EulerOrder),
    /// `'QUATERNION'` (`ROT_MODE_QUAT = 0`).
    Quaternion,
    /// `'AXIS_ANGLE'` (`ROT_MODE_AXISANGLE = -1`).
    AxisAngle,
}

impl Default for RotationMode {
    fn default() -> Self {
        RotationMode::Euler(EulerOrder::XYZ)
    }
}

impl RotationMode {
    /// Blender's identifier (`'XYZ'`, `'QUATERNION'`, `'AXIS_ANGLE'`).
    pub const fn as_str(self) -> &'static str {
        match self {
            RotationMode::Euler(o) => o.as_str(),
            RotationMode::Quaternion => "QUATERNION",
            RotationMode::AxisAngle => "AXIS_ANGLE",
        }
    }
    /// Parses Blender's identifier.
    pub fn from_blender(s: &str) -> Option<Self> {
        match s {
            "QUATERNION" => Some(RotationMode::Quaternion),
            "AXIS_ANGLE" => Some(RotationMode::AxisAngle),
            _ => EulerOrder::from_blender(s).map(RotationMode::Euler),
        }
    }
}

/// The rotation channels of an object or pose bone.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RotationValues {
    /// `rotation_quaternion` (w-first)
    pub quaternion: QuatWxyz,
    /// `rotation_euler`
    pub euler: [f32; 3],
    /// `rotation_axis_angle` as RNA shows it: `(angle, x, y, z)`
    pub axis_angle: [f32; 4],
}

/// `BKE_rotMode_change_values`: converts the stored rotation when `rotation_mode` changes
/// (Euler → Euler of another order converts nothing, as in Blender).
pub fn rot_mode_change_values(v: &mut RotationValues, old: RotationMode, new: RotationMode) {
    let axis = [v.axis_angle[1], v.axis_angle[2], v.axis_angle[3]];
    let angle = v.axis_angle[0];
    match new {
        RotationMode::Euler(order) => match old {
            RotationMode::AxisAngle => v.euler = axis_angle_to_eul_o(axis, angle, order),
            RotationMode::Quaternion => {
                v.quaternion = normalize_qt(v.quaternion).0;
                v.euler = quat_to_eul_o(v.quaternion, order);
            }
            RotationMode::Euler(_) => {}
        },
        RotationMode::Quaternion => match old {
            RotationMode::AxisAngle => v.quaternion = axis_angle_to_quat(axis, angle),
            RotationMode::Euler(order) => v.quaternion = eul_o_to_quat(v.euler, order),
            RotationMode::Quaternion => {}
        },
        RotationMode::AxisAngle => {
            let (mut axis, mut angle) = (axis, angle);
            match old {
                RotationMode::Euler(order) => (axis, angle) = eul_o_to_axis_angle(v.euler, order),
                RotationMode::Quaternion => {
                    v.quaternion = normalize_qt(v.quaternion).0;
                    (axis, angle) = quat_to_axis_angle(v.quaternion);
                }
                RotationMode::AxisAngle => {}
            }
            // IS_EQF: no axis, rotate around Y (the roll).
            if (axis[0] - axis[1]).abs() < f32::EPSILON && (axis[1] - axis[2]).abs() < f32::EPSILON
            {
                axis[1] = 1.0;
            }
            v.axis_angle = [angle, axis[0], axis[1], axis[2]];
        }
    }
}

/// The local transform channels of an object (`Object.location`, rotations, `scale` and the
/// delta channels), with Blender's composition and decomposition rules.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ObjectTransform {
    /// `location`
    pub location: [f32; 3],
    /// `rotation_mode`
    pub rotation_mode: RotationMode,
    /// Rotation channels (all kept, whatever the mode, as Blender does).
    pub rotation: RotationValues,
    /// `scale`
    pub scale: [f32; 3],
    /// `delta_location`
    pub delta_location: [f32; 3],
    /// `delta_rotation_euler`
    pub delta_rotation_euler: [f32; 3],
    /// `delta_rotation_quaternion` (w-first)
    pub delta_rotation_quaternion: QuatWxyz,
    /// `delta_scale`
    pub delta_scale: [f32; 3],
}

impl Default for ObjectTransform {
    /// Blender's fresh-object values (`DNA_object_defaults.h`).
    fn default() -> Self {
        ObjectTransform {
            location: [0.0; 3],
            rotation_mode: RotationMode::default(),
            rotation: RotationValues {
                quaternion: QUAT_IDENTITY,
                euler: [0.0; 3],
                axis_angle: [0.0, 0.0, 1.0, 0.0],
            },
            scale: [1.0; 3],
            delta_location: [0.0; 3],
            delta_rotation_euler: [0.0; 3],
            delta_rotation_quaternion: QUAT_IDENTITY,
            delta_scale: [1.0; 3],
        }
    }
}

impl ObjectTransform {
    /// `rna_Object_rotation_mode_set`: converts the stored rotation, then switches mode.
    pub fn set_rotation_mode(&mut self, new: RotationMode) {
        rot_mode_change_values(&mut self.rotation, self.rotation_mode, new);
        self.rotation_mode = new;
    }

    fn rot_to_m3(&self, use_drot: bool) -> M3 {
        let (rmat, dmat) = match self.rotation_mode {
            RotationMode::Euler(order) => (
                eul_o_to_m3(self.rotation.euler, order),
                eul_o_to_m3(self.delta_rotation_euler, order),
            ),
            RotationMode::AxisAngle => {
                let a = self.rotation.axis_angle;
                // RNA exposes no delta axis-angle; DNA's default (angle 0) is the identity.
                (axis_angle_to_m3([a[1], a[2], a[3]], a[0]), UNIT_M3)
            }
            RotationMode::Quaternion => (
                quat_to_m3(normalize_qt(self.rotation.quaternion).0),
                quat_to_m3(normalize_qt(self.delta_rotation_quaternion).0),
            ),
        };
        if use_drot {
            mul_m3_m3m3(&dmat, &rmat)
        } else {
            rmat
        }
    }

    /// `BKE_object_rot_to_mat3`.
    pub fn rot_to_mat3(&self, use_drot: bool) -> Mat3 {
        from_m3(&self.rot_to_m3(use_drot))
    }

    /// `BKE_object_to_mat3`: rotation (with deltas) times scale (with delta scale).
    pub fn to_mat3(self) -> Mat3 {
        from_m3(&self.basis_m3())
    }

    fn basis_m3(&self) -> M3 {
        let s = [
            self.scale[0] * self.delta_scale[0],
            self.scale[1] * self.delta_scale[1],
            self.scale[2] * self.delta_scale[2],
        ];
        mul_m3_m3m3(&self.rot_to_m3(true), &size_to_m3(s))
    }

    /// `BKE_object_to_mat4` (`matrix_basis`).
    pub fn to_mat4(self) -> Mat4 {
        let loc = [
            self.location[0] + self.delta_location[0],
            self.location[1] + self.delta_location[1],
            self.location[2] + self.delta_location[2],
        ];
        m4_from_m3_loc(&self.basis_m3(), loc)
    }

    /// `BKE_object_mat3_to_rot`: writes a normalized rotation matrix into the channel of the
    /// current mode, removing the delta rotation.
    pub fn mat3_to_rot(&mut self, mat: &Mat3, use_compat: bool) {
        self.m3_to_rot(&to_m3(mat), use_compat);
    }

    fn m3_to_rot(&mut self, mat: &M3, use_compat: bool) {
        match self.rotation_mode {
            RotationMode::Quaternion => {
                let q = m3_normalized_to_quat(mat);
                let dquat = conjugate_qt(normalize_qt(self.delta_rotation_quaternion).0);
                self.rotation.quaternion = mul_qt_qtqt(dquat, q);
            }
            RotationMode::AxisAngle => {
                let q = m3_normalized_to_quat(mat);
                let dquat = conjugate_qt(QUAT_IDENTITY);
                let q = mul_qt_qtqt(dquat, q);
                let (axis, angle) = quat_to_axis_angle(q);
                self.rotation.axis_angle = [angle, axis[0], axis[1], axis[2]];
            }
            RotationMode::Euler(order) => {
                let q = m3_normalized_to_quat(mat);
                let dquat = conjugate_qt(eul_o_to_quat(self.delta_rotation_euler, order));
                let q = mul_qt_qtqt(dquat, q);
                let qm = quat_to_m3(q);
                self.rotation.euler = if use_compat {
                    m3_normalized_to_compatible_eul_o(&qm, self.rotation.euler, order)
                } else {
                    m3_normalized_to_eul_o(&qm, order)
                };
            }
        }
    }

    /// `BKE_object_apply_mat4_ex` without a parent: decomposes `mat` (shear is lost) into
    /// location, rotation (current mode) and scale, removing the delta channels. Blender writes
    /// `matrix_world`/`matrix_basis` this way (`use_compat = false`).
    pub fn apply_mat4(&mut self, mat: &Mat4, use_compat: bool) {
        let (rot, size) = m3_to_rot_size(&m3_of_m4(mat));
        let loc = mat.w_axis.truncate().to_array();
        self.location = loc;
        self.scale = size;
        self.m3_to_rot(&rot, use_compat);
        for i in 0..3 {
            self.location[i] -= self.delta_location[i];
            if self.delta_scale[i] != 0.0 {
                self.scale[i] /= self.delta_scale[i];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompose_loses_shear_and_keeps_mirror_in_scale() {
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, -2.0, 3.0),
            Quat::from_rotation_z(0.5),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let (loc, q, s) = decompose(&m);
        assert!(loc.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-6));
        // Blender negates all three sizes and the rotation of a mirrored matrix.
        assert!(s.abs_diff_eq(Vec3::new(-1.0, -2.0, -3.0), 1e-5), "{s:?}");
        let back = Mat4::from_scale_rotation_translation(s, q, loc);
        assert!(back.abs_diff_eq(m, 1e-5));
        let (l2, r2, s2) = mat4_to_loc_rot_size(&m);
        assert_eq!(l2, loc);
        assert_eq!(s2, s);
        assert!(!is_negative_m3(&r2));
        assert!(is_negative_m3(&Mat3::from_mat4(m)));
        assert!(mat3_to_size(&Mat3::from_mat4(m)).abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-5));
        let (r3, s3) = mat3_to_rot_size(&Mat3::from_mat4(m));
        assert_eq!((r3, s3), (r2, s2));
    }

    #[test]
    fn compose_matches_glam_for_xyz() {
        let m = loc_eul_o_size_to_mat4(
            Vec3::new(1.0, 2.0, 3.0),
            [0.1, 0.2, 0.3],
            EulerOrder::XYZ,
            Vec3::new(2.0, 2.0, 2.0),
        );
        let r =
            Mat3::from_rotation_z(0.3) * Mat3::from_rotation_y(0.2) * Mat3::from_rotation_x(0.1);
        let g = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0))
            * Mat4::from_mat3(r)
            * Mat4::from_scale(Vec3::splat(2.0));
        assert!(m.abs_diff_eq(g, 1e-6));
        let q = loc_quat_size_to_mat4(Vec3::ZERO, QUAT_IDENTITY, Vec3::ONE);
        assert_eq!(q, Mat4::IDENTITY);
        let a = loc_axis_angle_size_to_mat4(Vec3::ZERO, [0.0, 0.0, 1.0], 0.0, Vec3::ONE);
        assert_eq!(a, Mat4::IDENTITY);
    }

    #[test]
    fn rotation_mode_identifiers() {
        for s in ["XYZ", "ZYX", "QUATERNION", "AXIS_ANGLE"] {
            assert_eq!(RotationMode::from_blender(s).map(|m| m.as_str()), Some(s));
        }
        assert_eq!(RotationMode::from_blender("xyz"), None);
    }

    #[test]
    fn object_transform_round_trip() {
        let mut t = ObjectTransform {
            location: [1.0, 2.0, 3.0],
            scale: [1.0, 2.0, 0.5],
            delta_location: [0.5, 0.0, 0.0],
            delta_scale: [2.0, 1.0, 1.0],
            delta_rotation_euler: [0.0, 0.0, 0.25],
            ..ObjectTransform::default()
        };
        t.rotation.euler = [0.3, -0.2, 0.1];
        let m = t.to_mat4();
        let mut u = ObjectTransform {
            delta_location: t.delta_location,
            delta_scale: t.delta_scale,
            delta_rotation_euler: t.delta_rotation_euler,
            ..ObjectTransform::default()
        };
        u.apply_mat4(&m, false);
        for i in 0..3 {
            assert!((u.location[i] - t.location[i]).abs() < 1e-5);
            assert!((u.scale[i] - t.scale[i]).abs() < 1e-5);
            assert!((u.rotation.euler[i] - t.rotation.euler[i]).abs() < 1e-5);
        }
        // Quaternion and axis-angle modes use their own delta channels; drop the Euler one.
        u.delta_rotation_euler = [0.0; 3];
        let m = u.to_mat4();
        u.set_rotation_mode(RotationMode::Quaternion);
        let mq = u.to_mat4();
        assert!(mq.abs_diff_eq(m, 1e-5));
        u.set_rotation_mode(RotationMode::AxisAngle);
        assert!(u.to_mat4().abs_diff_eq(m, 1e-5));
        assert_eq!(u.rotation_mode, RotationMode::AxisAngle);
    }
}
