// SPDX-License-Identifier: GPL-3.0-or-later
//! The BLI math the primitive operators use, ported with Blender's evaluation order
//! (`blender/source/blender/blenlib/intern/math_matrix_c.cc`, `math_rotation_c.cc`,
//! `math_base_inline.cc`). Matrices are `float[4][4]` / `float[3][3]` in Blender's layout:
//! `m[col][row]`, translation in `m[3]`. Transcendentals go through `libm` (rule 2.2.12).

use super::V3;
use crate::primitives::Matrix4;

pub(crate) type Matrix3 = [[f32; 3]; 3];

/// `unit_m4`.
pub(crate) fn unit_m4() -> Matrix4 {
    let mut m = [[0.0f32; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

/// `mul_m4_v3(M, r)`: `x*M[0] + y*M[1] + M[2]*z + M[3]` per component, left to right.
#[inline]
pub(crate) fn mul_m4_v3(m: &Matrix4, r: V3) -> V3 {
    let (x, y, z) = (r[0], r[1], r[2]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * z + m[3][0],
        x * m[0][1] + y * m[1][1] + m[2][1] * z + m[3][1],
        x * m[0][2] + y * m[1][2] + m[2][2] * z + m[3][2],
    ]
}

/// `mul_mat3_m4_v3(M, r)`: rotation/scale part only.
#[inline]
pub(crate) fn mul_mat3_m4_v3(m: &Matrix4, r: V3) -> V3 {
    let (x, y, z) = (r[0], r[1], r[2]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * z,
        x * m[0][1] + y * m[1][1] + m[2][1] * z,
        x * m[0][2] + y * m[1][2] + m[2][2] * z,
    ]
}

/// `mul_v3_m3v3(r, M, a)`.
#[inline]
pub(crate) fn mul_v3_m3v3(m: &Matrix3, a: V3) -> V3 {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
        m[0][2] * a[0] + m[1][2] * a[1] + m[2][2] * a[2],
    ]
}

/// `mul_m4_m4m4(R, A, B)` = `A * B`, with the SSE2 path's summation order
/// (`(B0*A0 + B1*A1) + (B2*A2 + B3*A3)`, the path Blender's x86 and arm64 (sse2neon) builds
/// take).
pub(crate) fn mul_m4_m4m4(a: &Matrix4, b: &Matrix4) -> Matrix4 {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for k in 0..4 {
            r[i][k] =
                (b[i][0] * a[0][k] + b[i][1] * a[1][k]) + (b[i][2] * a[2][k] + b[i][3] * a[3][k]);
        }
    }
    r
}

/// `copy_m4_m3` onto a unit matrix.
pub(crate) fn m4_from_m3(m: &Matrix3) -> Matrix4 {
    let mut r = unit_m4();
    for c in 0..3 {
        for k in 0..3 {
            r[c][k] = m[c][k];
        }
    }
    r
}

/// `transform_pivot_set_m4(mat, pivot)`: `T(pivot) * mat * T(-pivot)`.
pub(crate) fn transform_pivot_set_m4(mat: &Matrix4, pivot: V3) -> Matrix4 {
    let mut t = unit_m4();
    t[3][0] = pivot[0];
    t[3][1] = pivot[1];
    t[3][2] = pivot[2];
    let m = mul_m4_m4m4(&t, mat);
    t[3][0] = -pivot[0];
    t[3][1] = -pivot[1];
    t[3][2] = -pivot[2];
    mul_m4_m4m4(&m, &t)
}

/// `invert_m4_m4` (Blender uses Eigen; frap inverts in f64 by Gauss-Jordan with partial
/// pivoting and rounds, which agrees to the last bit for the scale/rotation matrices
/// primitives use and within 1 ulp otherwise). Returns the zero matrix when singular.
pub(crate) fn invert_m4(m: &Matrix4) -> Matrix4 {
    let mut a = [[0.0f64; 8]; 4];
    // Row-major view: row r, col c = m[c][r].
    for r in 0..4 {
        for c in 0..4 {
            a[r][c] = m[c][r] as f64;
        }
        a[r][4 + r] = 1.0;
    }
    for col in 0..4 {
        let mut piv = col;
        for r in col + 1..4 {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col] == 0.0 {
            return [[0.0; 4]; 4];
        }
        a.swap(col, piv);
        let d = a[col][col];
        for x in a[col].iter_mut() {
            *x /= d;
        }
        for r in 0..4 {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    let row = a[col];
                    for (x, y) in a[r].iter_mut().zip(row.iter()) {
                        *x -= f * y;
                    }
                }
            }
        }
    }
    let mut out = [[0.0f32; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            out[c][r] = a[r][4 + c] as f32;
        }
    }
    out
}

/// `mod_inline(a, b)`: `a - b * floorf(a / b)`.
#[inline]
pub(crate) fn mod_inline(a: f32, b: f32) -> f32 {
    a - b * (a / b).floor()
}

/// `angle_wrap_rad(float)`.
#[inline]
pub(crate) fn angle_wrap_rad(angle: f32) -> f32 {
    let pi = std::f32::consts::PI;
    mod_inline(angle + pi, pi * 2.0) - pi
}

/// `axis_angle_normalized_to_mat3_ex(mat, axis, sin, cos)`.
pub(crate) fn axis_angle_normalized_to_mat3(axis: V3, angle: f32) -> Matrix3 {
    let (angle_sin, angle_cos) = (libm::sinf(angle), libm::cosf(angle));
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

// Ported from blender/source/blender/blenlib/intern/math_rotation_c.cc (sin_cos_from_fraction).
/// `sin(2π n/d)` and `cos(2π n/d)` computed in the octant closest to zero so circles are exactly
/// symmetrical (Blender #87779).
pub(crate) fn sin_cos_from_fraction(numerator: i32, denominator: i32) -> (f32, f32) {
    let mut numerator = numerator * 8;
    let octant = numerator / denominator;
    let denominator = denominator * 8;
    let mut cos_sign = 1.0f32;
    let mut swap = false;
    match octant {
        0 => {}
        1 | 2 => {
            numerator = (denominator / 4) - numerator;
            swap = true;
        }
        3 | 4 => {
            numerator = (denominator / 2) - numerator;
            cos_sign = -1.0;
        }
        5 | 6 => {
            numerator -= denominator * 3 / 4;
            swap = true;
            cos_sign = -1.0;
        }
        _ => numerator -= denominator,
    }
    let angle = (2.0 * std::f64::consts::PI) as f32 * (numerator as f32 / denominator as f32);
    let s = libm::sinf(angle);
    let c = libm::cosf(angle) * cos_sign;
    // `SWAP(float *, r_sin, r_cos)` swaps the output pointers; the sign still applies to the
    // value written through `r_cos` (which is then the sine slot).
    if swap { (c, s) } else { (s, c) }
}

/// `normalize_v3_length` (C style; see `frap_geom::normals`).
#[inline]
pub(crate) fn normalize_v3_length(a: V3, unit_length: f32) -> V3 {
    frap_geom::normals::normalize_v3_length(a, unit_length).0
}

/// `len_v3`.
#[inline]
pub(crate) fn len_v3(a: V3) -> f32 {
    frap_geom::normals::dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_cos_fraction_is_symmetric() {
        for d in [3, 5, 8, 32, 64] {
            for n in 0..=d {
                let (s, c) = sin_cos_from_fraction(n, d);
                let a = 2.0 * std::f64::consts::PI * n as f64 / d as f64;
                assert!((s as f64 - libm::sin(a)).abs() < 1e-6, "sin {n}/{d}");
                assert!((c as f64 - libm::cos(a)).abs() < 1e-6, "cos {n}/{d}");
            }
        }
        assert_eq!(sin_cos_from_fraction(1, 2), (0.0, -1.0));
    }

    #[test]
    fn invert_scale() {
        let mut m = unit_m4();
        m[0][0] = 2.0;
        m[1][1] = 4.0;
        m[3][2] = 1.0;
        let i = invert_m4(&m);
        assert_eq!(i[0][0], 0.5);
        assert_eq!(i[1][1], 0.25);
        assert_eq!(i[3][2], -1.0);
    }
}
