// SPDX-License-Identifier: GPL-3.0-or-later
//! BLI math helpers used by the bevel port, written in Blender's operation order without
//! `mul_add` (ARCHITECTURE §21.3); transcendentals come from `libm` (rule 2.2.12).
//!
//! Ported from blender/source/blender/blenlib/intern/{math_vector.cc, math_vector_inline.cc,
//! math_geom.cc, math_matrix_c.cc, math_base_inline.cc} (Blender 5.2.4, GPL-2.0-or-later).

pub(crate) use crate::query::geom::{V3, add, cross, dot, normalize_v3, scale, sub};

/// `float[4][4]` in Blender's layout: `m[col][row]`.
pub(crate) type M4 = [[f32; 4]; 4];

/// `M_PI` as a float (`float(M_PI)`).
pub(crate) const PI_F: f32 = std::f32::consts::PI;

/// `DEG2RADF(d)`: `d * float(M_PI / 180.0)`.
#[inline]
pub(crate) fn deg2radf(d: f32) -> f32 {
    d * (std::f64::consts::PI / 180.0) as f32
}

/// `madd_v3_v3v3fl(r, a, b, f)`: `a + b * f`.
#[inline]
pub(crate) fn madd(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

/// `negate_v3`.
#[inline]
pub(crate) fn neg(a: V3) -> V3 {
    [-a[0], -a[1], -a[2]]
}

/// `normalize_v3` keeping only the vector.
#[inline]
pub(crate) fn norm(a: V3) -> V3 {
    normalize_v3(a).0
}

/// `is_zero_v3`.
#[inline]
pub(crate) fn is_zero_v3(a: V3) -> bool {
    a[0] == 0.0 && a[1] == 0.0 && a[2] == 0.0
}

/// `len_squared_v3`.
#[inline]
pub(crate) fn len_sq(a: V3) -> f32 {
    dot(a, a)
}

/// `len_squared_v3v3(a, b)`.
#[inline]
pub(crate) fn len_sq_v3v3(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}

/// `len_v3v3(a, b)`.
#[inline]
pub(crate) fn len_v3v3(a: V3, b: V3) -> f32 {
    len_sq_v3v3(a, b).sqrt()
}

/// `interp_v3_v3v3(r, a, b, t)`.
#[inline]
pub(crate) fn interp(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `mid_v3_v3v3(r, a, b)`.
#[inline]
pub(crate) fn mid(a: V3, b: V3) -> V3 {
    [
        0.5 * (a[0] + b[0]),
        0.5 * (a[1] + b[1]),
        0.5 * (a[2] + b[2]),
    ]
}

/// `compare_ff(a, b, max_diff)`.
#[inline]
pub(crate) fn compare_ff(a: f32, b: f32, max_diff: f32) -> bool {
    (a - b).abs() <= max_diff
}

/// `compare_v3v3(a, b, limit)` (`limit` is a double literal in bevel; compared in float).
#[inline]
pub(crate) fn compare_v3v3(a: V3, b: V3, limit: f32) -> bool {
    compare_ff(a[0], b[0], limit) && compare_ff(a[1], b[1], limit) && compare_ff(a[2], b[2], limit)
}

/// `safe_asinf`.
#[inline]
fn safe_asinf(a: f32) -> f32 {
    libm::asinf(a.clamp(-1.0, 1.0))
}

/// `angle_normalized_v3v3`.
pub(crate) fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    PI_F - 2.0 * safe_asinf(len_v3v3(v1, neg(v2)) / 2.0)
}

/// `angle_v3v3`.
pub(crate) fn angle_v3v3(a: V3, b: V3) -> f32 {
    angle_normalized_v3v3(norm(a), norm(b))
}

/// `angle_v3v3v3(a, b, c)`: the angle at `b`.
pub(crate) fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    angle_normalized_v3v3(norm(sub(b, a)), norm(sub(b, c)))
}

/// `project_v3_v3v3(out, p, v_proj)`.
pub(crate) fn project_v3_v3v3(p: V3, v_proj: V3) -> V3 {
    if is_zero_v3(v_proj) {
        return [0.0; 3];
    }
    let mul = dot(p, v_proj) / dot(v_proj, v_proj);
    scale(v_proj, mul)
}

/// `isect_line_line_epsilon_v3` with `epsilon = 0.000001f` (`isect_line_line_v3`): returns
/// the kind (0 none, 1 one point, 2 nearest points of skew lines) and the two points.
pub(crate) fn isect_line_line_v3(v1: V3, v2: V3, v3: V3, v4: V3) -> (i32, V3, V3) {
    let epsilon = 0.000_001f32;
    let mut c = sub(v3, v1);
    let mut a = sub(v2, v1);
    let mut b = sub(v4, v3);
    let mut ab = cross(a, b);
    let d = dot(c, ab);
    let div = dot(ab, ab);
    if div == 0.0 {
        return (0, [0.0; 3], [0.0; 3]);
    }
    if d.abs() <= epsilon {
        let cb = cross(c, b);
        a = scale(a, dot(cb, ab) / div);
        let i1 = add(v1, a);
        return (1, i1, i1);
    }
    let mut t = sub(v1, v3);
    let n = cross(a, b);
    t = project_v3_v3v3(t, n);
    let v3t = add(v3, t);
    let v4t = add(v4, t);
    c = sub(v3t, v1);
    a = sub(v2, v1);
    b = sub(v4t, v3t);
    ab = cross(a, b);
    let cb = cross(c, b);
    a = scale(a, dot(cb, ab) / dot(ab, ab));
    let i1 = add(v1, a);
    let i2 = sub(i1, t);
    (2, i1, i2)
}

/// `isect_line_plane_v3`.
pub(crate) fn isect_line_plane_v3(l1: V3, l2: V3, plane_co: V3, plane_no: V3) -> Option<V3> {
    let u = sub(l2, l1);
    let h = sub(l1, plane_co);
    let d = dot(plane_no, u);
    if d.abs() > f32::EPSILON {
        let lambda = -dot(plane_no, h) / d;
        return Some(madd(l1, u, lambda));
    }
    None
}

/// `closest_to_line_v3` (`closest_to_ray_v3`): the closest point and its factor along
/// `l1 → l2`; `(l1, 0)` for a zero-length line (no 0/0 NaN).
pub(crate) fn closest_to_line_v3(p: V3, l1: V3, l2: V3) -> (V3, f32) {
    let ray_dir = sub(l2, l1);
    if ray_dir == [0.0; 3] {
        return (l1, 0.0);
    }
    let h = sub(p, l1);
    let lambda = dot(ray_dir, h) / dot(ray_dir, ray_dir);
    (madd(l1, ray_dir, lambda), lambda)
}

/// `closest_to_line_segment_v3`.
pub(crate) fn closest_to_line_segment_v3(p: V3, l1: V3, l2: V3) -> V3 {
    let (cp, lambda) = closest_to_line_v3(p, l1, l2);
    // Flip checks for the !finite case (when the segment is a point).
    if lambda <= 0.0 {
        return l1;
    }
    if lambda >= 1.0 {
        return l2;
    }
    cp
}

/// `dist_squared_to_line_segment_v3`.
pub(crate) fn dist_squared_to_line_segment_v3(p: V3, l1: V3, l2: V3) -> f32 {
    let c = closest_to_line_segment_v3(p, l1, l2);
    let d = sub(p, c);
    dot(d, d)
}

/// `plane_from_point_normal_v3`.
pub(crate) fn plane_from_point_normal_v3(co: V3, no: V3) -> [f32; 4] {
    [no[0], no[1], no[2], -dot(no, co)]
}

#[inline]
fn plane_point_side_v3(plane: [f32; 4], co: V3) -> f32 {
    dot(co, [plane[0], plane[1], plane[2]]) + plane[3]
}

/// `closest_to_plane_v3`.
pub(crate) fn closest_to_plane_v3(plane: [f32; 4], pt: V3) -> V3 {
    let pn = [plane[0], plane[1], plane[2]];
    let len_sq = dot(pn, pn);
    let side = plane_point_side_v3(plane, pt);
    madd(pt, pn, -side / len_sq)
}

/// `closest_to_plane_normalized_v3`.
pub(crate) fn closest_to_plane_normalized_v3(plane: [f32; 4], pt: V3) -> V3 {
    let side = plane_point_side_v3(plane, pt);
    madd(pt, [plane[0], plane[1], plane[2]], -side)
}

/// `interp_bilinear_quad_v3(data, u, v, res)`.
pub(crate) fn interp_bilinear_quad_v3(data: [V3; 4], u: f32, v: f32) -> V3 {
    let mut res = scale(data[0], (1.0 - u) * (1.0 - v));
    res = add(res, scale(data[1], u * (1.0 - v)));
    res = add(res, scale(data[2], u * v));
    add(res, scale(data[3], (1.0 - u) * v))
}

/// `mul_v3_m4v3(r, mat, vec)`.
pub(crate) fn mul_v3_m4v3(m: &M4, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * v[2] + m[3][0],
        x * m[0][1] + y * m[1][1] + m[2][1] * v[2] + m[3][1],
        x * m[0][2] + y * m[1][2] + m[2][2] * v[2] + m[3][2],
    ]
}

/// `mul_v4_m4v4(r, mat, v)` (`mul_m4_v4`).
pub(crate) fn mul_v4_m4v4(m: &M4, v: [f32; 4]) -> [f32; 4] {
    let (x, y, z) = (v[0], v[1], v[2]);
    [
        x * m[0][0] + y * m[1][0] + z * m[2][0] + m[3][0] * v[3],
        x * m[0][1] + y * m[1][1] + z * m[2][1] + m[3][1] * v[3],
        x * m[0][2] + y * m[1][2] + z * m[2][2] + m[3][2] * v[3],
        x * m[0][3] + y * m[1][3] + z * m[2][3] + m[3][3] * v[3],
    ]
}

/// `invert_m4_m4_fallback`: Gauss-Jordan elimination with partial pivoting (Blender itself
/// calls Eigen's 4x4 inverse; results agree to float precision). `None` when singular.
pub(crate) fn invert_m4_m4(mat: &M4) -> Option<M4> {
    let mut inverse = [[0.0f32; 4]; 4];
    for (i, row) in inverse.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    let mut tempmat = *mat;
    for i in 0..4 {
        // Look for row with max pivot.
        let mut max = tempmat[i][i].abs();
        let mut maxj = i;
        for (j, row) in tempmat.iter().enumerate().skip(i + 1) {
            if row[i].abs() > max {
                max = row[i].abs();
                maxj = j;
            }
        }
        // Swap rows if necessary.
        if maxj != i {
            tempmat.swap(i, maxj);
            inverse.swap(i, maxj);
        }
        let temp = tempmat[i][i];
        if temp == 0.0 {
            return None; // No non-zero pivot.
        }
        for k in 0..4 {
            tempmat[i][k] = (f64::from(tempmat[i][k]) / f64::from(temp)) as f32;
            inverse[i][k] = (f64::from(inverse[i][k]) / f64::from(temp)) as f32;
        }
        for j in 0..4 {
            if j != i {
                let temp = tempmat[j][i];
                for k in 0..4 {
                    tempmat[j][k] -= (f64::from(tempmat[i][k]) * f64::from(temp)) as f32;
                    inverse[j][k] -= (f64::from(inverse[i][k]) * f64::from(temp)) as f32;
                }
            }
        }
    }
    Some(inverse)
}

/// `cross_poly_v2` (trapezium rule).
fn cross_poly_v2(verts: &[[f32; 2]]) -> f32 {
    let nr = verts.len();
    let mut co_prev = verts[nr - 1];
    let mut cross = 0.0f32;
    for &co_curr in verts {
        cross += (co_prev[0] - co_curr[0]) * (co_curr[1] + co_prev[1]);
        co_prev = co_curr;
    }
    cross
}

/// `area_poly_v2`.
pub(crate) fn area_poly_v2(verts: &[[f32; 2]]) -> f32 {
    (0.5 * cross_poly_v2(verts)).abs()
}

/// `isect_point_poly_v2`.
pub(crate) fn isect_point_poly_v2(pt: [f32; 2], verts: &[[f32; 2]]) -> bool {
    let nr = verts.len();
    let mut isect = false;
    let mut j = nr - 1;
    for i in 0..nr {
        if ((verts[i][1] > pt[1]) != (verts[j][1] > pt[1]))
            && (pt[0]
                < (verts[j][0] - verts[i][0]) * (pt[1] - verts[i][1]) / (verts[j][1] - verts[i][1])
                    + verts[i][0])
        {
            isect = !isect;
        }
        j = i;
    }
    isect
}

/// `safe_divide`.
#[inline]
pub(crate) fn safe_divide(a: f32, b: f32) -> f32 {
    if b == 0.0 { 0.0 } else { a / b }
}

/// `is_power_of_2_i`.
#[inline]
pub(crate) fn is_power_of_2_i(n: i32) -> bool {
    (n & (n - 1)) == 0
}

/// `power_of_2_max_i`: the smallest power of two `>= n`.
pub(crate) fn power_of_2_max_i(mut n: i32) -> i32 {
    if is_power_of_2_i(n) {
        return n;
    }
    loop {
        n &= n - 1;
        if is_power_of_2_i(n) {
            break;
        }
    }
    n * 2
}

/// Dense least-squares solve (`EIG_linear_least_squares_solver`): `MᵀM x = Mᵀ b` in double,
/// as Blender's Eigen solver forms it (sparse LU of `MᵀM`; here Gaussian elimination with
/// partial pivoting, which agrees to double precision). `entries` are summed `(row, col,
/// value)` triplets. `None` when `MᵀM` is singular.
pub(crate) fn least_squares(
    nrows: usize,
    ncols: usize,
    entries: &[(usize, usize, f64)],
    b: &[f64],
) -> Option<Vec<f64>> {
    let mut m = vec![vec![0.0f64; ncols]; nrows];
    for &(r, c, v) in entries {
        m[r][c] += v;
    }
    let mut a = vec![vec![0.0f64; ncols]; ncols];
    let mut rhs = vec![0.0f64; ncols];
    for i in 0..ncols {
        for j in 0..ncols {
            let mut s = 0.0;
            for row in &m {
                s += row[i] * row[j];
            }
            a[i][j] = s;
        }
        let mut s = 0.0;
        for (k, row) in m.iter().enumerate() {
            s += row[i] * b[k];
        }
        rhs[i] = s;
    }
    for col in 0..ncols {
        let mut piv = col;
        for r in col + 1..ncols {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col] == 0.0 {
            return None;
        }
        a.swap(col, piv);
        rhs.swap(col, piv);
        let (top, rest) = a.split_at_mut(col + 1);
        let pivot_row = &top[col];
        for (k, row) in rest.iter_mut().enumerate() {
            let f = row[col] / pivot_row[col];
            if f != 0.0 {
                for (x, &p) in row.iter_mut().zip(pivot_row.iter()).skip(col) {
                    *x -= f * p;
                }
                rhs[col + 1 + k] -= f * rhs[col];
            }
        }
    }
    let mut x = vec![0.0f64; ncols];
    for i in (0..ncols).rev() {
        let mut s = rhs[i];
        for c in i + 1..ncols {
            s -= a[i][c] * x[c];
        }
        x[i] = s / a[i][i];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_of_two() {
        assert_eq!(power_of_2_max_i(1), 1);
        assert_eq!(power_of_2_max_i(3), 4);
        assert_eq!(power_of_2_max_i(4), 4);
        assert_eq!(power_of_2_max_i(5), 8);
        assert_eq!(power_of_2_max_i(1000), 1024);
    }

    #[test]
    fn line_line() {
        let (k, p, _) = isect_line_line_v3(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.5, -1.0, 0.0],
            [0.5, 1.0, 0.0],
        );
        assert_eq!(k, 1);
        assert!((p[0] - 0.5).abs() < 1e-6 && p[1].abs() < 1e-6);
        let (k, a, b) = isect_line_line_v3(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.5, -1.0, 1.0],
            [0.5, 1.0, 1.0],
        );
        assert_eq!(k, 2);
        assert!((a[0] - 0.5).abs() < 1e-6 && a[2].abs() < 1e-6);
        assert!((b[2] - 1.0).abs() < 1e-6);
        let (k, _, _) =
            isect_line_line_v3([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [2.0, 1.0, 0.0]);
        assert_eq!(k, 0);
    }

    #[test]
    fn inverse_and_lsq() {
        let m: M4 = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 4.0, 0.0, 0.0],
            [1.0, 0.0, 1.0, 0.0],
            [3.0, 2.0, 1.0, 1.0],
        ];
        let inv = invert_m4_m4(&m).unwrap();
        let p = mul_v3_m4v3(&m, [1.0, 2.0, 3.0]);
        let q = mul_v3_m4v3(&inv, p);
        assert!(len_v3v3(q, [1.0, 2.0, 3.0]) < 1e-5);
        // x = 1, y = 2 from an overdetermined consistent system.
        let x = least_squares(
            3,
            2,
            &[(0, 0, 1.0), (1, 1, 1.0), (2, 0, 1.0), (2, 1, 1.0)],
            &[1.0, 2.0, 3.0],
        )
        .unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12 && (x[1] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn angles() {
        assert!((angle_v3v3([1.0, 0.0, 0.0], [0.0, 2.0, 0.0]) - PI_F / 2.0).abs() < 1e-6);
        assert!((angle_v3v3([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]) - PI_F).abs() < 1e-6);
        let poly = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert!((area_poly_v2(&poly) - 1.0).abs() < 1e-6);
        assert!(isect_point_poly_v2([0.5, 0.5], &poly));
        assert!(!isect_point_poly_v2([1.5, 0.5], &poly));
    }
}
