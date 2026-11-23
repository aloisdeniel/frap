// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BLI_math` functions the data transfer and the vertex-weight modifiers use, in `f32` and
//! in Blender's operation order (ARCHITECTURE §21.3).
//!
//! Ported from blender/source/blender/blenlib/intern/math_geom.cc, math_vector.cc,
//! math_vector_inline.cc, math_matrix_c.cc, math_rotation_c.cc and math_color_blend_inline.cc,
//! and blender/source/blender/blenkernel/intern/mesh_evaluate.cc (`face_center_calc`),
//! Blender 5.2.4, GPL-2.0-or-later.

/// A 3D vector.
pub(crate) type V3 = [f32; 3];
/// A 3×3 matrix in Blender's `float[3][3]` layout (`m[col][row]`).
pub(crate) type M3 = [[f32; 3]; 3];

/// `sub_v3_v3v3`
#[inline]
pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `add_v3_v3v3`
#[inline]
pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `mul_v3_v3fl`
#[inline]
pub(crate) fn scale(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}

/// `madd_v3_v3v3fl`: `a + b·f`.
#[inline]
pub(crate) fn madd(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

/// `negate_v3`
#[inline]
pub(crate) fn negate(a: V3) -> V3 {
    [-a[0], -a[1], -a[2]]
}

/// `dot_v3v3`
#[inline]
pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `cross_v3_v3v3`
#[inline]
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `len_squared_v3`
#[inline]
pub(crate) fn len_sq(a: V3) -> f32 {
    dot(a, a)
}

/// `len_v3`
#[inline]
pub(crate) fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// `len_squared_v3v3`
#[inline]
pub(crate) fn len_sq_v3v3(a: V3, b: V3) -> f32 {
    len_sq(sub(a, b))
}

/// `len_v3v3`
#[inline]
pub(crate) fn len_v3v3(a: V3, b: V3) -> f32 {
    len(sub(a, b))
}

/// `normalize_v3`: the unit vector and the former length (zero below `1e-35`).
pub(crate) fn normalize(a: V3) -> (V3, f32) {
    let d = dot(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        (scale(a, 1.0 / d), d)
    } else {
        ([0.0; 3], 0.0)
    }
}

/// `normalize_v3` (the vector only).
#[inline]
pub(crate) fn normalized(a: V3) -> V3 {
    normalize(a).0
}

/// `is_zero_v3`
#[inline]
pub(crate) fn is_zero(a: V3) -> bool {
    a[0] == 0.0 && a[1] == 0.0 && a[2] == 0.0
}

/// `interp_v3_v3v3`: `(1 − t)·a + t·b`.
pub(crate) fn interp(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `dot_v3v3v3(p, a, b)`: `(a − p)·(b − p)`.
pub(crate) fn dot_v3v3v3(p: V3, a: V3, b: V3) -> f32 {
    dot(sub(a, p), sub(b, p))
}

/// `normal_tri_v3`: the unit normal of a triangle.
pub(crate) fn normal_tri(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    normalized(cross(n1, n2))
}

/// `closest_to_ray_v3`: the point of the ray nearest to `p` and its factor.
fn closest_to_ray(p: V3, ray_orig: V3, ray_dir: V3) -> (V3, f32) {
    if is_zero(ray_dir) {
        return (ray_orig, 0.0);
    }
    let h = sub(p, ray_orig);
    let lambda = dot(ray_dir, h) / dot(ray_dir, ray_dir);
    (madd(ray_orig, ray_dir, lambda), lambda)
}

/// `closest_to_line_segment_v3`.
pub(crate) fn closest_to_line_segment(p: V3, l1: V3, l2: V3) -> V3 {
    let (cp, lambda) = closest_to_ray(p, l1, sub(l2, l1));
    if lambda <= 0.0 {
        l1
    } else if lambda >= 1.0 {
        l2
    } else {
        cp
    }
}

/// `dist_squared_to_line_segment_v3`.
pub(crate) fn dist_squared_to_line_segment(p: V3, l1: V3, l2: V3) -> f32 {
    len_sq_v3v3(closest_to_line_segment(p, l1, l2), p)
}

/// `line_point_factor_v3`.
pub(crate) fn line_point_factor(p: V3, l1: V3, l2: V3) -> f32 {
    let u = sub(l2, l1);
    let h = sub(p, l1);
    let d = len_sq(u);
    if d > 0.0 { dot(u, h) / d } else { 0.0 }
}

/// `closest_on_tri_to_point_v3` (Ericson, "Real-Time Collision Detection").
pub(crate) fn closest_on_tri_to_point(p: V3, v1: V3, v2: V3, v3: V3) -> V3 {
    let ab = sub(v2, v1);
    let ac = sub(v3, v1);
    let ap = sub(p, v1);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return v1;
    }
    let bp = sub(p, v2);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return v2;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let ab_squared = d1 - d3;
        if ab_squared == 0.0 {
            return v1;
        }
        return madd(v1, ab, d1 / ab_squared);
    }
    let cp = sub(p, v3);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return v3;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let ac_squared = d2 - d6;
        if ac_squared == 0.0 {
            return v1;
        }
        return madd(v1, ac, d2 / ac_squared);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let bc_squared = (d4 - d3) + (d5 - d6);
        if bc_squared == 0.0 {
            return v2;
        }
        let r = scale(sub(v3, v2), (d4 - d3) / bc_squared);
        return add(r, v2);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let acw = scale(ac, w);
    add(madd(v1, ab, v), acw)
}

/// `project_v3_v3v3`: the projection of `p` on `v_proj`.
fn project(p: V3, v_proj: V3) -> V3 {
    let mul = dot(p, v_proj) / dot(v_proj, v_proj);
    scale(v_proj, mul)
}

/// `isect_line_line_v3` (`isect_line_line_epsilon_v3` with `1e-6`): 0 (parallel), 1 (one
/// intersection, both points equal) or 2 (the two nearest points).
pub(crate) fn isect_line_line(v1: V3, v2: V3, v3: V3, v4: V3) -> (i32, V3, V3) {
    let epsilon = 0.000_001f32;
    let c = sub(v3, v1);
    let a = sub(v2, v1);
    let b = sub(v4, v3);
    let ab = cross(a, b);
    let d = dot(c, ab);
    let div = dot(ab, ab);
    if div == 0.0 {
        return (0, [0.0; 3], [0.0; 3]);
    }
    if d.abs() <= epsilon {
        let cb = cross(c, b);
        let a = scale(a, dot(cb, ab) / div);
        let i1 = add(v1, a);
        return (1, i1, i1);
    }
    let t = sub(v1, v3);
    let n = cross(a, b);
    let t = project(t, n);
    let v3t = add(v3, t);
    let v4t = add(v4, t);
    let c = sub(v3t, v1);
    let a = sub(v2, v1);
    let b = sub(v4t, v3t);
    let ab = cross(a, b);
    let cb = cross(c, b);
    let a = scale(a, dot(cb, ab) / dot(ab, ab));
    let i1 = add(v1, a);
    let i2 = sub(i1, t);
    (2, i1, i2)
}

/// `getLowestRoot`
fn lowest_root(a: f32, b: f32, c: f32, max_r: f32) -> Option<f32> {
    let determinant = b * b - 4.0 * a * c;
    if determinant >= 0.0 {
        let sqrt_d = determinant.sqrt();
        let mut r1 = (-b - sqrt_d) / (2.0 * a);
        let mut r2 = (-b + sqrt_d) / (2.0 * a);
        if r1 > r2 {
            std::mem::swap(&mut r1, &mut r2);
        }
        if r1 > 0.0 && r1 < max_r {
            return Some(r1);
        }
        if r2 > 0.0 && r2 < max_r {
            return Some(r2);
        }
    }
    None
}

/// `isect_sweeping_sphere_tri_v3`: the factor along `p1 → p2` where a sphere of `radius`
/// first touches the triangle, and the touching point.
pub(crate) fn isect_sweeping_sphere_tri(
    p1: V3,
    p2: V3,
    radius: f32,
    v0: V3,
    v1: V3,
    v2: V3,
) -> Option<(f32, V3)> {
    let radius2 = radius * radius;
    let e1 = sub(v1, v0);
    let e2 = sub(v2, v0);
    let vel = sub(p2, p1);
    let mut nor = normalized(cross(e1, e2));
    if dot(nor, vel) > 0.0 {
        nor = negate(nor);
    }
    let a = dot(p1, nor) - dot(v0, nor);
    let nordotv = dot(nor, vel);
    if nordotv.abs() < 0.000_001 {
        if a.abs() >= radius {
            return None;
        }
    } else {
        let mut t0 = (-a + radius) / nordotv;
        let mut t1 = (-a - radius) / nordotv;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        if t0 > 1.0 || t1 < 0.0 {
            return None;
        }
        t0 = t0.clamp(0.0, 1.0);
        let point = [
            p1[0] + vel[0] * t0 - nor[0] * radius,
            p1[1] + vel[1] * t0 - nor[1] * radius,
            p1[2] + vel[2] * t0 - nor[2] * radius,
        ];
        let a = dot(e1, e1);
        let b = dot(e1, e2);
        let c = dot(e2, e2);
        let temp = sub(point, v0);
        let d = dot(temp, e1);
        let e = dot(temp, e2);
        let x = d * c - e * b;
        let y = e * a - d * b;
        let z = x + y - (a * c - b * b);
        if z <= 0.0 && (x >= 0.0 && y >= 0.0) {
            return Some((t0, point));
        }
    }
    let mut lambda = 1.0f32;
    let mut ipoint = [0.0f32; 3];
    let mut found = false;
    let a = dot(vel, vel);
    for v in [v0, v1, v2] {
        let temp = sub(p1, v);
        let b = 2.0 * dot(vel, temp);
        let c = dot(temp, temp) - radius2;
        if let Some(r) = lowest_root(a, b, c, lambda) {
            lambda = r;
            ipoint = v;
            found = true;
        }
    }
    let e3 = sub(v2, v1);
    let edge = |e: V3, base: V3, bv: V3, lambda: &mut f32, ipoint: &mut V3, found: &mut bool| {
        let elen2 = dot(e, e);
        let edotv = dot(e, vel);
        let edotbv = dot(e, bv);
        let a = elen2 * -dot(vel, vel) + edotv * edotv;
        let b = 2.0 * (elen2 * dot(vel, bv) - edotv * edotbv);
        let c = elen2 * (radius2 - dot(bv, bv)) + edotbv * edotbv;
        if let Some(new_lambda) = lowest_root(a, b, c, *lambda) {
            let f = (edotv * new_lambda - edotbv) / elen2;
            if (0.0..=1.0).contains(&f) {
                *lambda = new_lambda;
                *ipoint = add(scale(e, f), base);
                *found = true;
            }
        }
    };
    let bv = sub(v0, p1);
    edge(e1, v0, bv, &mut lambda, &mut ipoint, &mut found);
    edge(e2, v0, bv, &mut lambda, &mut ipoint, &mut found);
    let bv = sub(v1, p1);
    edge(e3, v1, bv, &mut lambda, &mut ipoint, &mut found);
    found.then_some((lambda, ipoint))
}

/// `axis_dominant_v3_single`
fn axis_dominant_single(v: V3) -> usize {
    let (x, y, z) = (v[0].abs(), v[1].abs(), v[2].abs());
    if x > y {
        if x > z { 0 } else { 2 }
    } else if y > z {
        1
    } else {
        2
    }
}

/// `ortho_v3_v3`: a vector orthogonal to `v` (not unit).
fn ortho(v: V3) -> V3 {
    match axis_dominant_single(v) {
        0 => [-v[1] - v[2], v[0], v[0]],
        1 => [v[1], -v[0] - v[2], v[1]],
        _ => [v[2], v[2], -v[0] - v[1]],
    }
}

/// `interp_dot_slerp`
fn interp_dot_slerp(t: f32, cosom: f32) -> [f32; 2] {
    let eps = 1e-4f32;
    if cosom.abs() < 1.0 - eps {
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

/// `interp_v3_v3v3_slerp`: `None` for opposite vectors.
fn interp_slerp(a: V3, b: V3, t: f32) -> Option<V3> {
    let cosom = dot(a, b);
    if cosom < -1.0 + f32::EPSILON {
        return None;
    }
    let w = interp_dot_slerp(t, cosom);
    Some([
        w[0] * a[0] + w[1] * b[0],
        w[0] * a[1] + w[1] * b[1],
        w[0] * a[2] + w[1] * b[2],
    ])
}

/// `interp_v3_v3v3_slerp_safe`
pub(crate) fn interp_slerp_safe(a: V3, b: V3, t: f32) -> V3 {
    if let Some(r) = interp_slerp(a, b, t) {
        return r;
    }
    let ab_ortho = normalized(ortho(a));
    if t < 0.5 {
        interp_slerp(a, ab_ortho, t * 2.0).unwrap_or(a)
    } else {
        interp_slerp(ab_ortho, b, (t - 0.5) * 2.0).unwrap_or(b)
    }
}

/// `mean_value_half_tan_v3` (`d` = direction, `l` = its length).
fn mean_value_half_tan(d_curr: (V3, f32), d_next: (V3, f32)) -> f32 {
    let area = len(cross(d_curr.0, d_next.0));
    if area != 0.0 {
        let dt = dot(d_curr.0, d_next.0);
        let l = d_curr.1 * d_next.1;
        let result = (l - dt) / area;
        if result.is_finite() {
            return result;
        }
    }
    0.0
}

/// `interp_weights_poly_v3`: mean-value weights of `co` in the polygon `v`.
pub(crate) fn interp_weights_poly(v: &[V3], co: V3) -> Vec<f32> {
    let n = v.len();
    let mut w = vec![0.0f32; n];
    let mut max_value = 0.0f32;
    for p in v {
        for k in 0..3 {
            max_value = max_value.max((p[k] - co[k]).abs());
        }
    }
    let eps = 16.0 * f32::EPSILON * max_value;
    let eps_sq = eps * eps;
    let dir = |a: V3| {
        let d = sub(a, co);
        (d, len(d))
    };
    let mut totweight = 0.0f32;
    let mut i_curr = n - 1;
    let mut i_next = 0usize;
    let mut v_curr = v[i_curr];
    let mut v_next = v[i_next];
    let mut d_curr;
    let mut d_next = dir(v_curr);
    let mut ht_prev = mean_value_half_tan(dir(v[n - 2]), d_next);
    // 0: none, 1: point, 2: segment.
    let mut ix_flag = 0;
    while i_next < n {
        if d_next.1 < eps {
            ix_flag = 1;
            break;
        }
        if dist_squared_to_line_segment(co, v_curr, v_next) < eps_sq {
            ix_flag = 2;
            break;
        }
        d_curr = d_next;
        d_next = dir(v_next);
        let ht = mean_value_half_tan(d_curr, d_next);
        w[i_curr] = (ht_prev + ht) / d_curr.1;
        totweight += w[i_curr];
        i_curr = i_next;
        i_next += 1;
        v_curr = v_next;
        if i_next < n {
            v_next = v[i_next];
        }
        ht_prev = ht;
    }
    if ix_flag != 0 {
        w.fill(0.0);
        if ix_flag == 1 {
            w[i_curr] = 1.0;
        } else {
            let fac = line_point_factor(co, v_curr, v_next).clamp(0.0, 1.0);
            w[i_curr] = 1.0 - fac;
            w[i_next] = fac;
        }
    } else if totweight != 0.0 {
        for x in &mut w {
            *x /= totweight;
        }
    }
    w
}

/// `bke::mesh::face_center_calc`.
pub(crate) fn face_center(positions: &[V3], verts: &[i32]) -> V3 {
    let p = |i: usize| positions[verts[i] as usize];
    match verts.len() {
        3 => {
            let (a, b, c) = (p(0), p(1), p(2));
            [
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ]
        }
        4 => {
            let (a, b, c, d) = (p(0), p(1), p(2), p(3));
            [
                (a[0] + b[0] + c[0] + d[0]) / 4.0,
                (a[1] + b[1] + c[1] + d[1]) / 4.0,
                (a[2] + b[2] + c[2] + d[2]) / 4.0,
            ]
        }
        k => {
            let w = 1.0 / k as f32;
            let mut c = [0.0f32; 3];
            for i in 0..k {
                let q = p(i);
                for j in 0..3 {
                    c[j] += q[j] * w;
                }
            }
            c
        }
    }
}

/// `cross_tri_v2`
fn cross_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0])
}

/// `area_tri_v2`
pub(crate) fn area_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (0.5 * cross_tri_v2(v1, v2, v3)).abs()
}

/// `area_poly_v2`
pub(crate) fn area_poly_v2(verts: &[[f32; 2]]) -> f32 {
    let nr = verts.len();
    let mut cross = 0.0f32;
    let mut prev = verts[nr - 1];
    for &curr in verts {
        cross += (prev[0] - curr[0]) * (curr[1] + prev[1]);
        prev = curr;
    }
    (0.5 * cross).abs()
}

/// `ortho_basis_v3v3_v3`
pub(crate) fn ortho_basis(n: V3) -> (V3, V3) {
    let eps = f32::EPSILON;
    let f = n[0] * n[0] + n[1] * n[1];
    if f > eps {
        let d = 1.0 / f.sqrt();
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

/// `axis_dominant_v3_to_m3`: the rotation taking `normal` to +Z (rows: the ortho basis and
/// the normal).
pub(crate) fn axis_dominant_v3_to_m3(normal: V3) -> M3 {
    let (n1, n2) = ortho_basis(normal);
    // Columns n1, n2, normal, transposed: `m[col][row]` with rows n1, n2, normal.
    [
        [n1[0], n2[0], normal[0]],
        [n1[1], n2[1], normal[1]],
        [n1[2], n2[2], normal[2]],
    ]
}

/// `mul_m3_v3` / `mul_v3_m3v3`
pub(crate) fn mul_m3_v3(m: &M3, a: V3) -> V3 {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
        m[0][2] * a[0] + m[1][2] * a[1] + m[2][2] * a[2],
    ]
}

/// `mul_v2_m3v3`
pub(crate) fn mul_v2_m3v3(m: &M3, a: V3) -> [f32; 2] {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
    ]
}

/// `invert_m3_m3` (adjoint over determinant; a singular matrix keeps the adjoint).
pub(crate) fn invert_m3(m: &M3) -> M3 {
    let (m00, m01, m02) = (m[0][0], m[0][1], m[0][2]);
    let (m10, m11, m12) = (m[1][0], m[1][1], m[1][2]);
    let (m20, m21, m22) = (m[2][0], m[2][1], m[2][2]);
    let mut r = [
        [
            m11 * m22 - m12 * m21,
            -m01 * m22 + m02 * m21,
            m01 * m12 - m02 * m11,
        ],
        [
            -m10 * m22 + m12 * m20,
            m00 * m22 - m02 * m20,
            -m00 * m12 + m02 * m10,
        ],
        [
            m10 * m21 - m11 * m20,
            -m00 * m21 + m01 * m20,
            m00 * m11 - m01 * m10,
        ],
    ];
    let det = m00 * (m11 * m22 - m12 * m21) - m10 * (m01 * m22 - m02 * m21)
        + m20 * (m01 * m12 - m02 * m11);
    if det != 0.0 {
        let d = 1.0 / det;
        for col in &mut r {
            for x in col.iter_mut() {
                *x *= d;
            }
        }
    }
    r
}

/// `divide_round_i`
#[inline]
fn divide_round_i(a: i32, b: i32) -> i32 {
    (2 * a + b) / (2 * b)
}

/// `unit_float_to_uchar_clamp`-free rounding of `round_fl_to_uchar_clamp`.
pub(crate) fn round_fl_to_uchar_clamp(a: f32) -> u8 {
    let r = (a + 0.5).floor();
    if r <= 0.0 {
        0
    } else if r >= 255.0 {
        255
    } else {
        r as u8
    }
}

/// `blend_color_mix_byte`
pub(crate) fn blend_mix_byte(src1: [u8; 4], src2: [u8; 4]) -> [u8; 4] {
    if src2[3] == 0 {
        return src1;
    }
    let t = i32::from(src2[3]);
    let mt = 255 - t;
    let s1 = src1.map(i32::from);
    let s2 = src2.map(i32::from);
    let tmp = [
        mt * s1[3] * s1[0] + t * 255 * s2[0],
        mt * s1[3] * s1[1] + t * 255 * s2[1],
        mt * s1[3] * s1[2] + t * 255 * s2[2],
        mt * s1[3] + t * 255,
    ];
    [
        divide_round_i(tmp[0], tmp[3]) as u8,
        divide_round_i(tmp[1], tmp[3]) as u8,
        divide_round_i(tmp[2], tmp[3]) as u8,
        divide_round_i(tmp[3], 255) as u8,
    ]
}

/// `blend_color_add_byte`
pub(crate) fn blend_add_byte(src1: [u8; 4], src2: [u8; 4]) -> [u8; 4] {
    if src2[3] == 0 {
        return src1;
    }
    let t = i32::from(src2[3]);
    let c = |k: usize| {
        let tmp = i32::from(src1[k]) * 255 + i32::from(src2[k]) * t;
        divide_round_i(tmp, 255).min(255) as u8
    };
    [c(0), c(1), c(2), src1[3]]
}

/// `blend_color_sub_byte`
pub(crate) fn blend_sub_byte(src1: [u8; 4], src2: [u8; 4]) -> [u8; 4] {
    if src2[3] == 0 {
        return src1;
    }
    let t = i32::from(src2[3]);
    let c = |k: usize| {
        let tmp = i32::from(src1[k]) * 255 - i32::from(src2[k]) * t;
        divide_round_i(tmp, 255).max(0) as u8
    };
    [c(0), c(1), c(2), src1[3]]
}

/// `blend_color_mul_byte`
pub(crate) fn blend_mul_byte(src1: [u8; 4], src2: [u8; 4]) -> [u8; 4] {
    if src2[3] == 0 {
        return src1;
    }
    let t = i32::from(src2[3]);
    let mt = 255 - t;
    let c = |k: usize| {
        let s1 = i32::from(src1[k]);
        let tmp = mt * s1 * 255 + t * s1 * i32::from(src2[k]);
        divide_round_i(tmp, 255 * 255) as u8
    };
    [c(0), c(1), c(2), src1[3]]
}

/// `blend_color_interpolate_byte`
pub(crate) fn blend_interpolate_byte(src1: [u8; 4], src2: [u8; 4], ft: f32) -> [u8; 4] {
    let t = (255.0 * ft) as i32;
    let mt = 255 - t;
    let s1 = src1.map(i32::from);
    let s2 = src2.map(i32::from);
    let tmp = mt * s1[3] + t * s2[3];
    if tmp > 0 {
        [
            divide_round_i(mt * s1[0] * s1[3] + t * s2[0] * s2[3], tmp) as u8,
            divide_round_i(mt * s1[1] * s1[3] + t * s2[1] * s2[3], tmp) as u8,
            divide_round_i(mt * s1[2] * s1[3] + t * s2[2] * s2[3], tmp) as u8,
            divide_round_i(tmp, 255) as u8,
        ]
    } else {
        [src1[0], src1[1], src1[2], 0]
    }
}

/// `blend_color_mix_float`
pub(crate) fn blend_mix_float(src1: [f32; 4], src2: [f32; 4]) -> [f32; 4] {
    if src2[3] == 0.0 {
        return src1;
    }
    let t = src2[3];
    let mt = 1.0 - t;
    [
        mt * src1[0] + src2[0],
        mt * src1[1] + src2[1],
        mt * src1[2] + src2[2],
        mt * src1[3] + t,
    ]
}

/// `blend_color_add_float`
pub(crate) fn blend_add_float(src1: [f32; 4], src2: [f32; 4]) -> [f32; 4] {
    if src2[3] == 0.0 {
        return src1;
    }
    [
        src1[0] + src2[0] * src1[3],
        src1[1] + src2[1] * src1[3],
        src1[2] + src2[2] * src1[3],
        src1[3],
    ]
}

/// `blend_color_sub_float`
pub(crate) fn blend_sub_float(src1: [f32; 4], src2: [f32; 4]) -> [f32; 4] {
    if src2[3] == 0.0 {
        return src1;
    }
    [
        (src1[0] - src2[0] * src1[3]).max(0.0),
        (src1[1] - src2[1] * src1[3]).max(0.0),
        (src1[2] - src2[2] * src1[3]).max(0.0),
        src1[3],
    ]
}

/// `blend_color_mul_float`
pub(crate) fn blend_mul_float(src1: [f32; 4], src2: [f32; 4]) -> [f32; 4] {
    if src2[3] == 0.0 {
        return src1;
    }
    let t = src2[3];
    let mt = 1.0 - t;
    [
        mt * src1[0] + src1[0] * src2[0] * src1[3],
        mt * src1[1] + src1[1] * src2[1] * src1[3],
        mt * src1[2] + src1[2] * src2[2] * src1[3],
        src1[3],
    ]
}

/// `blend_color_interpolate_float`
pub(crate) fn blend_interpolate_float(src1: [f32; 4], src2: [f32; 4], t: f32) -> [f32; 4] {
    let mt = 1.0 - t;
    [
        mt * src1[0] + t * src2[0],
        mt * src1[1] + t * src2[1],
        mt * src1[2] + t * src2[2],
        mt * src1[3] + t * src2[3],
    ]
}

/// `interpf(target, origin, t)`: `t·target + (1 − t)·origin`.
#[inline]
pub(crate) fn interpf(target: f32, origin: f32, t: f32) -> f32 {
    (t * target) + ((1.0 - t) * origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_on_tri_regions() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(closest_on_tri_to_point([-1.0, -1.0, 1.0], a, b, c), a);
        assert_eq!(closest_on_tri_to_point([2.0, -0.5, 0.0], a, b, c), b);
        assert_eq!(
            closest_on_tri_to_point([0.25, 0.25, 3.0], a, b, c),
            [0.25, 0.25, 0.0]
        );
        assert_eq!(
            closest_on_tri_to_point([0.5, -1.0, 0.0], a, b, c),
            [0.5, 0.0, 0.0]
        );
    }

    #[test]
    fn mean_value_weights() {
        let quad = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let w = interp_weights_poly(&quad, [0.5, 0.5, 0.0]);
        for x in &w {
            assert!((x - 0.25).abs() < 1e-6, "{w:?}");
        }
        // On a corner and on an edge.
        assert_eq!(
            interp_weights_poly(&quad, [1.0, 1.0, 0.0]),
            [0.0, 0.0, 1.0, 0.0]
        );
        let e = interp_weights_poly(&quad, [0.25, 0.0, 0.0]);
        assert!(
            (e[0] - 0.75).abs() < 1e-6 && (e[1] - 0.25).abs() < 1e-6,
            "{e:?}"
        );
    }

    #[test]
    fn sweeping_sphere_hits_the_face_and_an_edge() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let (l, p) =
            isect_sweeping_sphere_tri([0.2, 0.2, 1.0], [0.2, 0.2, -1.0], 0.1, a, b, c).unwrap();
        assert!((l - 0.45).abs() < 1e-6, "{l}");
        assert!((p[2]).abs() < 1e-6);
        // Passing beside edge AB within the radius.
        assert!(
            isect_sweeping_sphere_tri([0.5, -0.05, 1.0], [0.5, -0.05, -1.0], 0.1, a, b, c)
                .is_some()
        );
        assert!(
            isect_sweeping_sphere_tri([0.5, -0.5, 1.0], [0.5, -0.5, -1.0], 0.1, a, b, c).is_none()
        );
    }

    #[test]
    fn byte_blends() {
        assert_eq!(
            blend_mix_byte([10, 20, 30, 255], [200, 100, 0, 0]),
            [10, 20, 30, 255]
        );
        assert_eq!(
            blend_mix_byte([0, 0, 0, 255], [255, 255, 255, 255]),
            [255, 255, 255, 255]
        );
        assert_eq!(
            blend_add_byte([100, 200, 0, 255], [100, 100, 0, 255]),
            [200, 255, 0, 255]
        );
        // `int(255 * 0.5)` = 127.
        assert_eq!(
            blend_interpolate_byte([0, 0, 0, 255], [255, 255, 255, 255], 0.5)[0],
            127
        );
        assert_eq!(round_fl_to_uchar_clamp(254.6), 255);
        assert_eq!(round_fl_to_uchar_clamp(-3.0), 0);
    }

    #[test]
    fn invert_rotation() {
        let m = axis_dominant_v3_to_m3(normalized([0.3, -0.2, 0.9]));
        let i = invert_m3(&m);
        let v = [0.1, 0.7, -0.4];
        let back = mul_m3_v3(&i, mul_m3_v3(&m, v));
        for k in 0..3 {
            assert!((back[k] - v[k]).abs() < 1e-6);
        }
        assert!((mul_m3_v3(&m, normalized([0.3, -0.2, 0.9]))[2] - 1.0).abs() < 1e-6);
    }
}
