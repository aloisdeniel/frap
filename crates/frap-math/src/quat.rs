// SPDX-License-Identifier: GPL-3.0-or-later
//! Quaternions, w-first at every API boundary (`rotation_quaternion = (w, x, y, z)`), and
//! axis-angle (`rotation_axis_angle = (angle, x, y, z)`).
//!
//! Provenance: Blender 5.2.4, `source/blender/blenlib/intern/math_rotation_c.cc`
//! (GPL-2.0-or-later): `quat_to_mat3`, `mat3_normalized_to_quat_fast`,
//! `mat3_normalized_to_quat`, `mat3_to_quat`, `mat4_to_quat`, `normalize_qt`, `mul_qt_qtqt`,
//! `axis_angle_to_quat`, `quat_to_axis_angle`, `axis_angle_to_mat3`, `axis_angle_to_eulO`,
//! `eulO_to_axis_angle`. Operation order and the f32/f64 mix are kept; transcendentals come
//! from `libm` (rule 2.2.12).

use glam::{Mat3, Mat4, Quat};

use crate::euler::{EulerOrder, eul_o_to_quat, quat_to_eul_o};
use crate::matrix::{M3, determinant_m3, from_m3, normalize_m3, normalize_v3, to_m3};

/// A w-first quaternion `(w, x, y, z)`, Blender's layout.
pub type QuatWxyz = [f32; 4];

/// The identity quaternion `(1, 0, 0, 0)`.
pub const QUAT_IDENTITY: QuatWxyz = [1.0, 0.0, 0.0, 0.0];

/// w-first → `glam::Quat` (x, y, z, w).
pub fn to_glam(q: QuatWxyz) -> Quat {
    Quat::from_xyzw(q[1], q[2], q[3], q[0])
}

/// `glam::Quat` → w-first.
pub fn from_glam(q: Quat) -> QuatWxyz {
    [q.w, q.x, q.y, q.z]
}

/// `dot_qtqt`
pub fn dot_qtqt(a: QuatWxyz, b: QuatWxyz) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// `mul_qt_fl`
pub fn mul_qt_fl(q: QuatWxyz, f: f32) -> QuatWxyz {
    [q[0] * f, q[1] * f, q[2] * f, q[3] * f]
}

/// `normalize_qt`: returns the normalized quaternion and the original length; a zero
/// quaternion becomes `(0, 1, 0, 0)`, as in Blender.
pub fn normalize_qt(q: QuatWxyz) -> (QuatWxyz, f32) {
    let len = dot_qtqt(q, q).sqrt();
    if len != 0.0 {
        (mul_qt_fl(q, 1.0 / len), len)
    } else {
        ([0.0, 1.0, 0.0, 0.0], len)
    }
}

/// `mul_qt_qtqt`: `a * b`.
pub fn mul_qt_qtqt(a: QuatWxyz, b: QuatWxyz) -> QuatWxyz {
    let t0 = a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3];
    let t1 = a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2];
    let t2 = a[0] * b[2] + a[2] * b[0] + a[3] * b[1] - a[1] * b[3];
    let t3 = a[0] * b[3] + a[3] * b[0] + a[1] * b[2] - a[2] * b[1];
    [t0, t1, t2, t3]
}

/// `conjugate_qt` (= `invert_qt_normalized` for unit quaternions).
pub fn conjugate_qt(q: QuatWxyz) -> QuatWxyz {
    [q[0], -q[1], -q[2], -q[3]]
}

/// `invert_qt`: conjugate divided by the squared length (unchanged when zero).
pub fn invert_qt(q: QuatWxyz) -> QuatWxyz {
    let f = dot_qtqt(q, q);
    if f == 0.0 {
        return q;
    }
    mul_qt_fl(conjugate_qt(q), 1.0 / f)
}

/// `quat_to_mat3` (double precision inside; `q` is expected to be normalized).
pub fn quat_to_mat3(q: QuatWxyz) -> Mat3 {
    from_m3(&quat_to_m3(q))
}

pub(crate) fn quat_to_m3(q: QuatWxyz) -> M3 {
    let s2 = std::f64::consts::SQRT_2;
    let q0 = s2 * f64::from(q[0]);
    let q1 = s2 * f64::from(q[1]);
    let q2 = s2 * f64::from(q[2]);
    let q3 = s2 * f64::from(q[3]);

    let qda = q0 * q1;
    let qdb = q0 * q2;
    let qdc = q0 * q3;
    let qaa = q1 * q1;
    let qab = q1 * q2;
    let qac = q1 * q3;
    let qbb = q2 * q2;
    let qbc = q2 * q3;
    let qcc = q3 * q3;

    [
        [
            (1.0 - qbb - qcc) as f32,
            (qdc + qab) as f32,
            (-qdb + qac) as f32,
        ],
        [
            (-qdc + qab) as f32,
            (1.0 - qaa - qcc) as f32,
            (qda + qbc) as f32,
        ],
        [
            (qdb + qac) as f32,
            (-qda + qbc) as f32,
            (1.0 - qaa - qbb) as f32,
        ],
    ]
}

/// `quat_to_mat4`.
pub fn quat_to_mat4(q: QuatWxyz) -> Mat4 {
    Mat4::from_mat3(quat_to_mat3(q))
}

/// `mat3_normalized_to_quat_fast`: Mike Day's method; `m` must be a non-negative rotation
/// with unit columns. W is kept non-negative.
pub fn mat3_normalized_to_quat_fast(m: &Mat3) -> QuatWxyz {
    m3_normalized_to_quat_fast(&to_m3(m))
}

pub(crate) fn m3_normalized_to_quat_fast(mat: &M3) -> QuatWxyz {
    let mut q = [0.0f32; 4];
    if mat[2][2] < 0.0 {
        if mat[0][0] > mat[1][1] {
            let trace = 1.0 + mat[0][0] - mat[1][1] - mat[2][2];
            let mut s = 2.0 * trace.sqrt();
            if mat[1][2] < mat[2][1] {
                s = -s;
            }
            q[1] = 0.25 * s;
            s = 1.0 / s;
            q[0] = (mat[1][2] - mat[2][1]) * s;
            q[2] = (mat[0][1] + mat[1][0]) * s;
            q[3] = (mat[2][0] + mat[0][2]) * s;
            if trace == 1.0 && q[0] == 0.0 && q[2] == 0.0 && q[3] == 0.0 {
                q[1] = 1.0;
            }
        } else {
            let trace = 1.0 - mat[0][0] + mat[1][1] - mat[2][2];
            let mut s = 2.0 * trace.sqrt();
            if mat[2][0] < mat[0][2] {
                s = -s;
            }
            q[2] = 0.25 * s;
            s = 1.0 / s;
            q[0] = (mat[2][0] - mat[0][2]) * s;
            q[1] = (mat[0][1] + mat[1][0]) * s;
            q[3] = (mat[1][2] + mat[2][1]) * s;
            if trace == 1.0 && q[0] == 0.0 && q[1] == 0.0 && q[3] == 0.0 {
                q[2] = 1.0;
            }
        }
    } else if mat[0][0] < -mat[1][1] {
        let trace = 1.0 - mat[0][0] - mat[1][1] + mat[2][2];
        let mut s = 2.0 * trace.sqrt();
        if mat[0][1] < mat[1][0] {
            s = -s;
        }
        q[3] = 0.25 * s;
        s = 1.0 / s;
        q[0] = (mat[0][1] - mat[1][0]) * s;
        q[1] = (mat[2][0] + mat[0][2]) * s;
        q[2] = (mat[1][2] + mat[2][1]) * s;
        if trace == 1.0 && q[0] == 0.0 && q[1] == 0.0 && q[2] == 0.0 {
            q[3] = 1.0;
        }
    } else {
        // A zero matrix falls through to here (no rotation, Blender #101848).
        let trace = 1.0 + mat[0][0] + mat[1][1] + mat[2][2];
        let mut s = 2.0 * trace.sqrt();
        q[0] = 0.25 * s;
        s = 1.0 / s;
        q[1] = (mat[1][2] - mat[2][1]) * s;
        q[2] = (mat[2][0] - mat[0][2]) * s;
        q[3] = (mat[0][1] - mat[1][0]) * s;
        if trace == 1.0 && q[1] == 0.0 && q[2] == 0.0 && q[3] == 0.0 {
            q[0] = 1.0;
        }
    }
    // Normalize only when round-off made it necessary (`BLI_ASSERT_UNIT_EPSILON * 3`).
    let q_len_squared = dot_qtqt(q, q);
    let threshold = 0.0002f32 * 3.0;
    if (q_len_squared - 1.0).abs() >= threshold {
        q = normalize_qt(q).0;
    }
    q
}

fn m3_normalized_to_quat_with_checks(mut m: M3) -> QuatWxyz {
    let det = determinant_m3(&m);
    if !det.is_finite() {
        m = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    } else if det < 0.0 {
        for c in m.iter_mut() {
            for v in c.iter_mut() {
                *v = -*v;
            }
        }
    }
    m3_normalized_to_quat_fast(&m)
}

/// `mat3_normalized_to_quat`: handles negative (mirrored) and non-finite matrices.
pub fn mat3_normalized_to_quat(m: &Mat3) -> QuatWxyz {
    m3_normalized_to_quat_with_checks(to_m3(m))
}

pub(crate) fn m3_normalized_to_quat(m: &M3) -> QuatWxyz {
    m3_normalized_to_quat_with_checks(*m)
}

/// `mat3_to_quat`: normalizes the columns first (`Matrix.to_quaternion()` on a 3×3).
pub fn mat3_to_quat(m: &Mat3) -> QuatWxyz {
    m3_normalized_to_quat_with_checks(normalize_m3(&to_m3(m)))
}

/// `mat4_to_quat`: upper 3×3, normalized (`Matrix.to_quaternion()` on a 4×4).
pub fn mat4_to_quat(m: &Mat4) -> QuatWxyz {
    m3_normalized_to_quat_with_checks(normalize_m3(&to_m3(&Mat3::from_mat4(*m))))
}

/// `angle_wrap_rad`: wraps an angle into [-π, π) (`mod_inline(a + π, 2π) - π`, f32 math).
/// `mathutils.Quaternion(axis, angle)` wraps its angle with it before `axis_angle_to_quat`.
pub fn angle_wrap_rad(angle: f32) -> f32 {
    let pi = std::f64::consts::PI as f32;
    let a = angle + pi;
    let b = pi * 2.0;
    (a - (b * (a / b).floor())) - pi
}

/// `axis_angle_normalized_to_quat` (`axis` must be unit length).
pub fn axis_angle_normalized_to_quat(axis: [f32; 3], angle: f32) -> QuatWxyz {
    let phi = 0.5 * angle;
    let si = libm::sinf(phi);
    let co = libm::cosf(phi);
    [co, axis[0] * si, axis[1] * si, axis[2] * si]
}

/// `axis_angle_to_quat`: a zero axis gives the identity.
pub fn axis_angle_to_quat(axis: [f32; 3], angle: f32) -> QuatWxyz {
    let (nor, len) = normalize_v3(axis);
    if len != 0.0 {
        axis_angle_normalized_to_quat(nor, angle)
    } else {
        QUAT_IDENTITY
    }
}

/// `quat_to_axis_angle`: `(axis, angle)`; a zero axis becomes `(0, 1, 0)`.
pub fn quat_to_axis_angle(q: QuatWxyz) -> ([f32; 3], f32) {
    let ha = libm::acosf(q[0]);
    let mut si = libm::sinf(ha);
    let angle = ha * 2.0;
    if si.abs() < 0.0005 {
        si = 1.0;
    }
    let mut axis = [q[1] / si, q[2] / si, q[3] / si];
    if axis == [0.0; 3] {
        axis[1] = 1.0;
    }
    (axis, angle)
}

/// `axis_angle_normalized_to_mat3_ex`.
pub(crate) fn axis_angle_normalized_to_m3(axis: [f32; 3], angle_sin: f32, angle_cos: f32) -> M3 {
    let ico = 1.0 - angle_cos;
    let nsi = [
        axis[0] * angle_sin,
        axis[1] * angle_sin,
        axis[2] * angle_sin,
    ];
    let n_00 = (axis[0] * axis[0]) * ico;
    let n_01 = (axis[0] * axis[1]) * ico;
    let n_11 = (axis[1] * axis[1]) * ico;
    let n_02 = (axis[0] * axis[2]) * ico;
    let n_12 = (axis[1] * axis[2]) * ico;
    let n_22 = (axis[2] * axis[2]) * ico;
    [
        [n_00 + angle_cos, n_01 + nsi[2], n_02 - nsi[1]],
        [n_01 - nsi[2], n_11 + angle_cos, n_12 + nsi[0]],
        [n_02 + nsi[1], n_12 - nsi[0], n_22 + angle_cos],
    ]
}

/// `axis_angle_to_mat3`: a zero axis gives the identity.
pub fn axis_angle_to_mat3(axis: [f32; 3], angle: f32) -> Mat3 {
    from_m3(&axis_angle_to_m3(axis, angle))
}

pub(crate) fn axis_angle_to_m3(axis: [f32; 3], angle: f32) -> M3 {
    let (nor, len) = normalize_v3(axis);
    if len == 0.0 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    axis_angle_normalized_to_m3(nor, libm::sinf(angle), libm::cosf(angle))
}

/// `axis_angle_to_eulO` (through a quaternion, as Blender does).
pub fn axis_angle_to_eul_o(axis: [f32; 3], angle: f32, order: EulerOrder) -> [f32; 3] {
    quat_to_eul_o(axis_angle_to_quat(axis, angle), order)
}

/// `eulO_to_axis_angle` (through a quaternion, as Blender does).
pub fn eul_o_to_axis_angle(e: [f32; 3], order: EulerOrder) -> ([f32; 3], f32) {
    quat_to_axis_angle(eul_o_to_quat(e, order))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glam_layout() {
        let q = [0.5f32, 0.1, 0.2, 0.3];
        let g = to_glam(q);
        assert_eq!((g.w, g.x, g.y, g.z), (0.5, 0.1, 0.2, 0.3));
        assert_eq!(from_glam(g), q);
    }

    #[test]
    fn matrix_round_trip() {
        let q = normalize_qt([0.3, -0.5, 0.2, 0.7]).0;
        let m = quat_to_mat3(q);
        let back = mat3_normalized_to_quat(&m);
        for i in 0..4 {
            assert!((back[i] - q[i]).abs() < 1e-6, "{back:?} {q:?}");
        }
        let g = Mat3::from_quat(to_glam(q));
        assert!(m.abs_diff_eq(g, 1e-6));
    }

    #[test]
    fn negative_and_degenerate_matrices() {
        let m = Mat3::from_diagonal(glam::Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(mat3_normalized_to_quat(&m), QUAT_IDENTITY);
        assert_eq!(mat3_normalized_to_quat(&Mat3::ZERO), QUAT_IDENTITY);
        let nan = Mat3::from_diagonal(glam::Vec3::splat(f32::NAN));
        assert_eq!(mat3_normalized_to_quat(&nan), QUAT_IDENTITY);
    }

    #[test]
    fn axis_angle_conventions() {
        assert_eq!(axis_angle_to_quat([0.0; 3], 1.0), QUAT_IDENTITY);
        let (axis, angle) = quat_to_axis_angle(QUAT_IDENTITY);
        assert_eq!((axis, angle), ([0.0, 1.0, 0.0], 0.0));
        let q = axis_angle_to_quat([0.0, 0.0, 2.0], std::f32::consts::FRAC_PI_2);
        let (axis, angle) = quat_to_axis_angle(q);
        assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((axis[2] - 1.0).abs() < 1e-6);
        let m = axis_angle_to_mat3([0.0, 0.0, 1.0], 0.5);
        assert!(m.abs_diff_eq(Mat3::from_rotation_z(0.5), 1e-6));
        assert_eq!(axis_angle_to_mat3([0.0; 3], 0.5), Mat3::IDENTITY);
        assert!((angle_wrap_rad(4.0) - (4.0 - 2.0 * std::f32::consts::PI)).abs() < 1e-6);
        assert_eq!(angle_wrap_rad(0.5), 0.5);
        let e = axis_angle_to_eul_o([1.0, 0.0, 0.0], 0.25, EulerOrder::XYZ);
        assert!((e[0] - 0.25).abs() < 1e-6);
        let (ax, an) = eul_o_to_axis_angle([0.0, 0.0, 0.5], EulerOrder::ZYX);
        assert!((an - 0.5).abs() < 1e-6 && (ax[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn algebra() {
        let a = normalize_qt([0.9, 0.1, 0.3, -0.2]).0;
        let id = mul_qt_qtqt(a, conjugate_qt(a));
        assert!((id[0] - 1.0).abs() < 1e-6 && id[1].abs() < 1e-6);
        let inv = invert_qt([2.0, 0.0, 0.0, 0.0]);
        assert_eq!(inv, [0.5, -0.0, -0.0, -0.0]);
        assert_eq!(invert_qt([0.0; 4]), [0.0; 4]);
        assert_eq!(normalize_qt([0.0; 4]), ([0.0, 1.0, 0.0, 0.0], 0.0));
        assert_eq!(quat_to_mat4(QUAT_IDENTITY), Mat4::IDENTITY);
        assert_eq!(
            mat4_to_quat(&Mat4::from_scale(glam::Vec3::splat(3.0))),
            QUAT_IDENTITY
        );
        assert_eq!(
            mat3_to_quat(&Mat3::from_diagonal(glam::Vec3::splat(2.0))),
            QUAT_IDENTITY
        );
    }
}
