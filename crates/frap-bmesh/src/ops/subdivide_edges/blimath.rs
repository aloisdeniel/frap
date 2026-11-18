// SPDX-License-Identifier: GPL-3.0-or-later
//! The BLI math helpers the subdivide family needs (`subdivide_edges`, `subdivide_edgering`),
//! written in Blender's operation order without `mul_add` (ARCHITECTURE §21.3).
//!
//! Ported from blender/source/blender/blenlib/intern/math_vector.cc, math_geom.cc,
//! math_geom_inline.cc, math_base_inline.cc, math_rotation_c.cc, math_matrix_c.cc and
//! blender/source/blender/blenkernel/intern/curve.cc (`BKE_curve_forward_diff_bezier`),
//! Blender 5.2.4, GPL-2.0-or-later. Transcendentals go through `libm` (rule 2.2.12).

pub(crate) use frap_geom::normals::{
    V3, add, cross, dot, len_v3v3, normalize_v3, normalize_v3_length, scale, sub,
};

/// `SMALL_NUMBER` of `math_geom_inline.cc`.
const SMALL_NUMBER: f32 = 1.0e-8;

/// `len_squared_v3`.
#[inline]
pub(crate) fn len_squared_v3(a: V3) -> f32 {
    dot(a, a)
}

/// `negate_v3`.
#[inline]
pub(crate) fn negate(a: V3) -> V3 {
    [-a[0], -a[1], -a[2]]
}

/// `madd_v3_v3v3fl(r, a, b, f)`: `a + b * f`.
#[inline]
pub(crate) fn madd(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

/// `interp_v3_v3v3(r, a, b, t)`.
#[inline]
pub(crate) fn interp_v3_v3v3(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `mid_v3_v3v3`.
#[inline]
pub(crate) fn mid_v3_v3v3(a: V3, b: V3) -> V3 {
    [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ]
}

/// `interpf(target, origin, fac)`.
#[inline]
pub(crate) fn interpf(target: f32, origin: f32, fac: f32) -> f32 {
    (fac * target) + (1.0 - fac) * origin
}

/// `reflect_v3_v3v3`.
#[inline]
pub(crate) fn reflect_v3_v3v3(v: V3, normal: V3) -> V3 {
    let dot2 = 2.0 * dot(v, normal);
    madd(v, normal, -dot2)
}

/// `project_v3_plane(out, plane_no, plane_co)`.
pub(crate) fn project_v3_plane(out: V3, plane_no: V3, plane_co: V3) -> V3 {
    let vector = sub(out, plane_co);
    let mul = dot(vector, plane_no) / len_squared_v3(plane_no);
    let mut r = out;
    r[0] += plane_no[0] * -mul;
    r[1] += plane_no[1] * -mul;
    r[2] += plane_no[2] * -mul;
    r
}

/// `plane_from_point_normal_v3`.
#[inline]
pub(crate) fn plane_from_point_normal_v3(plane_co: V3, plane_no: V3) -> [f32; 4] {
    [plane_no[0], plane_no[1], plane_no[2], -dot(plane_no, plane_co)]
}

/// `determinant_m2`.
#[inline]
fn determinant_m2(a: f32, b: f32, c: f32, d: f32) -> f32 {
    a * d - b * c
}

/// `determinant_m3(a1, a2, a3, b1, b2, b3, c1, c2, c3)`.
#[allow(clippy::too_many_arguments)]
fn determinant_m3(
    a1: f32,
    a2: f32,
    a3: f32,
    b1: f32,
    b2: f32,
    b3: f32,
    c1: f32,
    c2: f32,
    c3: f32,
) -> f32 {
    a1 * determinant_m2(b2, b3, c2, c3) - b1 * determinant_m2(a2, a3, c2, c3)
        + c1 * determinant_m2(a2, a3, b2, b3)
}

fn xyz(p: [f32; 4]) -> V3 {
    [p[0], p[1], p[2]]
}

/// `isect_plane_plane_plane_v3`.
pub(crate) fn isect_plane_plane_plane_v3(a: [f32; 4], b: [f32; 4], c: [f32; 4]) -> Option<V3> {
    let det = determinant_m3(a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]);
    if det == 0.0 {
        return None;
    }
    let tmp = cross(xyz(c), xyz(b));
    let mut r = scale(tmp, a[3]);
    let tmp = cross(xyz(a), xyz(c));
    r = madd(r, tmp, b[3]);
    let tmp = cross(xyz(b), xyz(a));
    r = madd(r, tmp, c[3]);
    Some(scale(r, 1.0 / det))
}

/// `interp_dot_slerp`.
fn interp_dot_slerp(t: f32, cosom: f32) -> [f32; 2] {
    let eps = 1e-4f32;
    if cosom.abs() < (1.0 - eps) {
        let omega = libm::acosf(cosom);
        let sinom = libm::sinf(omega);
        [
            libm::sinf((1.0 - t) * omega) / sinom,
            libm::sinf(t * omega) / sinom,
        ]
    } else {
        [1.0 - t, t]
    }
}

/// `interp_v3_v3v3_slerp`: `None` for direct opposites.
pub(crate) fn interp_v3_v3v3_slerp(a: V3, b: V3, t: f32) -> Option<V3> {
    let cosom = dot(a, b);
    if cosom < (-1.0 + f32::EPSILON) {
        return None;
    }
    let w = interp_dot_slerp(t, cosom);
    Some([
        w[0] * a[0] + w[1] * b[0],
        w[0] * a[1] + w[1] * b[1],
        w[0] * a[2] + w[1] * b[2],
    ])
}

/// `shell_v3v3_mid_normalized_to_dist`.
pub(crate) fn shell_v3v3_mid_normalized_to_dist(a: V3, b: V3) -> f32 {
    let (ab, len) = normalize_v3(add(a, b));
    let angle_cos = if len != 0.0 { dot(a, ab).abs() } else { 0.0 };
    if angle_cos < SMALL_NUMBER {
        1.0
    } else {
        1.0 / angle_cos
    }
}

/// `safe_asinf`.
fn safe_asinf(a: f32) -> f32 {
    libm::asinf(a.clamp(-1.0, 1.0))
}

/// `safe_acosf`.
fn safe_acosf(a: f32) -> f32 {
    libm::acosf(a.clamp(-1.0, 1.0))
}

/// `len_v3`.
#[inline]
pub(crate) fn len_v3(a: V3) -> f32 {
    libm::sqrtf(dot(a, a))
}

/// `angle_normalized_v3v3`.
pub(crate) fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = negate(v2);
    std::f32::consts::PI - 2.0 * safe_asinf(len_v3v3(v1, v2_n) / 2.0)
}

/// `bisect_v3_v3v3v3`.
pub(crate) fn bisect_v3_v3v3v3(a: V3, b: V3, c: V3) -> V3 {
    let d_12 = normalize_v3(sub(b, a)).0;
    let d_23 = normalize_v3(sub(c, b)).0;
    normalize_v3(add(d_12, d_23)).0
}

/// `closest_to_line_v3(r, p, l1, l2)` (through `closest_to_ray_v3`).
pub(crate) fn closest_to_line_v3(p: V3, l1: V3, l2: V3) -> V3 {
    let u = sub(l2, l1);
    if u == [0.0; 3] {
        return l1;
    }
    let h = sub(p, l1);
    let lambda = dot(u, h) / dot(u, u);
    madd(l1, u, lambda)
}

/// A quaternion `[w, x, y, z]`.
pub(crate) type Quat = [f32; 4];

/// `normalize_qt` (a zero quaternion becomes `(0, 1, 0, 0)`, as in Blender).
pub(crate) fn normalize_qt(q: Quat) -> Quat {
    let len = libm::sqrtf(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]);
    if len != 0.0 {
        let f = 1.0 / len;
        [q[0] * f, q[1] * f, q[2] * f, q[3] * f]
    } else {
        [0.0, 1.0, 0.0, 0.0]
    }
}

/// `mul_qt_qtqt(q, a, b)`.
pub(crate) fn mul_qt_qtqt(a: Quat, b: Quat) -> Quat {
    let t0 = a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3];
    let t1 = a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2];
    let t2 = a[0] * b[2] + a[2] * b[0] + a[3] * b[1] - a[1] * b[3];
    let t3 = a[0] * b[3] + a[3] * b[0] + a[1] * b[2] - a[2] * b[1];
    [t0, t1, t2, t3]
}

/// `mul_qt_v3(q, r)`: rotates `r` by `q`.
pub(crate) fn mul_qt_v3(q: Quat, r: V3) -> V3 {
    let t0 = -q[1] * r[0] - q[2] * r[1] - q[3] * r[2];
    let t1 = q[0] * r[0] + q[2] * r[2] - q[3] * r[1];
    let t2 = q[0] * r[1] + q[3] * r[0] - q[1] * r[2];
    let r2 = q[0] * r[2] + q[1] * r[1] - q[2] * r[0];
    let (r0, r1) = (t1, t2);
    let t1 = t0 * -q[1] + r0 * q[0] - r1 * q[3] + r2 * q[2];
    let t2 = t0 * -q[2] + r1 * q[0] - r2 * q[1] + r0 * q[3];
    let r2 = t0 * -q[3] + r2 * q[0] - r0 * q[2] + r1 * q[1];
    [t1, t2, r2]
}

/// `axis_angle_normalized_to_quat`.
pub(crate) fn axis_angle_normalized_to_quat(axis: V3, angle: f32) -> Quat {
    let phi = 0.5 * angle;
    let si = libm::sinf(phi);
    let co = libm::cosf(phi);
    [co, axis[0] * si, axis[1] * si, axis[2] * si]
}

/// `axis_angle_to_quat`: the identity for a zero axis.
pub(crate) fn axis_angle_to_quat(axis: V3, angle: f32) -> Quat {
    let (nor, len) = normalize_v3(axis);
    if len != 0.0 {
        axis_angle_normalized_to_quat(nor, angle)
    } else {
        [1.0, 0.0, 0.0, 0.0]
    }
}

/// `quat_to_mat3_no_error` (`m[col][row]`, as Blender stores it).
pub(crate) fn quat_to_mat3(q: Quat) -> [V3; 3] {
    const M_SQRT2: f64 = std::f64::consts::SQRT_2;
    let q0 = M_SQRT2 * f64::from(q[0]);
    let q1 = M_SQRT2 * f64::from(q[1]);
    let q2 = M_SQRT2 * f64::from(q[2]);
    let q3 = M_SQRT2 * f64::from(q[3]);
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

/// `vec_to_quat(q, vec, axis, upflag)` (`axis` 0..5 = X, Y, Z, -X, -Y, -Z; `upflag` 0..2).
pub(crate) fn vec_to_quat(vec: V3, axis: i16, upflag: i16) -> Quat {
    let eps = 1e-4f32;
    let len = len_v3(vec);
    if len == 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    let (tvec, axis) = if axis > 2 {
        (vec, axis - 3)
    } else {
        (negate(vec), axis)
    };
    let mut nor;
    let co;
    if axis == 0 {
        nor = [0.0, -tvec[2], tvec[1]];
        if tvec[1].abs() + tvec[2].abs() < eps {
            nor[1] = 1.0;
        }
        co = tvec[0];
    } else if axis == 1 {
        nor = [tvec[2], 0.0, -tvec[0]];
        if tvec[0].abs() + tvec[2].abs() < eps {
            nor[2] = 1.0;
        }
        co = tvec[1];
    } else {
        nor = [-tvec[1], tvec[0], 0.0];
        if tvec[0].abs() + tvec[1].abs() < eps {
            nor[0] = 1.0;
        }
        co = tvec[2];
    }
    let co = co / len;
    let nor = normalize_v3(nor).0;
    let mut q = axis_angle_normalized_to_quat(nor, safe_acosf(co));
    if axis != upflag {
        let mat = quat_to_mat3(q);
        let fp = mat[2];
        let angle = if axis == 0 {
            if upflag == 1 {
                0.5 * libm::atan2f(fp[2], fp[1])
            } else {
                -0.5 * libm::atan2f(fp[1], fp[2])
            }
        } else if axis == 1 {
            if upflag == 0 {
                -0.5 * libm::atan2f(fp[2], fp[0])
            } else {
                0.5 * libm::atan2f(fp[0], fp[2])
            }
        } else if upflag == 0 {
            0.5 * libm::atan2f(-fp[1], -fp[0])
        } else {
            -0.5 * libm::atan2f(-fp[0], -fp[1])
        };
        let co = libm::cosf(angle);
        let si = libm::sinf(angle) / len;
        let q2 = [co, tvec[0] * si, tvec[1] * si, tvec[2] * si];
        q = mul_qt_qtqt(q2, q);
    }
    q
}

/// `BKE_curve_forward_diff_bezier(q0, q1, q2, q3, p, it, stride)` for one coordinate:
/// `it + 1` samples of the cubic Bezier.
pub(crate) fn forward_diff_bezier(q0: f32, q1: f32, q2: f32, q3: f32, it: usize) -> Vec<f32> {
    let mut f = it as f32;
    let rt0 = q0;
    let rt1 = 3.0 * (q1 - q0) / f;
    f *= f;
    let rt2 = 3.0 * (q0 - 2.0 * q1 + q2) / f;
    f *= it as f32;
    let rt3 = (q3 - q0 + 3.0 * (q1 - q2)) / f;
    let mut q0 = rt0;
    let mut q1 = rt1 + rt2 + rt3;
    let mut q2 = 2.0 * rt2 + 6.0 * rt3;
    let q3 = 6.0 * rt3;
    let mut out = Vec::with_capacity(it + 1);
    for _ in 0..=it {
        out.push(q0);
        q0 += q1;
        q1 += q2;
        q2 += q3;
    }
    out
}

/// `normal_tri_v3`.
pub(crate) fn normal_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    normalize_v3(cross_tri_v3(v1, v2, v3)).0
}

/// `cross_tri_v3`.
fn cross_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    cross(n1, n2)
}

/// `area_tri_v3`.
fn area_tri_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    len_v3(cross_tri_v3(v1, v2, v3)) * 0.5
}

/// `cross_tri_v2`.
fn cross_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0])
}

/// `area_tri_v2`.
fn area_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (0.5 * ((v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0]))).abs()
}

/// `barycentric_weights_v2`.
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

/// `axis_dominant_v3_to_m3`, as rows: `rows[i]` dotted with a point gives coordinate `i`.
fn axis_dominant_v3_to_m3_rows(normal: V3) -> [V3; 3] {
    let (n1, n2) = frap_geom::polyfill::ortho_basis_v3v3_v3(normal);
    [n1, n2, normal]
}

/// `mul_v3_m3v3` with the transposed matrix of [`axis_dominant_v3_to_m3_rows`].
fn mul_rows(rows: &[V3; 3], a: V3) -> V3 {
    [
        rows[0][0] * a[0] + rows[0][1] * a[1] + rows[0][2] * a[2],
        rows[1][0] * a[0] + rows[1][1] * a[1] + rows[1][2] * a[2],
        rows[2][0] * a[0] + rows[2][1] * a[1] + rows[2][2] * a[2],
    ]
}

/// `interp_v3_v3v3v3`.
fn interp_v3_v3v3v3(v1: V3, v2: V3, v3: V3, w: V3) -> V3 {
    [
        v1[0] * w[0] + v2[0] * w[1] + v3[0] * w[2],
        v1[1] * w[0] + v2[1] * w[1] + v3[1] * w[2],
        v1[2] * w[0] + v2[2] * w[1] + v3[2] * w[2],
    ]
}

/// `transform_point_by_tri_v3(pt_tar, pt_src, tar…, src…)`: maps `pt_src` from triangle `src`
/// to triangle `tar` (barycentric weights in the source plane plus the scaled normal offset).
pub(crate) fn transform_point_by_tri_v3(pt_src: V3, tar: [V3; 3], src: [V3; 3]) -> V3 {
    let no_tar = normal_tri_v3(tar[0], tar[1], tar[2]);
    let no_src = normal_tri_v3(src[0], src[1], src[2]);
    let mat_src = axis_dominant_v3_to_m3_rows(no_src);
    let pt_src_xy = mul_rows(&mat_src, pt_src);
    let tri_xy_src = [
        mul_rows(&mat_src, src[0]),
        mul_rows(&mat_src, src[1]),
        mul_rows(&mat_src, src[2]),
    ];
    let xy = |v: V3| [v[0], v[1]];
    let w_src = barycentric_weights_v2(
        xy(tri_xy_src[0]),
        xy(tri_xy_src[1]),
        xy(tri_xy_src[2]),
        xy(pt_src_xy),
    );
    let pt_tar = interp_v3_v3v3v3(tar[0], tar[1], tar[2], w_src);
    let area_tar = libm::sqrtf(area_tri_v3(tar[0], tar[1], tar[2]));
    let area_src = libm::sqrtf(area_tri_v2(
        xy(tri_xy_src[0]),
        xy(tri_xy_src[1]),
        xy(tri_xy_src[2]),
    ));
    let z_ofs_src = pt_src_xy[2] - tri_xy_src[0][2];
    madd(pt_tar, no_tar, (z_ofs_src / area_src) * area_tar)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slerp_halfway_is_the_bisector() {
        let r = interp_v3_v3v3_slerp([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.5).unwrap();
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!((r[0] - s).abs() < 1e-6 && (r[1] - s).abs() < 1e-6);
        assert!(interp_v3_v3v3_slerp([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], 0.5).is_none());
    }

    #[test]
    fn three_planes_meet_at_a_point() {
        let a = plane_from_point_normal_v3([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let b = plane_from_point_normal_v3([0.0, 2.0, 0.0], [0.0, 1.0, 0.0]);
        let c = plane_from_point_normal_v3([0.0, 0.0, 3.0], [0.0, 0.0, 1.0]);
        let p = isect_plane_plane_plane_v3(a, b, c).unwrap();
        assert_eq!(p, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn bezier_samples_hit_the_end_points() {
        let s = forward_diff_bezier(0.0, 1.0, 2.0, 3.0, 3);
        assert_eq!(s.len(), 4);
        for (i, x) in s.iter().enumerate() {
            assert!((x - i as f32).abs() < 1e-5, "{s:?}");
        }
    }

    #[test]
    fn quaternion_rotation() {
        let q = axis_angle_to_quat([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2);
        let r = mul_qt_v3(q, [1.0, 0.0, 0.0]);
        assert!(r[0].abs() < 1e-6 && (r[1] - 1.0).abs() < 1e-6 && r[2].abs() < 1e-6);
    }
}
