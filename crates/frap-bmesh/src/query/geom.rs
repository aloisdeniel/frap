// SPDX-License-Identifier: GPL-3.0-or-later
//! BLI math helpers the BMesh core needs (`blenlib/intern/math_geom.cc`,
//! `math_vector.cc`, `math_vector_inline.cc`), written in Blender's operation order without
//! `mul_add` (ARCHITECTURE §21.3). Vector basics come from `frap_geom::normals`.

pub use frap_geom::normals::{V3, add, cross, dot, normalize_v3, safe_acos_approx, scale, sub};

/// `len_squared_v3`.
#[inline]
pub fn len_squared_v3(a: V3) -> f32 {
    dot(a, a)
}

/// `madd_v3_v3fl(r, a, f)`: `r += a * f`.
#[inline]
pub fn madd_v3_v3fl(r: &mut V3, a: V3, f: f32) {
    r[0] += a[0] * f;
    r[1] += a[1] * f;
    r[2] += a[2] * f;
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (normal_tri_v3).
/// `normal_tri_v3`: unit normal (zero for degenerate triangles) and the cross length.
pub fn normal_tri_v3(v1: V3, v2: V3, v3: V3) -> (V3, f32) {
    let n1 = [v1[0] - v2[0], v1[1] - v2[1], v1[2] - v2[2]];
    let n2 = [v2[0] - v3[0], v2[1] - v3[1], v2[2] - v3[2]];
    let n = [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ];
    normalize_v3(n)
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (normal_quad_v3).
/// `normal_quad_v3`: the "real cross" of the diagonals.
pub fn normal_quad_v3(v1: V3, v2: V3, v3: V3, v4: V3) -> (V3, f32) {
    let n1 = [v1[0] - v3[0], v1[1] - v3[1], v1[2] - v3[2]];
    let n2 = [v2[0] - v4[0], v2[1] - v4[1], v2[2] - v4[2]];
    let n = [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ];
    normalize_v3(n)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc (bm_face_calc_poly_normal).
/// Newell normal of a polygon given in loop order (starting at `l_first`).
pub fn normal_poly_newell(cos: &[V3]) -> (V3, f32) {
    let mut n = [0.0f32; 3];
    let Some(&last) = cos.last() else {
        return (n, 0.0);
    };
    let mut v_prev = last;
    for &v_curr in cos {
        n[0] += (v_prev[1] - v_curr[1]) * (v_prev[2] + v_curr[2]);
        n[1] += (v_prev[2] - v_curr[2]) * (v_prev[0] + v_curr[0]);
        n[2] += (v_prev[0] - v_curr[0]) * (v_prev[1] + v_curr[1]);
        v_prev = v_curr;
    }
    normalize_v3(n)
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (axis_dominant_v3_to_m3).
/// The two projection rows of `axis_dominant_v3_to_m3` (after its transpose):
/// `mul_v2_m3v3(r, m, co)` is `(dot(r0, co), dot(r1, co))` in Blender's order.
pub fn axis_dominant_v3_rows(normal: V3) -> [V3; 2] {
    let (x, y) = frap_geom::polyfill::ortho_basis_v3v3_v3(normal);
    [x, y]
}

/// `mul_v2_m3v3` with the rows of [`axis_dominant_v3_rows`].
#[inline]
pub fn project_v2(rows: &[V3; 2], a: V3) -> [f32; 2] {
    [
        rows[0][0] * a[0] + rows[0][1] * a[1] + rows[0][2] * a[2],
        rows[1][0] * a[0] + rows[1][1] * a[1] + rows[1][2] * a[2],
    ]
}

// Ported from blender/source/blender/blenlib/intern/math_vector.cc (ortho_v3_v3).
/// `ortho_v3_v3`: a vector orthogonal to `v`.
pub fn ortho_v3_v3(v: V3) -> V3 {
    let (x, y, z) = (v[0].abs(), v[1].abs(), v[2].abs());
    // axis_dominant_v3_single
    let axis = if x > y {
        if x > z { 0 } else { 2 }
    } else if y > z {
        1
    } else {
        2
    };
    match axis {
        0 => [-v[1] - v[2], v[0], v[0]],
        1 => [v[1], -v[0] - v[2], v[1]],
        _ => [v[2], v[2], -v[0] - v[1]],
    }
}

fn closest_to_line_segment_v2(p: [f32; 2], l1: [f32; 2], l2: [f32; 2]) -> [f32; 2] {
    let u = [l2[0] - l1[0], l2[1] - l1[1]];
    let h = [p[0] - l1[0], p[1] - l1[1]];
    let denom = u[0] * u[0] + u[1] * u[1];
    if denom == 0.0 {
        return l1;
    }
    let lambda = (u[0] * h[0] + u[1] * h[1]) / denom;
    if lambda <= 0.0 {
        return l1;
    }
    if lambda >= 1.0 {
        return l2;
    }
    [l1[0] + u[0] * lambda, l1[1] + u[1] * lambda]
}

fn line_point_factor_v2(p: [f32; 2], l1: [f32; 2], l2: [f32; 2]) -> f32 {
    let u = [l2[0] - l1[0], l2[1] - l1[1]];
    let h = [p[0] - l1[0], p[1] - l1[1]];
    let dot = u[0] * u[0] + u[1] * u[1];
    if dot > 0.0 {
        (u[0] * h[0] + u[1] * h[1]) / dot
    } else {
        0.0
    }
}

#[derive(Copy, Clone)]
struct Dir2 {
    dir: [f64; 2],
    len: f64,
}

fn dir_v2(a: [f32; 2], b: [f32; 2]) -> Dir2 {
    let dir = [a[0] as f64 - b[0] as f64, a[1] as f64 - b[1] as f64];
    Dir2 {
        dir,
        len: (dir[0] * dir[0] + dir[1] * dir[1]).sqrt(),
    }
}

fn mean_value_half_tan_v2_db(c: &Dir2, n: &Dir2) -> f64 {
    let area = c.dir[0] * n.dir[1] - c.dir[1] * n.dir[0];
    if area != 0.0 {
        let dot = c.dir[0] * n.dir[0] + c.dir[1] * n.dir[1];
        let len = c.len * n.len;
        let result = (len - dot) / area;
        if result.is_finite() {
            return result;
        }
    }
    0.0
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (interp_weights_poly_v2).
/// Mean-value interpolation weights of `co` inside the 2D polygon `v` (linear along an edge
/// and exact at a corner when `co` lies on the boundary).
pub fn interp_weights_poly_v2(v: &[[f32; 2]], co: [f32; 2]) -> Vec<f32> {
    let n = v.len();
    let mut w = vec![0.0f32; n];
    if n == 0 {
        return w;
    }
    let mut max_value = 0.0f32;
    for p in v {
        max_value = max_value.max((p[0] - co[0]).abs());
        max_value = max_value.max((p[1] - co[1]).abs());
    }
    let eps = 16.0 * f32::EPSILON * max_value;
    let eps_sq = eps * eps;

    let mut i_curr = n - 1;
    let mut i_next = 0usize;
    let mut v_curr = v[i_curr];
    let mut v_next = v[i_next];
    // `v_curr - 2`: v[n - 2] (wraps for triangles and quads alike since n >= 3 in practice).
    let mut d_curr = dir_v2(v[(n + n - 2) % n], co);
    let mut d_next = dir_v2(v_curr, co);
    let mut ht_prev = mean_value_half_tan_v2_db(&d_curr, &d_next);
    let mut totweight = 0.0f32;
    // 0 none, 1 point, 2 segment.
    let mut ix = 0u8;
    while i_next < n {
        if d_next.len < eps as f64 {
            ix = 1;
            break;
        }
        let c = closest_to_line_segment_v2(co, v_curr, v_next);
        let dsq = (c[0] - co[0]) * (c[0] - co[0]) + (c[1] - co[1]) * (c[1] - co[1]);
        if dsq < eps_sq {
            ix = 2;
            break;
        }
        d_curr = d_next;
        d_next = dir_v2(v_next, co);
        let ht = mean_value_half_tan_v2_db(&d_curr, &d_next);
        w[i_curr] = if d_curr.len == 0.0 {
            0.0
        } else {
            ((ht_prev + ht) / d_curr.len) as f32
        };
        totweight += w[i_curr];
        i_curr = i_next;
        i_next += 1;
        v_curr = v_next;
        v_next = v[i_next % n];
        ht_prev = ht;
    }
    if ix != 0 {
        w.iter_mut().for_each(|x| *x = 0.0);
        if ix == 1 {
            w[i_curr] = 1.0;
        } else {
            let fac = line_point_factor_v2(co, v_curr, v_next).clamp(0.0, 1.0);
            w[i_curr] = 1.0 - fac;
            w[i_next % n] = fac;
        }
    } else if totweight != 0.0 {
        for x in &mut w {
            *x /= totweight;
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_centre_weights_are_equal() {
        let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let w = interp_weights_poly_v2(&sq, [0.5, 0.5]);
        for x in w {
            assert!((x - 0.25).abs() < 1e-6);
        }
    }

    #[test]
    fn corner_and_edge_points() {
        let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert_eq!(
            interp_weights_poly_v2(&sq, [1.0, 0.0]),
            vec![0.0, 1.0, 0.0, 0.0]
        );
        let w = interp_weights_poly_v2(&sq, [0.25, 0.0]);
        assert!((w[0] - 0.75).abs() < 1e-6 && (w[1] - 0.25).abs() < 1e-6);
    }
}
