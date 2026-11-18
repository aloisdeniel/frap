// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BLI_math_*` helpers the parametrizer, the packer and the projections use, with
//! Blender's exact formulas (normalisation by the reciprocal of the length, `asin`-based angles,
//! column-major `float[2][2]` matrices: `m[col][row]`).
//!
//! Ported from blender/source/blender/blenlib/intern/{math_vector.cc, math_vector_inline.cc,
//! math_geom.cc, math_geom_inline.cc, math_matrix_c.cc, math_rotation_c.cc,
//! math_base_inline.cc} (Blender 5.2.4).

/// 2D vector.
pub type V2 = [f32; 2];
/// 3D vector.
pub type V3 = [f32; 3];
/// Blender's column-major `float[2][2]` (`m[col][row]`).
pub type M2 = [[f32; 2]; 2];

/// `sub_v3_v3v3`.
#[inline]
pub fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `sub_v2_v2v2`.
#[inline]
pub fn sub2(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

/// `dot_v3v3`.
#[inline]
pub fn dot3(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `dot_v2v2`.
#[inline]
pub fn dot2(a: V2, b: V2) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

/// `len_v3`.
#[inline]
pub fn len3(a: V3) -> f32 {
    dot3(a, a).sqrt()
}

/// `len_v3v3`.
#[inline]
pub fn len_v3v3(a: V3, b: V3) -> f32 {
    len3(sub3(a, b))
}

/// `len_squared_v3v3`.
#[inline]
pub fn len_squared_v3v3(a: V3, b: V3) -> f32 {
    let d = sub3(a, b);
    dot3(d, d)
}

/// `len_v2v2`.
#[inline]
pub fn len_v2v2(a: V2, b: V2) -> f32 {
    let d = sub2(a, b);
    dot2(d, d).sqrt()
}

/// `len_squared_v2v2`.
#[inline]
pub fn len_squared_v2v2(a: V2, b: V2) -> f32 {
    let d = sub2(a, b);
    dot2(d, d)
}

/// `cross_v3_v3v3`.
#[inline]
pub fn cross3(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `normalize_v3`: multiplies by the reciprocal of the length; zero (and length 0) when the
/// squared length is `<= 1e-35`.
#[inline]
pub fn normalize3(a: V3) -> (V3, f32) {
    let d = dot3(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let f = 1.0 / d;
        ([a[0] * f, a[1] * f, a[2] * f], d)
    } else {
        ([0.0; 3], 0.0)
    }
}

/// `normalize_v2`.
#[inline]
pub fn normalize2(a: V2) -> (V2, f32) {
    let d = dot2(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        let f = 1.0 / d;
        ([a[0] * f, a[1] * f], d)
    } else {
        ([0.0; 2], 0.0)
    }
}

/// `math::normalize_and_get_length` (C++ `BLI_math_vector.hh`): divides by the length.
#[inline]
pub fn normalize2_div(a: V2) -> (V2, f32) {
    let d = dot2(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        ([a[0] / d, a[1] / d], d)
    } else {
        ([0.0; 2], 0.0)
    }
}

/// `safe_asinf`.
#[inline]
pub fn safe_asinf(a: f32) -> f32 {
    libm::asinf(a.clamp(-1.0, 1.0))
}

/// `angle_normalized_v3v3`.
pub fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if dot3(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = [-v2[0], -v2[1], -v2[2]];
    core::f32::consts::PI - 2.0 * safe_asinf(len_v3v3(v1, v2_n) / 2.0)
}

/// `angle_normalized_v2v2`.
pub fn angle_normalized_v2v2(a: V2, b: V2) -> f32 {
    if dot2(a, b) >= 0.0 {
        return 2.0 * safe_asinf(len_v2v2(a, b) / 2.0);
    }
    let b_n = [-b[0], -b[1]];
    core::f32::consts::PI - 2.0 * safe_asinf(len_v2v2(a, b_n) / 2.0)
}

/// `angle_v3v3v3`: the angle at `b`.
pub fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    let (v1, _) = normalize3(sub3(b, a));
    let (v2, _) = normalize3(sub3(b, c));
    angle_normalized_v3v3(v1, v2)
}

/// `angle_v2v2v2`: the angle at `b`.
pub fn angle_v2v2v2(a: V2, b: V2, c: V2) -> f32 {
    let (v1, _) = normalize2(sub2(b, a));
    let (v2, _) = normalize2(sub2(b, c));
    angle_normalized_v2v2(v1, v2)
}

/// `cross_tri_v3`.
#[inline]
pub fn cross_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub3(v1, v2);
    let n2 = sub3(v2, v3);
    [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ]
}

/// `area_tri_v3`.
#[inline]
pub fn area_tri_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    len3(cross_tri_v3(v1, v2, v3)) * 0.5
}

/// `add_newell_cross_v3_v3v3`.
#[inline]
pub fn add_newell_cross(n: &mut V3, v_prev: V3, v_curr: V3) {
    n[0] += (v_prev[1] - v_curr[1]) * (v_prev[2] + v_curr[2]);
    n[1] += (v_prev[2] - v_curr[2]) * (v_prev[0] + v_curr[0]);
    n[2] += (v_prev[0] - v_curr[0]) * (v_prev[1] + v_curr[1]);
}

/// `line_point_side_v2`.
#[inline]
pub fn line_point_side_v2(l1: V2, l2: V2, pt: V2) -> f32 {
    ((l1[0] - pt[0]) * (l2[1] - pt[1])) - ((l2[0] - pt[0]) * (l1[1] - pt[1]))
}

/// `mul_m2_v2` / `mul_v2_m2v2` (column-major).
#[inline]
pub fn mul_m2_v2(m: &M2, v: V2) -> V2 {
    [
        m[0][0] * v[0] + m[1][0] * v[1],
        m[0][1] * v[0] + m[1][1] * v[1],
    ]
}

/// `angle_to_mat2`.
pub fn angle_to_mat2(angle: f32) -> M2 {
    let c = libm::cosf(angle);
    let s = libm::sinf(angle);
    [[c, s], [-s, c]]
}

/// `invert_m2_m2`: `None` when the determinant is zero.
pub fn invert_m2_m2(m: &M2) -> Option<M2> {
    let det = m[0][0] * m[1][1] - m[1][0] * m[0][1];
    if det == 0.0 {
        return None;
    }
    Some([
        [m[1][1] / det, -m[0][1] / det],
        [-m[1][0] / det, m[0][0] / det],
    ])
}

/// `ortho_basis_v3v3_v3`.
pub fn ortho_basis_v3v3_v3(n: V3) -> (V3, V3) {
    let eps = f32::EPSILON;
    let f = n[0] * n[0] + n[1] * n[1];
    if f > eps {
        let d = 1.0 / f.sqrt();
        let r1 = [n[1] * d, -n[0] * d, 0.0];
        let r2 = [-n[2] * r1[1], n[2] * r1[0], n[0] * r1[1] - n[1] * r1[0]];
        (r1, r2)
    } else {
        (
            [if n[2] < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
    }
}

/// Blender's column-major `float[3][3]` (`m[col][row]`).
pub type M3 = [[f32; 3]; 3];

/// `axis_dominant_v3_to_m3`: rows of the result are the projection basis (`x`, `y`, `normal`)
/// after Blender's transpose, so [`mul_v2_m3v3`] projects onto the plane of `normal`.
pub fn axis_dominant_v3_to_m3(normal: V3) -> M3 {
    let z = normal;
    let (x, y) = ortho_basis_v3v3_v3(z);
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
}

/// `axis_dominant_v3_to_m3_negate`.
pub fn axis_dominant_v3_to_m3_negate(normal: V3) -> M3 {
    let z = [-normal[0], -normal[1], -normal[2]];
    let (x, y) = ortho_basis_v3v3_v3(z);
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
}

/// `mul_v2_m3v3` (column-major).
#[inline]
pub fn mul_v2_m3v3(m: &M3, a: V3) -> V2 {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
    ]
}

/// `mul_m3_v3` (column-major).
#[inline]
pub fn mul_m3_v3(m: &M3, a: V3) -> V3 {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
        m[0][2] * a[0] + m[1][2] * a[1] + m[2][2] * a[2],
    ]
}

/// `ulp_diff_ff`.
pub fn ulp_diff_ff(a: f32, b: f32) -> u32 {
    let sign_bit = 0x8000_0000u32;
    let infinity = 0x7f80_0000u32;
    let (ua, ub) = (a.to_bits(), b.to_bits());
    let (a_sign, b_sign) = (ua & sign_bit, ub & sign_bit);
    let (a_abs, b_abs) = (ua & !sign_bit, ub & !sign_bit);
    if a_abs > infinity || b_abs > infinity {
        return 0xffff_ffff;
    }
    if a_sign == b_sign {
        a_abs.max(b_abs) - a_abs.min(b_abs)
    } else {
        a_abs.wrapping_add(b_abs)
    }
}

/// `compare_ff_relative`.
pub fn compare_ff_relative(a: f32, b: f32, max_diff: f32, max_ulps: u32) -> bool {
    if (a - b).abs() <= max_diff {
        return true;
    }
    ulp_diff_ff(a, b) <= max_ulps
}

fn isect_tri_tri_v2_impl_vert(a0: V2, a1: V2, a2: V2, b0: V2, b1: V2, b2: V2) -> bool {
    let s = line_point_side_v2;
    if s(b2, b0, a1) >= 0.0 {
        if s(b2, b1, a1) <= 0.0 {
            if s(a0, b0, a1) > 0.0 {
                return s(a0, b1, a1) <= 0.0;
            }
            if s(a0, b0, a2) >= 0.0 {
                return s(a1, a2, b0) >= 0.0;
            }
            return false;
        }
        if s(a0, b1, a1) <= 0.0 {
            if s(b2, b1, a2) <= 0.0 {
                return s(a1, a2, b1) >= 0.0;
            }
            return false;
        }
        return false;
    }
    if s(b2, b0, a2) >= 0.0 {
        if s(a1, a2, b2) >= 0.0 {
            return s(a0, b0, a2) >= 0.0;
        }
        if s(a1, a2, b1) >= 0.0 {
            return s(b2, a2, b1) >= 0.0;
        }
        return false;
    }
    false
}

fn isect_tri_tri_v2_impl_edge(a0: V2, a1: V2, a2: V2, b0: V2, _b1: V2, b2: V2) -> bool {
    let s = line_point_side_v2;
    if s(b2, b0, a1) >= 0.0 {
        if s(a0, b0, a1) >= 0.0 {
            return s(a0, a1, b2) >= 0.0;
        }
        if s(a1, a2, b0) >= 0.0 {
            return s(a2, a0, b0) >= 0.0;
        }
        return false;
    }
    if s(b2, b0, a2) >= 0.0 {
        if s(a0, b0, a2) >= 0.0 {
            if s(a0, a2, b2) >= 0.0 {
                return true;
            }
            return s(a1, a2, b2) >= 0.0;
        }
        return false;
    }
    false
}

fn isect_tri_tri_impl_ccw_v2(a0: V2, a1: V2, a2: V2, b0: V2, b1: V2, b2: V2) -> bool {
    let s = line_point_side_v2;
    if s(b0, b1, a0) >= 0.0 {
        if s(b1, b2, a0) >= 0.0 {
            if s(b2, b0, a0) >= 0.0 {
                return true;
            }
            return isect_tri_tri_v2_impl_edge(a0, a1, a2, b0, b1, b2);
        }
        if s(b2, b0, a0) >= 0.0 {
            return isect_tri_tri_v2_impl_edge(a0, a1, a2, b2, b0, b1);
        }
        return isect_tri_tri_v2_impl_vert(a0, a1, a2, b0, b1, b2);
    }
    if s(b1, b2, a0) >= 0.0 {
        if s(b2, b0, a0) >= 0.0 {
            return isect_tri_tri_v2_impl_edge(a0, a1, a2, b1, b2, b0);
        }
        return isect_tri_tri_v2_impl_vert(a0, a1, a2, b1, b2, b0);
    }
    isect_tri_tri_v2_impl_vert(a0, a1, a2, b2, b0, b1)
}

/// `isect_tri_tri_v2` (Guigue–Devillers orientation predicates).
pub fn isect_tri_tri_v2(a0: V2, a1: V2, a2: V2, b0: V2, b1: V2, b2: V2) -> bool {
    let s = line_point_side_v2;
    if s(a0, a1, a2) < 0.0 {
        if s(b0, b1, b2) < 0.0 {
            return isect_tri_tri_impl_ccw_v2(a0, a2, a1, b0, b2, b1);
        }
        return isect_tri_tri_impl_ccw_v2(a0, a2, a1, b0, b1, b2);
    }
    if s(b0, b1, b2) < 0.0 {
        return isect_tri_tri_impl_ccw_v2(a0, a1, a2, b0, b2, b1);
    }
    isect_tri_tri_impl_ccw_v2(a0, a1, a2, b0, b1, b2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_match_acos() {
        let a = angle_v3v3v3([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((a - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
        let b = angle_v2v2v2([1.0, 0.0], [0.0, 0.0], [-1.0, 0.1]);
        assert!((b - libm::acosf(-1.0 / libm::sqrtf(1.01))).abs() < 1e-5);
    }

    #[test]
    fn tri_tri_overlap() {
        let t = ([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]);
        assert!(isect_tri_tri_v2(
            t.0,
            t.1,
            t.2,
            [0.2, 0.2],
            [2.0, 0.2],
            [0.2, 2.0]
        ));
        assert!(!isect_tri_tri_v2(
            t.0,
            t.1,
            t.2,
            [2.0, 2.0],
            [3.0, 2.0],
            [2.0, 3.0]
        ));
        // Reversed winding of both.
        assert!(isect_tri_tri_v2(
            t.0,
            t.2,
            t.1,
            [0.2, 0.2],
            [0.2, 2.0],
            [2.0, 0.2]
        ));
    }

    #[test]
    fn invert_and_ulps() {
        let m = [[2.0, 0.0], [0.0, 4.0]];
        let i = invert_m2_m2(&m).unwrap();
        assert_eq!(i, [[0.5, 0.0], [0.0, 0.25]]);
        assert!(invert_m2_m2(&[[1.0, 2.0], [2.0, 4.0]]).is_none());
        assert_eq!(ulp_diff_ff(1.0, f32::from_bits(1.0f32.to_bits() + 3)), 3);
        assert!(compare_ff_relative(1.0, 1.0 + 1e-7, 0.0, 64));
    }
}
