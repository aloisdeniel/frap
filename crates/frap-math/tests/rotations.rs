// SPDX-License-Identifier: GPL-3.0-or-later
//! Euler / quaternion / axis-angle / matrix decomposition within 1e-6 of Blender 5.2.4
//! (WP 1.2 acceptance). The fixture is dumped by `tests/blender/dump_rotations.py`; matrices are
//! column-major like Blender's `float m[4][4]`.

use frap_math::euler::{
    EulerOrder, eul_o_to_mat3, eul_o_to_quat, mat3_to_compatible_eul_o, mat3_to_eul_o,
};
use frap_math::matrix::{
    ObjectTransform, RotationMode, RotationValues, decompose_wxyz, rot_mode_change_values,
};
use frap_math::quat::{
    angle_wrap_rad, axis_angle_to_mat3, axis_angle_to_quat, mat4_to_quat, quat_to_axis_angle,
    quat_to_mat3,
};
use frap_math::{Euler, decompose};
use glam::{Mat3, Mat4};

#[allow(clippy::excessive_precision, clippy::type_complexity)] // generated fixture
mod blender {
    include!("data/blender_rotations.rs");
}

const TOL: f32 = 1e-6;

fn order(s: &str) -> EulerOrder {
    EulerOrder::from_blender(s).expect("fixture order")
}

fn close(label: &str, got: &[f32], want: &[f32], tol: f32) {
    assert_eq!(got.len(), want.len(), "{label}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        // Relative for large values (decomposed sizes/locations up to ~10).
        let t = tol * w.abs().max(1.0);
        assert!(
            (g - w).abs() <= t || (g.is_nan() && w.is_nan()),
            "{label}[{i}]: got {g} want {w}\n  got  {got:?}\n  want {want:?}"
        );
    }
}

fn m4(cols: &[f32; 16]) -> Mat4 {
    Mat4::from_cols_slice(cols)
}

#[test]
fn euler_to_matrix_and_quaternion() {
    for (e, o, mat, q) in blender::EULER {
        let o = order(o);
        let got = eul_o_to_mat3(*e, o);
        close(
            &format!("eulO_to_mat3 {e:?} {o:?}"),
            &got.to_cols_array(),
            mat,
            TOL,
        );
        assert_eq!(Euler::new(*e, o).to_matrix(), got);
        close(
            &format!("eulO_to_quat {e:?} {o:?}"),
            &eul_o_to_quat(*e, o),
            q,
            TOL,
        );
    }
}

#[test]
fn matrix_to_euler_quaternion_and_decompose() {
    for (m, o, compat, eul, eul_compat, quat, loc, dq, size) in blender::MATRIX {
        let o = order(o);
        let mat4 = m4(m);
        let mat3 = Mat3::from_mat4(mat4);
        let label = format!("{o:?} {m:?}");
        close(
            &format!("to_euler {label}"),
            &mat3_to_eul_o(&mat3, o),
            eul,
            TOL,
        );
        close(
            &format!("to_euler compat {label}"),
            &mat3_to_compatible_eul_o(&mat3, *compat, o),
            eul_compat,
            TOL,
        );
        assert_eq!(Euler::from_matrix(&mat3, o).angles, mat3_to_eul_o(&mat3, o));
        assert_eq!(
            Euler::from_matrix_compatible(&mat3, o, *compat).angles,
            mat3_to_compatible_eul_o(&mat3, *compat, o)
        );
        close(
            &format!("to_quaternion {label}"),
            &mat4_to_quat(&mat4),
            quat,
            TOL,
        );
        let (l, q, s) = decompose_wxyz(&mat4);
        close(&format!("decompose loc {label}"), &l.to_array(), loc, TOL);
        close(&format!("decompose quat {label}"), &q, dq, TOL);
        close(&format!("decompose size {label}"), &s.to_array(), size, TOL);
        let (gl, gq, gs) = decompose(&mat4);
        assert_eq!((gl, gs), (l, s));
        assert_eq!([gq.w, gq.x, gq.y, gq.z], q);
    }
}

#[test]
fn quaternion_to_matrix_and_axis_angle() {
    for (q, mat, axis, angle) in blender::QUAT {
        close(
            &format!("quat_to_mat3 {q:?}"),
            &quat_to_mat3(*q).to_cols_array(),
            mat,
            TOL,
        );
        let (a, an) = quat_to_axis_angle(*q);
        close(&format!("quat_to_axis_angle {q:?}"), &a, axis, TOL);
        close(
            &format!("quat_to_axis_angle angle {q:?}"),
            &[an],
            &[*angle],
            TOL,
        );
    }
}

#[test]
fn axis_angle_to_quaternion_and_matrix() {
    for (axis, angle, q, mat) in blender::AXIS_ANGLE {
        // `mathutils.Quaternion(axis, angle)` wraps the angle first (`angle_wrap_rad`).
        close(
            &format!("axis_angle_to_quat {axis:?} {angle}"),
            &axis_angle_to_quat(*axis, angle_wrap_rad(*angle)),
            q,
            TOL,
        );
        close(
            &format!("axis_angle_to_mat3 {axis:?} {angle}"),
            &axis_angle_to_mat3(*axis, *angle).to_cols_array(),
            mat,
            TOL,
        );
    }
}

#[test]
fn rotation_mode_change_values() {
    for (from, to, q0, e0, aa0, q1, e1, aa1) in blender::ROTMODE {
        let from_m = RotationMode::from_blender(from).expect("mode");
        let to_m = RotationMode::from_blender(to).expect("mode");
        let mut v = RotationValues {
            quaternion: *q0,
            euler: *e0,
            axis_angle: *aa0,
        };
        rot_mode_change_values(&mut v, from_m, to_m);
        let label = format!("{from} -> {to} q{q0:?} e{e0:?} aa{aa0:?}");
        close(&format!("quat {label}"), &v.quaternion, q1, TOL);
        close(&format!("euler {label}"), &v.euler, e1, TOL);
        close(&format!("axis-angle {label}"), &v.axis_angle, aa1, TOL);
    }
}

#[test]
fn matrix_world_write_decomposition() {
    for (mode, m, loc, eul, quat, aa, scale) in blender::MATRIX_WORLD {
        let mode_v = RotationMode::from_blender(mode).expect("mode");
        let mut t = ObjectTransform {
            rotation_mode: mode_v,
            ..ObjectTransform::default()
        };
        t.apply_mat4(&m4(m), false);
        let label = format!("{mode} {m:?}");
        close(&format!("location {label}"), &t.location, loc, TOL);
        close(&format!("scale {label}"), &t.scale, scale, TOL);
        match mode_v {
            RotationMode::Euler(_) => close(&format!("euler {label}"), &t.rotation.euler, eul, TOL),
            RotationMode::Quaternion => {
                close(&format!("quat {label}"), &t.rotation.quaternion, quat, TOL)
            }
            RotationMode::AxisAngle => close(
                &format!("axis-angle {label}"),
                &t.rotation.axis_angle,
                aa,
                TOL,
            ),
        }
    }
}

/// Bit patterns recorded on macOS arm64 for a fixed set of conversions; Linux and Windows CI
/// must produce the same bits (`libm` everywhere, rule 2.2.12).
#[test]
fn bit_identical_across_platforms() {
    let mut bits = Vec::new();
    for o in EulerOrder::ALL {
        let e = [0.7f32, -1.3, 2.9];
        bits.extend(eul_o_to_mat3(e, o).to_cols_array().map(f32::to_bits));
        bits.extend(eul_o_to_quat(e, o).map(f32::to_bits));
        let m = eul_o_to_mat3([1.1, 0.4, -2.2], o)
            * Mat3::from_diagonal(glam::Vec3::new(1.5, 2.0, 0.5));
        bits.extend(mat3_to_eul_o(&m, o).map(f32::to_bits));
        bits.extend(mat3_to_compatible_eul_o(&m, [6.0, -6.0, 3.0], o).map(f32::to_bits));
    }
    let (axis, angle) = quat_to_axis_angle([0.8, 0.2, -0.3, 0.4]);
    bits.extend(axis.map(f32::to_bits));
    bits.push(angle.to_bits());
    bits.extend(axis_angle_to_quat([1.0, 2.0, 3.0], 1.234).map(f32::to_bits));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in &bits {
        for byte in b.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    assert_eq!(bits.len(), 6 * (9 + 4 + 3 + 3) + 4 + 4);
    assert_eq!(h, PINNED_FNV, "rotation bit patterns changed: {h:#x}");
}

include!("data/pinned_rotation_bits.rs");
