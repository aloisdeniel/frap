// SPDX-License-Identifier: GPL-3.0-or-later
//! The `BLI_math_geom` / `BLI_math_vector` functions `BM_mesh_intersect` calls, in `f32` with
//! Blender's operation order. Blender 5.2.4's build does not contract `a * b + c` into fused
//! multiply-adds here: `mathutils.geometry.intersect_line_line` (this file's
//! `isect_line_line_epsilon_v3`) matches the unfused arithmetic bit for bit
//! (`blender_line_line.json`, checked by the tests below), so the intersection decisions
//! (coplanar lines and faces against zero and small epsilons) are Blender's.
//!
// Ported from blender/source/blender/blenlib/intern/math_geom.cc (isect_line_line_epsilon_v3,
// line_point_factor_v3_ex, isect_line_segment_tri_epsilon_v3, isect_ray_tri_epsilon_v3,
// point_in_slice, isect_point_tri_prism_v3, isect_point_tri_v3, closest_to_ray_v3,
// closest_to_line_v3, plane_from_point_normal_v3, closest_to_plane_v3, cross_tri_v3,
// normal_tri_v3, area_squared_tri_v3), math_vector.cc (interp_v3_v3v3, mid_v3_v3v3v3,
// project_v3_v3v3) and math_vector_inline.cc (dot_v3v3, cross_v3_v3v3, len_squared_v3,
// len_squared_v3v3, madd_v3_v3v3fl, normalize_v3_v3_length), Blender 5.2.4,
// GPL-2.0-or-later.

/// A 3D vector.
pub type V3 = [f32; 3];

/// `sub_v3_v3v3`.
#[inline]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `add_v3_v3v3`.
#[inline]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `mul_v3_v3fl`.
#[inline]
pub fn scale(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}

/// `dot_v3v3`.
#[inline]
pub fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `cross_v3_v3v3`.
#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `len_squared_v3`.
#[inline]
pub fn len_squared_v3(v: V3) -> f32 {
    dot(v, v)
}

/// `len_squared_v3v3(a, b)` (`dot(b - a, b - a)`).
#[inline]
pub fn len_squared_v3v3(a: V3, b: V3) -> f32 {
    let d = sub(b, a);
    dot(d, d)
}

/// `madd_v3_v3v3fl(r, a, b, f)`: `a + b * f`.
#[inline]
fn madd_v3_v3v3fl(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

/// `interp_v3_v3v3(r, a, b, t)`: `s * a + t * b` with `s = 1 - t`.
#[inline]
pub fn interp_v3_v3v3(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `mid_v3_v3v3v3`.
#[inline]
pub fn mid_v3_v3v3v3(v1: V3, v2: V3, v3: V3) -> V3 {
    [
        (v1[0] + v2[0] + v3[0]) / 3.0,
        (v1[1] + v2[1] + v3[1]) / 3.0,
        (v1[2] + v2[2] + v3[2]) / 3.0,
    ]
}

/// `normalize_v3` (in place, returns the input length).
#[inline]
pub fn normalize_v3(a: &mut V3) -> f32 {
    let d = dot(*a, *a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        *a = scale(*a, 1.0 / d);
        d
    } else {
        *a = [0.0; 3];
        0.0
    }
}

/// `cross_tri_v3` (not unit length).
#[inline]
pub fn cross_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    cross(n1, n2)
}

/// `normal_tri_v3`: the unit normal and the length before normalising.
#[inline]
pub fn normal_tri_v3(v1: V3, v2: V3, v3: V3) -> (V3, f32) {
    let mut n = cross_tri_v3(v1, v2, v3);
    let l = normalize_v3(&mut n);
    (n, l)
}

/// `area_squared_tri_v3`.
#[inline]
pub fn area_squared_tri_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    let n = scale(cross_tri_v3(v1, v2, v3), 0.5);
    len_squared_v3(n)
}

/// `is_zero_v3`.
#[inline]
fn is_zero_v3(a: V3) -> bool {
    a[0] == 0.0 && a[1] == 0.0 && a[2] == 0.0
}

/// `project_v3_v3v3(out, p, v_proj)`.
fn project_v3_v3v3(p: V3, v_proj: V3) -> V3 {
    if is_zero_v3(v_proj) {
        return [0.0; 3];
    }
    let mul = dot(p, v_proj) / dot(v_proj, v_proj);
    scale(v_proj, mul)
}

/// `isect_line_line_epsilon_v3(v1, v2, v3, v4, r_i1, r_i2, epsilon)`: 0 (parallel or a zero
/// length line), 1 (coplanar: one point in both) or 2 (the two nearest points).
pub fn isect_line_line_epsilon_v3(v1: V3, v2: V3, v3: V3, v4: V3, epsilon: f32) -> (i32, V3, V3) {
    let c = sub(v3, v1);
    let a = sub(v2, v1);
    let b = sub(v4, v3);

    let ab = cross(a, b);
    let d = dot(c, ab);
    let div = dot(ab, ab);

    // Important not to use an epsilon here (#45919): a zero length line.
    if div == 0.0 {
        return (0, [0.0; 3], [0.0; 3]);
    }
    // Coplanar lines.
    if d.abs() <= epsilon {
        let cb = cross(c, b);
        let a = scale(a, dot(cb, ab) / div);
        let i1 = add(v1, a);
        return (1, i1, i1);
    }

    let t = sub(v1, v3);
    // Offset between both planes the lines lie in.
    let n = cross(a, b);
    let t = project_v3_v3v3(t, n);

    // For the first line, offset the second line until it is coplanar.
    let v3t = add(v3, t);
    let v4t = add(v4, t);

    let c = sub(v3t, v1);
    let a = sub(v2, v1);
    let b = sub(v4t, v3t);

    let ab = cross(a, b);
    let cb = cross(c, b);

    let a = scale(a, dot(cb, ab) / dot(ab, ab));
    let i1 = add(v1, a);
    // For the second line, subtract the offset from the first intersection point.
    let i2 = sub(i1, t);
    (2, i1, i2)
}

/// `line_point_factor_v3_ex(p, l1, l2, epsilon, fallback)`.
#[inline]
pub fn line_point_factor_v3_ex(p: V3, l1: V3, l2: V3, epsilon: f32, fallback: f32) -> f32 {
    let u = sub(l2, l1);
    let h = sub(p, l1);
    let d = len_squared_v3(u);
    if d > epsilon { dot(u, h) / d } else { fallback }
}

/// `line_point_factor_v3(p, l1, l2)`.
#[inline]
pub fn line_point_factor_v3(p: V3, l1: V3, l2: V3) -> f32 {
    line_point_factor_v3_ex(p, l1, l2, 0.0, 0.0)
}

/// `isect_line_segment_tri_epsilon_v3(p1, p2, v0, v1, v2, &lambda, nullptr, epsilon)`: the
/// factor along `p1`-`p2` where the segment meets the triangle.
pub fn isect_line_segment_tri_epsilon_v3(
    p1: V3,
    p2: V3,
    v0: V3,
    v1: V3,
    v2: V3,
    epsilon: f32,
) -> Option<f32> {
    let e1 = sub(v1, v0);
    let e2 = sub(v2, v0);
    let d = sub(p2, p1);

    let p = cross(d, e2);
    let a = dot(e1, p);
    if a == 0.0 {
        return None;
    }
    let f = 1.0 / a;

    let s = sub(p1, v0);

    let u = f * dot(s, p);
    if (u < -epsilon) || (u > 1.0 + epsilon) {
        return None;
    }

    let q = cross(s, e1);

    let v = f * dot(d, q);
    if (v < -epsilon) || ((u + v) > 1.0 + epsilon) {
        return None;
    }

    let lambda = f * dot(e2, q);
    if !(0.0..=1.0).contains(&lambda) {
        return None;
    }
    Some(lambda)
}

/// `isect_ray_tri_epsilon_v3(ray_origin, ray_direction, v0, v1, v2, &lambda, nullptr,
/// epsilon)`.
pub fn isect_ray_tri_epsilon_v3(
    ray_origin: V3,
    ray_direction: V3,
    v0: V3,
    v1: V3,
    v2: V3,
    epsilon: f32,
) -> Option<f32> {
    let e1 = sub(v1, v0);
    let e2 = sub(v2, v0);

    let p = cross(ray_direction, e2);
    let a = dot(e1, p);
    if a == 0.0 {
        return None;
    }
    let f = 1.0 / a;

    let s = sub(ray_origin, v0);

    let u = f * dot(s, p);
    if (u < -epsilon) || (u > 1.0 + epsilon) {
        return None;
    }

    let q = cross(s, e1);

    let v = f * dot(ray_direction, q);
    if (v < -epsilon) || ((u + v) > 1.0 + epsilon) {
        return None;
    }

    let lambda = f * dot(e2, q);
    if lambda < 0.0 {
        return None;
    }
    Some(lambda)
}

/// `closest_to_ray_v3(r_close, p, ray_orig, ray_dir)`.
fn closest_to_ray_v3(p: V3, ray_orig: V3, ray_dir: V3) -> V3 {
    if is_zero_v3(ray_dir) {
        return ray_orig;
    }
    let h = sub(p, ray_orig);
    let lambda = dot(ray_dir, h) / dot(ray_dir, ray_dir);
    madd_v3_v3v3fl(ray_orig, ray_dir, lambda)
}

/// `closest_to_line_v3(r_close, p, l1, l2)`.
fn closest_to_line_v3(p: V3, l1: V3, l2: V3) -> V3 {
    let u = sub(l2, l1);
    closest_to_ray_v3(p, l1, u)
}

/// `point_in_slice(p, v1, l1, l2)`.
fn point_in_slice(p: V3, v1: V3, l1: V3, l2: V3) -> bool {
    let cp = closest_to_line_v3(v1, l1, l2);
    let q = sub(cp, v1);
    let q_squared = dot(q, q);
    // `math::is_zero` (exact comparison).
    if q_squared == 0.0 {
        return false;
    }
    let rp = sub(p, v1);
    let h = dot(q, rp) / q_squared;
    (0.0..=1.0).contains(&h)
}

/// `isect_point_tri_prism_v3`.
fn isect_point_tri_prism_v3(p: V3, v1: V3, v2: V3, v3: V3) -> bool {
    point_in_slice(p, v1, v2, v3) && point_in_slice(p, v2, v3, v1) && point_in_slice(p, v3, v1, v2)
}

/// `isect_point_tri_v3(p, v1, v2, v3, r_isect_co)`: the projection of `p` on the triangle's
/// plane when `p` lies in the triangle's prism.
pub fn isect_point_tri_v3(p: V3, v1: V3, v2: V3, v3: V3) -> Option<V3> {
    if !isect_point_tri_prism_v3(p, v1, v2, v3) {
        return None;
    }
    let no = cross_tri_v3(v1, v2, v3);
    // `plane_from_point_normal_v3` + `closest_to_plane_v3`.
    let plane_d = -dot(no, v1);
    let len_sq = len_squared_v3(no);
    let side = dot(p, no) + plane_d;
    Some(madd_v3_v3v3fl(p, no, -side / len_sq))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_line_coplanar_and_skew() {
        let o = [0.0f32; 3];
        let (n, a, _) =
            isect_line_line_epsilon_v3(o, [2.0, 0.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], 0.0);
        assert_eq!((n, a), (1, [1.0, 0.0, 0.0]));
        let (n, a, b) =
            isect_line_line_epsilon_v3(o, [2.0, 0.0, 0.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], 0.0);
        assert_eq!((n, a, b), (2, [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]));
        let (n, _, _) =
            isect_line_line_epsilon_v3(o, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], 0.0);
        assert_eq!(n, 0);
    }

    #[test]
    fn segment_and_ray_hit_the_triangle() {
        let t = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let l = isect_line_segment_tri_epsilon_v3(
            [0.2, 0.2, -1.0],
            [0.2, 0.2, 1.0],
            t.0,
            t.1,
            t.2,
            0.0,
        );
        assert_eq!(l, Some(0.5));
        assert_eq!(
            isect_line_segment_tri_epsilon_v3([0.2, 0.2, 1.0], [0.2, 0.2, 2.0], t.0, t.1, t.2, 0.0),
            None
        );
        let r = isect_ray_tri_epsilon_v3([0.2, 0.2, -1.0], [0.0, 0.0, 1.0], t.0, t.1, t.2, 0.0);
        assert_eq!(r, Some(1.0));
        let p = isect_point_tri_v3([0.2, 0.2, 0.5], t.0, t.1, t.2).unwrap();
        assert!((p[0] - 0.2).abs() < 1e-7 && (p[1] - 0.2).abs() < 1e-7 && p[2].abs() < 1e-7);
        assert_eq!(isect_point_tri_v3([2.0, 0.2, 0.5], t.0, t.1, t.2), None);
    }

    /// `mathutils.geometry.intersect_line_line` (Blender 5.2.4: `isect_line_line_v3`, then
    /// `closest_to_line_v3` for coplanar lines) on random lines, skew and coplanar:
    /// bit-identical.
    #[test]
    fn line_line_is_bit_identical_to_blender() {
        let d: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("blender_line_line.json")).unwrap();
        let v = |x: &serde_json::Value| -> V3 {
            let a = x.as_array().unwrap();
            [0, 1, 2].map(|i| a[i].as_f64().unwrap() as f32)
        };
        let mut n = 0;
        for c in &d {
            let i = c["in"].as_array().unwrap();
            let (v1, v2, v3, v4) = (v(&i[0]), v(&i[1]), v(&i[2]), v(&i[3]));
            let (k, i1, mut i2) = isect_line_line_epsilon_v3(v1, v2, v3, v4, 0.000001);
            // `M_Geometry_intersect_line_line` reports the second line's closest point.
            if k == 1 {
                i2 = closest_to_line_v3(i1, v3, v4);
            }
            match c["out"].as_array() {
                None => assert_eq!(k, 0),
                Some(o) => {
                    assert_ne!(k, 0);
                    assert_eq!((i1, i2), (v(&o[0]), v(&o[1])), "{c}");
                    n += 1;
                }
            }
        }
        assert!(n > 30);
    }
}
