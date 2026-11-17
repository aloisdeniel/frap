// SPDX-License-Identifier: GPL-3.0-or-later
//! Small matrix helpers of the transform system with Blender's exact semantics (degenerate
//! inputs included). Ports of `axis_angle_normalized_to_mat3_ex`, `size_to_mat3`,
//! `normalize_m3`, `orthogonalize_m3_zero_axes`, `invert_m3_m3_safe_ortho`,
//! `orthogonalize_m4_stable` (`blenlib/intern/math_matrix_c.cc`, `math_rotation_c.cc`) and of
//! `TransMat3ToSize` (`editors/transform/transform_mode_resize.cc`), Blender 5.2.4,
//! GPL-2.0-or-later. Matrices are `glam::Mat3` (column-major, like Blender's `float m[3][3]`
//! where `m[i]` is column `i`).

use frap_math::matrix::{mat3_to_rot_size, normalize_v3};
use glam::{Mat3, Vec3};

/// `axis_angle_normalized_to_mat3`: rotation by `angle` about `axis` (expected unit length).
/// A zero axis gives `cos(angle) · I`, as in Blender (the transform system relies on that for
/// an XYZ rotation constraint, which has no axis).
pub fn axis_angle_normalized_to_mat3(axis: Vec3, angle: f32) -> Mat3 {
    let (s, c) = (libm::sinf(angle), libm::cosf(angle));
    let ico = 1.0 - c;
    let nsi = axis * s;
    let n_00 = (axis.x * axis.x) * ico;
    let n_01 = (axis.x * axis.y) * ico;
    let n_11 = (axis.y * axis.y) * ico;
    let n_02 = (axis.x * axis.z) * ico;
    let n_12 = (axis.y * axis.z) * ico;
    let n_22 = (axis.z * axis.z) * ico;
    Mat3::from_cols(
        Vec3::new(n_00 + c, n_01 + nsi.z, n_02 - nsi.y),
        Vec3::new(n_01 - nsi.z, n_11 + c, n_12 + nsi.x),
        Vec3::new(n_02 + nsi.y, n_12 - nsi.x, n_22 + c),
    )
}

/// `size_to_mat3`
pub fn size_to_mat3(size: Vec3) -> Mat3 {
    Mat3::from_diagonal(size)
}

/// `normalize_m3`: every column unit length (a zero column stays zero).
pub fn normalize_m3(m: &Mat3) -> Mat3 {
    let n = |v: Vec3| Vec3::from_array(normalize_v3(v.to_array()).0);
    Mat3::from_cols(n(m.x_axis), n(m.y_axis), n(m.z_axis))
}

/// `transform_orientations_create_from_axis` / `normalize_m3` of a 4×4's upper 3×3: the
/// normalized axes of an object's world matrix. Zero axes are rebuilt from the others
/// (`orthogonalize_m3_zero_axes`) so the result stays usable for a zero-scaled object.
pub fn axes_of(m: &Mat3) -> Mat3 {
    let mut a = *m;
    orthogonalize_m3_zero_axes(&mut a, 1.0);
    normalize_m3(&a)
}

fn ortho_v3(v: Vec3) -> Vec3 {
    // `ortho_v3_v3`: a vector orthogonal to `v` (Blender's axis choice).
    let axis = if v.x.abs() > v.y.abs() {
        if v.x.abs() > v.z.abs() { 0 } else { 2 }
    } else if v.y.abs() > v.z.abs() {
        1
    } else {
        2
    };
    match axis {
        0 => Vec3::new(-v.y - v.z, v.x, v.x),
        1 => Vec3::new(v.y, -v.x - v.z, v.y),
        _ => Vec3::new(v.z, v.z, -v.x - v.y),
    }
}

/// `orthogonalize_m3_zero_axes`: rebuilds zero columns from the non-zero ones (each of length
/// `unit_length`). Returns false when no column or every column is zero (nothing to rebuild
/// from), leaving `m` unchanged.
pub fn orthogonalize_m3_zero_axes(m: &mut Mat3, unit_length: f32) -> bool {
    let mut cols = [m.x_axis, m.y_axis, m.z_axis];
    let mut flag = 0u8;
    for (i, c) in cols.iter().enumerate() {
        if *c == Vec3::ZERO {
            flag |= 1 << i;
        }
    }
    const X: u8 = 1;
    const Y: u8 = 2;
    const Z: u8 = 4;
    if flag == 0 || flag == X | Y | Z {
        return false;
    }
    match flag {
        f if f == X | Y => {
            cols[1] = ortho_v3(cols[2]);
            cols[0] = cols[1].cross(cols[2]);
        }
        X => cols[0] = cols[1].cross(cols[2]),
        f if f == Y | Z => {
            cols[2] = ortho_v3(cols[0]);
            cols[1] = cols[2].cross(cols[0]);
        }
        Y => cols[1] = cols[2].cross(cols[0]),
        f if f == Z | X => {
            cols[0] = ortho_v3(cols[1]);
            cols[2] = cols[0].cross(cols[1]);
        }
        _ => cols[2] = cols[0].cross(cols[1]),
    }
    for (i, c) in cols.iter_mut().enumerate() {
        if flag & (1 << i) != 0 {
            let (n, len) = normalize_v3(c.to_array());
            if len == 0.0 {
                *c = Vec3::ZERO;
                c[i] = unit_length;
            } else {
                *c = Vec3::from_array(n) * unit_length;
            }
        }
    }
    *m = Mat3::from_cols(cols[0], cols[1], cols[2]);
    true
}

/// `invert_m3_m3_safe_ortho`: the inverse, or (singular input) the inverse of the matrix with
/// its zero axes rebuilt, or the identity.
pub fn invert_m3_safe_ortho(m: &Mat3) -> Mat3 {
    if m.determinant() != 0.0 {
        return m.inverse();
    }
    let mut t = *m;
    if orthogonalize_m3_zero_axes(&mut t, 1.0) && t.determinant() != 0.0 {
        t.inverse()
    } else {
        Mat3::IDENTITY
    }
}

/// `invert_m3_m3` falling back to the identity for a singular matrix (`invert_m3` leaves the
/// output zeroed in Blender; callers here only use it on invertible matrices).
pub fn invert_m3_or_identity(m: &Mat3) -> Mat3 {
    if m.determinant() != 0.0 {
        m.inverse()
    } else {
        Mat3::IDENTITY
    }
}

/// `TransMat3ToSize`: the per-axis size of `mat` measured along the axes `smat`, with the
/// sign flipped where the rotated axis points away from `smat`'s (resize of objects keeps
/// mirroring this way).
pub fn trans_mat3_to_size(mat: &Mat3, smat: &Mat3) -> Vec3 {
    let (rmat, mut size) = mat3_to_rot_size(mat);
    for i in 0..3 {
        if rmat.col(i).dot(smat.col(i)) < 0.0 {
            size[i] = -size[i];
        }
    }
    size
}

/// `orthogonalize_m4_stable(R, axis = 0, normalize = true)` on the 3×3 part: Y and Z are made
/// orthogonal to X by plane projection (preserving the determinant), then Z to Y, then all
/// three are normalized.
pub fn orthogonalize_stable_x(m: &Mat3) -> Mat3 {
    let (mut v1, mut v2, mut v3) = (m.x_axis, m.y_axis, m.z_axis);
    let len_sq_v1 = v1.length_squared();
    if len_sq_v1 > 0.0 {
        v2 += v1 * (-v2.dot(v1) / len_sq_v1);
        v3 += v1 * (-v3.dot(v1) / len_sq_v1);
        v1 *= 1.0 / len_sq_v1.sqrt();
    }
    let len_sq_v2 = v2.length_squared();
    if len_sq_v2 > 0.0 {
        v3 += v2 * (-v3.dot(v2) / len_sq_v2);
        v2 *= 1.0 / len_sq_v2.sqrt();
    }
    v3 = Vec3::from_array(normalize_v3(v3.to_array()).0);
    Mat3::from_cols(v1, v2, v3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_angle_matches_glam_for_unit_axes() {
        let m = axis_angle_normalized_to_mat3(Vec3::Z, 0.5);
        assert!(m.abs_diff_eq(Mat3::from_rotation_z(0.5), 1e-6));
        let a = Vec3::new(1.0, 2.0, 3.0).normalize();
        let m = axis_angle_normalized_to_mat3(a, -1.2);
        assert!(m.abs_diff_eq(Mat3::from_axis_angle(a, -1.2), 1e-6));
    }

    #[test]
    fn zero_axis_is_a_uniform_scale_by_cos() {
        let m = axis_angle_normalized_to_mat3(Vec3::ZERO, 0.5);
        assert!(m.abs_diff_eq(Mat3::IDENTITY * libm::cosf(0.5), 1e-7));
    }

    #[test]
    fn mat3_to_size_flips_against_reference_axes() {
        // Resize (2, 1, 1) in global space of a cube rotated 0.5 rad about Z: Blender gives
        // scale (1.8195, 1.2998, 1.0) (verified on 5.2.4, the shear is lost).
        let axes = Mat3::from_rotation_z(0.5);
        let s = trans_mat3_to_size(&(size_to_mat3(Vec3::new(2.0, 1.0, 1.0)) * axes), &axes);
        assert!(s.abs_diff_eq(Vec3::new(1.8195, 1.2998, 1.0), 1e-4), "{s:?}");
        let s = trans_mat3_to_size(
            &(size_to_mat3(Vec3::new(2.0, 3.0, -1.0)) * Mat3::IDENTITY),
            &Mat3::IDENTITY,
        );
        assert!(s.abs_diff_eq(Vec3::new(2.0, 3.0, -1.0), 1e-6), "{s:?}");
    }

    #[test]
    fn zero_axes_are_rebuilt() {
        let mut m = Mat3::from_cols(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::ZERO,
        );
        assert!(orthogonalize_m3_zero_axes(&mut m, 1.0));
        assert_eq!(m.z_axis, Vec3::Z);
        let mut m = Mat3::ZERO;
        assert!(!orthogonalize_m3_zero_axes(&mut m, 1.0));
        assert_eq!(invert_m3_safe_ortho(&Mat3::ZERO), Mat3::IDENTITY);
        let s = Mat3::from_diagonal(Vec3::new(2.0, 0.0, 1.0));
        let inv = invert_m3_safe_ortho(&s);
        assert!(inv.abs_diff_eq(Mat3::from_diagonal(Vec3::new(0.5, 1.0, 1.0)), 1e-6));
    }

    #[test]
    fn stable_orthogonalization_keeps_x() {
        let m = Mat3::from_cols(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(1.0, 3.0, 0.0),
            Vec3::new(0.0, 1.0, 4.0),
        );
        let o = orthogonalize_stable_x(&m);
        assert!(o.x_axis.abs_diff_eq(Vec3::X, 1e-6));
        assert!(o.y_axis.abs_diff_eq(Vec3::Y, 1e-6));
        assert!(o.z_axis.abs_diff_eq(Vec3::Z, 1e-6));
        assert!(
            (normalize_m3(&Mat3::from_diagonal(Vec3::new(3.0, 0.0, 2.0))).y_axis).length() == 0.0
        );
        assert!(
            axes_of(&Mat3::from_diagonal(Vec3::new(3.0, 0.0, 2.0)))
                .abs_diff_eq(Mat3::IDENTITY, 1e-6)
        );
    }
}
