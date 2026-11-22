// SPDX-License-Identifier: GPL-3.0-or-later
//! The vector maths of `object.shape_key_transfer` (a Python operator of
//! `scripts/startup/bl_operators/object.py` built on `mathutils`): single-precision vectors,
//! `Vector.length` in double precision (`dot_vn_vn` sums float products in double, last
//! component first), and `mathutils.geometry.barycentric_transform`
//! (`transform_point_by_tri_v3`).
//!
//! Ported from blender/source/blender/blenlib/intern/math_geom.cc, math_vector.cc and
//! blender/source/blender/python/mathutils/mathutils_Vector.cc (Blender 5.2.4,
//! GPL-2.0-or-later).

/// A `mathutils.Vector` of size 3.
pub type V3 = [f32; 3];

/// `a + b`.
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a - b`.
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a * f`.
pub fn scale(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}

/// `dot_vn_vn` (float products summed in double, from the last component).
fn dot_vn(a: V3, b: V3) -> f64 {
    let mut d = 0.0f64;
    for i in (0..3).rev() {
        d += f64::from(a[i] * b[i]);
    }
    d
}

/// `Vector.length` (`sqrt(dot_vn_vn(v, v))` in double).
pub fn length(a: V3) -> f64 {
    dot_vn(a, a).sqrt()
}

/// `Vector.length = param` (`Vector_length_set`): zero for 0, unchanged for a zero vector or
/// the same length, else scaled by `1 / (len / param)` in single precision.
pub fn with_length(a: V3, param: f64) -> V3 {
    if param == 0.0 {
        return [0.0; 3];
    }
    let dot = dot_vn(a, a);
    if dot == 0.0 {
        return a;
    }
    let len = dot.sqrt();
    if len == param {
        return a;
    }
    let f = (1.0 / (len / param)) as f32;
    scale(a, f)
}

/// `Vector / n` (`mul_vn_vn_fl(v, 1.0f / scalar)`).
pub fn div(a: V3, n: f32) -> V3 {
    scale(a, 1.0 / n)
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `normalize_v3` (zero below `1e-35` squared length).
fn normalize(a: V3) -> V3 {
    let d = dot(a, a);
    if d > 1.0e-35 {
        scale(a, 1.0 / libm::sqrtf(d))
    } else {
        [0.0; 3]
    }
}

/// `cross_tri_v3`.
fn cross_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    cross(sub(v1, v2), sub(v2, v3))
}

/// `normal_tri_v3`.
fn normal_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    normalize(cross_tri_v3(v1, v2, v3))
}

/// `area_tri_v3`.
fn area_tri_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    libm::sqrtf(dot(cross_tri_v3(v1, v2, v3), cross_tri_v3(v1, v2, v3))) * 0.5
}

/// `cross_tri_v2`.
fn cross_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0])
}

/// `area_tri_v2`.
fn area_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (0.5 * cross_tri_v2(v1, v2, v3)).abs()
}

/// `barycentric_weights_v2` (release build: a non-finite result falls back to thirds).
fn barycentric_weights_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2], co: [f32; 2]) -> V3 {
    let w = [
        cross_tri_v2(v2, v3, co),
        cross_tri_v2(v3, v1, co),
        cross_tri_v2(v1, v2, co),
    ];
    let wtot = w[0] + w[1] + w[2];
    let r = scale(w, 1.0 / wtot);
    if r.iter().all(|x| x.is_finite()) {
        r
    } else {
        [1.0 / 3.0; 3]
    }
}

/// `ortho_basis_v3v3_v3`.
fn ortho_basis(n: V3) -> (V3, V3) {
    let f = n[0] * n[0] + n[1] * n[1];
    if f > f32::EPSILON {
        let d = 1.0 / libm::sqrtf(f);
        let n1 = [n[1] * d, -n[0] * d, 0.0];
        let n2 = [-n[2] * n1[1], n[2] * n1[0], n[0] * n1[1] - n[1] * n1[0]];
        (n1, n2)
    } else {
        (
            [if n[2] < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        )
    }
}

/// `mul_v3_m3v3(r, axis_dominant_v3_to_m3(normal), a)`: the rows of the transposed basis
/// dotted with `a`.
fn to_plane(rows: &(V3, V3, V3), a: V3) -> V3 {
    [dot(rows.0, a), dot(rows.1, a), dot(rows.2, a)]
}

/// `interp_v3_v3v3v3`.
fn interp_v3_v3v3v3(v1: V3, v2: V3, v3: V3, w: V3) -> V3 {
    [
        v1[0] * w[0] + v2[0] * w[1] + v3[0] * w[2],
        v1[1] * w[0] + v2[1] * w[1] + v3[1] * w[2],
        v1[2] * w[0] + v2[2] * w[1] + v3[2] * w[2],
    ]
}

/// `mathutils.geometry.barycentric_transform(point, src1, src2, src3, dst1, dst2, dst3)`
/// (`transform_point_by_tri_v3(pt_dst, pt_src, dst…, src…)`): `point` keeps its barycentric
/// weights in the source triangle's plane and its height above it, scaled by the square root
/// of the area ratio, along the target triangle's normal.
pub fn barycentric_transform(point: V3, src: [V3; 3], dst: [V3; 3]) -> V3 {
    let no_tar = normal_tri_v3(dst[0], dst[1], dst[2]);
    let no_src = normal_tri_v3(src[0], src[1], src[2]);
    let (n1, n2) = ortho_basis(no_src);
    let rows = (n1, n2, no_src);
    let pt_xy = to_plane(&rows, point);
    let tri_xy = [
        to_plane(&rows, src[0]),
        to_plane(&rows, src[1]),
        to_plane(&rows, src[2]),
    ];
    let xy = |v: V3| [v[0], v[1]];
    let w = barycentric_weights_v2(xy(tri_xy[0]), xy(tri_xy[1]), xy(tri_xy[2]), xy(pt_xy));
    let pt = interp_v3_v3v3v3(dst[0], dst[1], dst[2], w);
    let area_tar = libm::sqrtf(area_tri_v3(dst[0], dst[1], dst[2]));
    let area_src = libm::sqrtf(area_tri_v2(xy(tri_xy[0]), xy(tri_xy[1]), xy(tri_xy[2])));
    let z_ofs = pt_xy[2] - tri_xy[0][2];
    let f = (z_ofs / area_src) * area_tar;
    [
        pt[0] + no_tar[0] * f,
        pt[1] + no_tar[1] * f,
        pt[2] + no_tar[2] * f,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_keeps_barycentric_weights_and_height() {
        let src = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        // The same triangle scaled by 2 and moved up by 1.
        let dst = [[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [0.0, 2.0, 1.0]];
        let p = barycentric_transform([0.25, 0.25, 0.5], src, dst);
        assert!(
            (p[0] - 0.5).abs() < 1e-6 && (p[1] - 0.5).abs() < 1e-6,
            "{p:?}"
        );
        // Height 0.5 scaled by sqrt(area ratio 4) = 2.
        assert!((p[2] - 2.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn vector_length() {
        let v = with_length([3.0, 0.0, 4.0], 10.0);
        assert!((length(v) - 10.0).abs() < 1e-5);
        assert_eq!(with_length([0.0; 3], 2.0), [0.0; 3]);
        assert_eq!(with_length([1.0, 2.0, 3.0], 0.0), [0.0; 3]);
        assert_eq!(div([2.0, 4.0, 6.0], 2.0), [1.0, 2.0, 3.0]);
    }
}
