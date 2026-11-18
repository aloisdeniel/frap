// SPDX-License-Identifier: GPL-3.0-or-later
//! BLI math helpers the create/fill ops need (`blenlib/intern/math_vector.cc`,
//! `math_vector_inline.cc`, `math_geom.cc`, `math_base_inline.cc`; Blender 5.2.4), in
//! Blender's operation order without `mul_add` (ARCHITECTURE §21.3). Shared by the WP 2.8 ops
//! (`contextual_create`, `bridge_loops`, `grid_fill`, `beautify_fill`, `connect_*`,
//! `bisect_plane`, `convex_hull`).

pub use crate::query::geom::{
    V3, add, cross, dot, len_squared_v3, madd_v3_v3fl, normal_quad_v3, normal_tri_v3,
    normalize_v3, scale, sub,
};
pub use frap_geom::normals::{len_squared_v3v3, len_v3v3};

/// `M_PI` as a float (`float(M_PI)`).
pub const PI: f32 = std::f32::consts::PI;

// Ported from blender/source/blender/blenlib/intern/math_base_inline.cc (safe_asinf).
/// `safe_asinf`: `asinf` clamped to `[-1, 1]`.
#[inline]
pub fn safe_asinf(a: f32) -> f32 {
    if a <= -1.0 {
        -PI / 2.0
    } else if a >= 1.0 {
        PI / 2.0
    } else {
        libm::asinf(a)
    }
}

// Ported from blender/source/blender/blenlib/intern/math_base_inline.cc (safe_acosf).
/// `safe_acosf`: `acosf` clamped to `[-1, 1]`.
#[inline]
pub fn safe_acosf(a: f32) -> f32 {
    if a <= -1.0 {
        PI
    } else if a >= 1.0 {
        0.0
    } else {
        libm::acosf(a)
    }
}

/// `len_v3`.
#[inline]
pub fn len_v3(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// `negate_v3`.
#[inline]
pub fn negate(a: V3) -> V3 {
    [-a[0], -a[1], -a[2]]
}

/// `is_zero_v3`.
#[inline]
pub fn is_zero_v3(a: V3) -> bool {
    a[0] == 0.0 && a[1] == 0.0 && a[2] == 0.0
}

/// `is_finite_v3`.
#[inline]
pub fn is_finite_v3(a: V3) -> bool {
    a[0].is_finite() && a[1].is_finite() && a[2].is_finite()
}

/// `interp_v3_v3v3(r, a, b, t)`: `a + (b - a) * t` written as Blender's
/// `s * a + t * b` with `s = 1 - t`.
#[inline]
pub fn interp_v3_v3v3(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `mid_v3_v3v3`.
#[inline]
pub fn mid_v3_v3v3(a: V3, b: V3) -> V3 {
    [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ]
}

/// `madd_v3_v3v3fl(r, a, b, f)`: `a + b * f`.
#[inline]
pub fn madd_v3_v3v3fl(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

/// `add_newell_cross_v3_v3v3(n, v_prev, v_curr)`.
#[inline]
pub fn add_newell_cross_v3_v3v3(n: &mut V3, v_prev: V3, v_curr: V3) {
    n[0] += (v_prev[1] - v_curr[1]) * (v_prev[2] + v_curr[2]);
    n[1] += (v_prev[2] - v_curr[2]) * (v_prev[0] + v_curr[0]);
    n[2] += (v_prev[0] - v_curr[0]) * (v_prev[1] + v_curr[1]);
}

/// `angle_normalized_v3v3`.
pub fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = negate(v2);
    PI - 2.0 * safe_asinf(len_v3v3(v1, v2_n) / 2.0)
}

/// `angle_v3v3`.
pub fn angle_v3v3(a: V3, b: V3) -> f32 {
    angle_normalized_v3v3(normalize_v3(a).0, normalize_v3(b).0)
}

/// `angle_v3v3v3`: the angle at `b` of the corner `a`, `b`, `c`.
pub fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    let v1 = normalize_v3(sub(b, a)).0;
    let v2 = normalize_v3(sub(b, c)).0;
    angle_normalized_v3v3(v1, v2)
}

/// `cos_v3v3v3`.
pub fn cos_v3v3v3(p1: V3, p2: V3, p3: V3) -> f32 {
    let v1 = normalize_v3(sub(p2, p1)).0;
    let v2 = normalize_v3(sub(p2, p3)).0;
    dot(v1, v2)
}

/// `project_plane_normalized_v3_v3v3`.
#[inline]
pub fn project_plane_normalized_v3_v3v3(p: V3, v_plane: V3) -> V3 {
    let mul = dot(p, v_plane);
    madd_v3_v3v3fl(p, v_plane, -mul)
}

/// `project_plane_v3_v3v3` (`v_plane` not normalised).
#[inline]
pub fn project_plane_v3_v3v3(p: V3, v_plane: V3) -> V3 {
    let mul = dot(p, v_plane) / dot(v_plane, v_plane);
    madd_v3_v3v3fl(p, v_plane, -mul)
}

/// `angle_signed_on_axis_v3v3_v3`.
pub fn angle_signed_on_axis_v3v3_v3(v1: V3, v2: V3, axis: V3) -> f32 {
    let v1_proj = project_plane_normalized_v3_v3v3(v1, axis);
    let v2_proj = project_plane_normalized_v3_v3v3(v2, axis);
    let mut angle = angle_v3v3(v1_proj, v2_proj);
    let tproj = cross(v2_proj, v1_proj);
    if dot(tproj, axis) < 0.0 {
        angle = (std::f64::consts::PI * 2.0) as f32 - angle;
    }
    angle
}

/// `angle_signed_on_axis_v3v3v3_v3`.
pub fn angle_signed_on_axis_v3v3v3_v3(v1: V3, v2: V3, v3: V3, axis: V3) -> f32 {
    angle_signed_on_axis_v3v3_v3(sub(v1, v2), sub(v3, v2), axis)
}

/// `cross_tri_v3`.
pub fn cross_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ]
}

/// `area_tri_v3`.
pub fn area_tri_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    len_v3(cross_tri_v3(v1, v2, v3)) / 2.0
}

/// `area_squared_tri_v3`.
pub fn area_squared_tri_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    let n = cross_tri_v3(v1, v2, v3);
    dot(n, n) / 4.0
}

/// `cross_v2v2`.
#[inline]
pub fn cross_v2v2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

/// `area_tri_signed_v2`.
#[inline]
pub fn area_tri_signed_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    0.5 * ((v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0]))
}

/// `area_tri_v2`.
#[inline]
pub fn area_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    area_tri_signed_v2(v1, v2, v3).abs()
}

/// `len_v2v2`.
#[inline]
pub fn len_v2v2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1]];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (is_quad_flip_v3).
/// `is_quad_flip_v3`: the quad `v1 v2 v3 v4` folds over itself.
pub fn is_quad_flip_v3(v1: V3, v2: V3, v3: V3, v4: V3) -> bool {
    let d_12 = sub(v2, v1);
    let d_23 = sub(v3, v2);
    let d_34 = sub(v4, v3);
    let d_41 = sub(v1, v4);
    let cross_a = cross(d_12, d_23);
    let cross_b = cross(d_34, d_41);
    dot(cross_a, cross_b) < 0.0
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (is_quad_convex_v3).
/// `is_quad_convex_v3`.
pub fn is_quad_convex_v3(v1: V3, v2: V3, v3: V3, v4: V3) -> bool {
    let v13 = sub(v1, v3);
    let v24 = sub(v2, v4);
    let plane = cross(v13, v24);
    let eps_sq = 1e-8f32 * 1e-8f32;
    if len_squared_v3(plane) < eps_sq {
        return false;
    }
    let quad = [v1, v2, v3, v4];
    let proj: Vec<V3> = quad
        .iter()
        .map(|&c| project_plane_v3_v3v3(c, plane))
        .collect();
    let mut dirs = [[0.0f32; 3]; 4];
    let mut j = 3;
    for i in 0..4 {
        dirs[i] = sub(proj[i], proj[j]);
        j = i;
    }
    let cs = |a: V3, b: V3| dot(plane, cross(a, b)) > 0.0;
    cs(dirs[0], dirs[1]) && cs(dirs[1], dirs[2]) && cs(dirs[2], dirs[3]) && cs(dirs[3], dirs[0])
}

/// `ISECT_LINE_LINE_*` results of [`isect_seg_seg_v2`].
pub const ISECT_LINE_LINE_COLINEAR: i32 = -1;
/// No intersection.
pub const ISECT_LINE_LINE_NONE: i32 = 0;
/// Touching at an end point.
pub const ISECT_LINE_LINE_EXACT: i32 = 1;
/// Crossing.
pub const ISECT_LINE_LINE_CROSS: i32 = 2;

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (isect_seg_seg_v2).
/// `isect_seg_seg_v2`.
pub fn isect_seg_seg_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2], v4: [f32; 2]) -> i32 {
    let div = (v2[0] - v1[0]) * (v4[1] - v3[1]) - (v2[1] - v1[1]) * (v4[0] - v3[0]);
    if div == 0.0 {
        return ISECT_LINE_LINE_COLINEAR;
    }
    let lambda = ((v1[1] - v3[1]) * (v4[0] - v3[0]) - (v1[0] - v3[0]) * (v4[1] - v3[1])) / div;
    let mu = ((v1[1] - v3[1]) * (v2[0] - v1[0]) - (v1[0] - v3[0]) * (v2[1] - v1[1])) / div;
    if (0.0..=1.0).contains(&lambda) && (0.0..=1.0).contains(&mu) {
        if lambda == 0.0 || lambda == 1.0 || mu == 0.0 || mu == 1.0 {
            return ISECT_LINE_LINE_EXACT;
        }
        return ISECT_LINE_LINE_CROSS;
    }
    ISECT_LINE_LINE_NONE
}

/// `is_quad_convex_v2`: the two diagonals intersect.
pub fn is_quad_convex_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2], v4: [f32; 2]) -> bool {
    isect_seg_seg_v2(v1, v3, v2, v4) > 0
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (axis_dominant_v3_to_m3).
/// `axis_dominant_v3_to_m3`: rows of the projection matrix (`mul_v2_m3v3` uses the first two).
pub fn axis_dominant_v3_to_m3(normal: V3) -> [V3; 3] {
    let (x, y) = frap_geom::polyfill::ortho_basis_v3v3_v3(normal);
    [x, y, normal]
}

/// `mul_v2_m3v3` with the rows of [`axis_dominant_v3_to_m3`].
#[inline]
pub fn mul_v2_m3v3(m: &[V3; 3], a: V3) -> [f32; 2] {
    [
        m[0][0] * a[0] + m[0][1] * a[1] + m[0][2] * a[2],
        m[1][0] * a[0] + m[1][1] * a[1] + m[1][2] * a[2],
    ]
}

/// `mul_v3_m3v3` with the rows of [`axis_dominant_v3_to_m3`].
#[inline]
pub fn mul_v3_m3v3(m: &[V3; 3], a: V3) -> V3 {
    [dot(m[0], a), dot(m[1], a), dot(m[2], a)]
}

/// `divide_floor_i`.
#[inline]
pub fn divide_floor_i(a: i32, b: i32) -> i32 {
    let d = a / b;
    let r = a % b;
    if r != 0 { d - ((a < 0) ^ (b < 0)) as i32 } else { d }
}

/// `mod_i` (always non-negative for a positive `n`).
#[inline]
pub fn mod_i(i: i32, n: i32) -> i32 {
    ((i % n) + n) % n
}

/// `BLI_FOREACH_SPARSE_RANGE(src, dst, i)`: `dst` evenly spread indices in `0..src`.
pub fn sparse_range(src: i32, dst: i32) -> Vec<i32> {
    let mut out = Vec::new();
    if dst <= 0 {
        return out;
    }
    let src2 = src * 2;
    let dst2 = dst * 2;
    let mut error = dst2 - src;
    let mut i = 0i32;
    loop {
        let delta = divide_floor_i(error, dst2);
        i -= delta;
        if i >= src {
            break;
        }
        out.push(i);
        error -= (delta * dst2) + src2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_range_spreads() {
        assert_eq!(sparse_range(4, 2).len(), 2);
        assert_eq!(sparse_range(5, 5), vec![0, 1, 2, 3, 4]);
        assert!(sparse_range(10, 3).iter().all(|&i| (0..10).contains(&i)));
    }

    #[test]
    fn angles() {
        let a = angle_v3v3v3([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((a - PI / 2.0).abs() < 1e-6);
        let s = angle_signed_on_axis_v3v3_v3([1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]);
        assert!((s - PI / 2.0).abs() < 1e-5, "{s}");
    }
}
