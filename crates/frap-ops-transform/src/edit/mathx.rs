// SPDX-License-Identifier: GPL-3.0-or-later
//! BLI math helpers the edit-mode transforms need, with Blender's exact semantics and
//! operation order (no `mul_add`, ARCHITECTURE §21.3). Ported from
//! `blender/source/blender/blenlib/intern/{math_vector.cc, math_vector_inline.cc,
//! math_geom.cc, math_geom_inline.cc, math_matrix_c.cc, math_rotation_c.cc}` (Blender 5.2.4,
//! GPL-2.0-or-later). Vectors are `glam::Vec3`, matrices `glam::Mat3`/`Mat4` (columns are
//! Blender's `m[i]`).

use glam::{Mat3, Mat4, Vec2, Vec3};

/// `FLT_EPSILON`.
pub const FLT_EPSILON: f32 = f32::EPSILON;

/// `dot_v3v3` (left to right).
#[inline]
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// `cross_v3_v3v3`.
#[inline]
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

/// `len_squared_v3`.
#[inline]
pub fn len_sq(a: Vec3) -> f32 {
    dot(a, a)
}

/// `len_v3`.
#[inline]
pub fn len(a: Vec3) -> f32 {
    len_sq(a).sqrt()
}

/// `normalize_v3_v3_length`: `(a scaled to unit_length, original length)`; zero (and length 0)
/// at or below a squared length of 1e-35.
pub fn normalize_length(a: Vec3, unit_length: f32) -> (Vec3, f32) {
    let d = dot(a, a);
    if d > 1.0e-35 {
        let d = d.sqrt();
        (a * (unit_length / d), d)
    } else {
        (Vec3::ZERO, 0.0)
    }
}

/// `normalize_v3_v3`: `(unit vector, original length)`.
#[inline]
pub fn normalize(a: Vec3) -> (Vec3, f32) {
    normalize_length(a, 1.0)
}

/// `normalize_v3` keeping only the vector.
#[inline]
pub fn normalized(a: Vec3) -> Vec3 {
    normalize(a).0
}

/// `is_zero_v3`.
#[inline]
pub fn is_zero(a: Vec3) -> bool {
    a.x == 0.0 && a.y == 0.0 && a.z == 0.0
}

/// `mid_v3_v3v3`.
#[inline]
pub fn mid(a: Vec3, b: Vec3) -> Vec3 {
    (a + b) * 0.5
}

/// `madd_v3_v3v3fl`: `a + b * f`.
#[inline]
pub fn madd(a: Vec3, b: Vec3, f: f32) -> Vec3 {
    Vec3::new(a.x + b.x * f, a.y + b.y * f, a.z + b.z * f)
}

/// `madd_v3_v3fl(no, f_no, safe_acos_approx(x))` (`bm_loop_normal_accum`).
#[inline]
pub fn add_safe_acos(no: Vec3, f_no: Vec3, x: f32) -> Vec3 {
    madd(no, f_no, frap_geom::normals::safe_acos_approx(x))
}

/// `interp_v3_v3v3`.
#[inline]
pub fn interp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let s = 1.0 - t;
    Vec3::new(s * a.x + t * b.x, s * a.y + t * b.y, s * a.z + t * b.z)
}

/// `project_v3_v3v3`: the projection of `p` on `v_proj` (zero for a zero `v_proj`).
pub fn project(p: Vec3, v_proj: Vec3) -> Vec3 {
    if is_zero(v_proj) {
        return Vec3::ZERO;
    }
    let mul = dot(p, v_proj) / dot(v_proj, v_proj);
    v_proj * mul
}

/// `project_plane_v3_v3v3`.
pub fn project_plane(p: Vec3, v_plane: Vec3) -> Vec3 {
    let mul = dot(p, v_plane) / dot(v_plane, v_plane);
    madd(p, v_plane, -mul)
}

/// `project_plane_normalized_v3_v3v3`.
pub fn project_plane_normalized(p: Vec3, v_plane: Vec3) -> Vec3 {
    let mul = dot(p, v_plane);
    madd(p, v_plane, -mul)
}

/// `reflect_v3_v3v3`: `v` reflected by the plane of unit normal `normal`.
pub fn reflect(v: Vec3, normal: Vec3) -> Vec3 {
    let dot2 = 2.0 * dot(v, normal);
    Vec3::new(
        v.x - dot2 * normal.x,
        v.y - dot2 * normal.y,
        v.z - dot2 * normal.z,
    )
}

/// `saasin` / `safe_asinf`.
fn safe_asinf(a: f32) -> f32 {
    if a <= -1.0 {
        -std::f32::consts::FRAC_PI_2
    } else if a >= 1.0 {
        std::f32::consts::FRAC_PI_2
    } else {
        libm::asinf(a)
    }
}

/// `angle_normalized_v3v3`.
pub fn angle_normalized(v1: Vec3, v2: Vec3) -> f32 {
    if dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len(v1 - v2) / 2.0);
    }
    std::f32::consts::PI - 2.0 * safe_asinf(len(v1 + v2) / 2.0)
}

/// `angle_v3v3v3`: the angle at `b`.
pub fn angle_v3v3v3(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    angle_normalized(normalized(b - a), normalized(b - c))
}

/// `ortho_basis_v3v3_v3`.
pub fn ortho_basis(n: Vec3) -> (Vec3, Vec3) {
    let f = n.x * n.x + n.y * n.y;
    if f > FLT_EPSILON {
        let d = 1.0 / f.sqrt();
        let n1 = Vec3::new(n.y * d, -n.x * d, 0.0);
        let n2 = Vec3::new(-n.z * n1.y, n.z * n1.x, n.x * n1.y - n.y * n1.x);
        (n1, n2)
    } else {
        let n1 = Vec3::new(if n.z < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0);
        (n1, Vec3::new(0.0, 1.0, 0.0))
    }
}

/// `ortho_v3_v3`: a vector orthogonal to `v` (Blender's dominant-axis choice).
pub fn ortho(v: Vec3) -> Vec3 {
    Vec3::from_array(frap_bmesh::query::geom::ortho_v3_v3(v.to_array()))
}

/// `axis_dominant_v3_to_m3`: rows are an orthonormal basis whose last row is `normal`
/// (returned as a column matrix, i.e. Blender's `r_mat`).
pub fn axis_dominant_v3_to_m3(normal: Vec3) -> Mat3 {
    let (a, b) = ortho_basis(normal);
    // `r_mat[0..2] = (a, b, normal)` then transposed.
    Mat3::from_cols(a, b, normal).transpose()
}

/// `invert_m3_m3` with Blender's determinant test (`false` and a zero matrix when singular).
pub fn invert_m3(m: &Mat3) -> Option<Mat3> {
    let det = m.determinant();
    if det == 0.0 {
        return None;
    }
    Some(m.inverse())
}

/// `normalize_m3`: every column unit length (zero columns stay zero).
pub fn normalize_m3(m: &Mat3) -> Mat3 {
    Mat3::from_cols(
        normalized(m.x_axis),
        normalized(m.y_axis),
        normalized(m.z_axis),
    )
}

/// `interp_dot_slerp`.
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

/// `interp_v3_v3v3_slerp` (`None` for direct opposites).
pub fn slerp(a: Vec3, b: Vec3, t: f32) -> Option<Vec3> {
    let cosom = dot(a, b);
    if cosom < -1.0 + FLT_EPSILON {
        return None;
    }
    let w = interp_dot_slerp(t, cosom);
    Some(Vec3::new(
        w[0] * a.x + w[1] * b.x,
        w[0] * a.y + w[1] * b.y,
        w[0] * a.z + w[1] * b.z,
    ))
}

/// `interp_v3_v3v3_slerp_safe`.
pub fn slerp_safe(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    if let Some(r) = slerp(a, b, t) {
        return r;
    }
    let ab_ortho = normalized(ortho(a));
    if t < 0.5 {
        slerp(a, ab_ortho, t * 2.0).unwrap_or(a)
    } else {
        slerp(ab_ortho, b, (t - 0.5) * 2.0).unwrap_or(b)
    }
}

/// `line_point_factor_v3`.
pub fn line_point_factor(p: Vec3, l1: Vec3, l2: Vec3) -> f32 {
    let u = l2 - l1;
    let h = p - l1;
    let d = len_sq(u);
    if d > 0.0 { dot(u, h) / d } else { 0.0 }
}

/// `isect_line_plane_v3`.
pub fn isect_line_plane(l1: Vec3, l2: Vec3, plane_co: Vec3, plane_no: Vec3) -> Option<Vec3> {
    let u = l2 - l1;
    let h = l1 - plane_co;
    let d = dot(plane_no, u);
    if d.abs() > FLT_EPSILON {
        let lambda = -dot(plane_no, h) / d;
        return Some(madd(l1, u, lambda));
    }
    None
}

/// `isect_line_line_epsilon_v3`: 0 (no intersection), 1 (`r.0 == r.1`) or 2 (closest
/// points).
pub fn isect_line_line_epsilon(
    v1: Vec3,
    v2: Vec3,
    v3: Vec3,
    v4: Vec3,
    epsilon: f32,
) -> (i32, Vec3, Vec3) {
    let c = v3 - v1;
    let a = v2 - v1;
    let b = v4 - v3;
    let ab = cross(a, b);
    let d = dot(c, ab);
    let div = dot(ab, ab);
    if div == 0.0 {
        return (0, Vec3::ZERO, Vec3::ZERO);
    }
    if d.abs() <= epsilon {
        let cb = cross(c, b);
        let a = a * (dot(cb, ab) / div);
        let i1 = v1 + a;
        return (1, i1, i1);
    }
    let t = v1 - v3;
    let n = cross(a, b);
    let t = project(t, n);
    let v3t = v3 + t;
    let v4t = v4 + t;
    let c = v3t - v1;
    let a = v2 - v1;
    let b = v4t - v3t;
    let ab = cross(a, b);
    let cb = cross(c, b);
    let a = a * (dot(cb, ab) / dot(ab, ab));
    let i1 = v1 + a;
    (2, i1, i1 - t)
}

/// `dist_signed_squared_to_plane3_v3`.
fn dist_signed_squared_to_plane3(p: Vec3, plane: Vec3) -> f32 {
    let len_sq = len_sq(plane);
    let side = dot(plane, p);
    let fac = side / len_sq;
    (len_sq * (fac * fac)).copysign(side)
}

/// `dist_signed_squared_to_corner_v3v3v3`.
pub fn dist_signed_squared_to_corner(p: Vec3, v1: Vec3, v2: Vec3, v3: Vec3, axis_ref: Vec3) -> f32 {
    let dir_a = v1 - v2;
    let dir_b = v3 - v2;
    let mut axis = cross(dir_a, dir_b);
    let mut flip = false;
    if len_sq(axis) < FLT_EPSILON {
        axis = axis_ref;
    } else if dot(axis, axis_ref) < 0.0 {
        flip = true;
        axis = -axis;
    }
    let plane_a = cross(dir_a, axis);
    let plane_b = cross(axis, dir_b);
    let s_p_v2 = p - v2;
    let dist_a = dist_signed_squared_to_plane3(s_p_v2, plane_a);
    let dist_b = dist_signed_squared_to_plane3(s_p_v2, plane_b);
    if flip {
        dist_a.min(dist_b)
    } else {
        dist_a.max(dist_b)
    }
}

/// `geodesic_distance_propagate_across_triangle`.
pub fn geodesic_distance_propagate_across_triangle(
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
    dist1: f32,
    dist2: f32,
) -> f32 {
    let v10 = v0 - v1;
    let v12 = v2 - v1;
    if dist1 != 0.0 && dist2 != 0.0 {
        let (u, d12) = normalize(v12);
        if d12 * d12 > 0.0 {
            let n = normalized(cross(v12, v10));
            let v = cross(n, u);
            let v0_ = Vec2::new(dot(v10, u), dot(v10, v).abs());
            let a = 0.5 * (1.0 + (dist1 * dist1 - dist2 * dist2) / (d12 * d12));
            let hh = dist1 * dist1 - a * a * d12 * d12;
            if hh > 0.0 {
                let h = hh.sqrt();
                let s_ = Vec2::new(a * d12, -h);
                let x_intercept = s_.x + h * (v0_.x - s_.x) / (v0_.y + h);
                if x_intercept >= 0.0 && x_intercept <= d12 {
                    let dx = s_.x - v0_.x;
                    let dy = s_.y - v0_.y;
                    let dist0 = (dx * dx + dy * dy).sqrt();
                    if dist0 >= dist1.min(dist2) {
                        return dist0;
                    }
                }
            }
        }
    }
    (dist1 + len(v10)).min(dist2 + len(v0 - v2))
}

/// `axis_sort_v3`: the axis order by increasing value.
pub fn axis_sort_v3(values: [f32; 3]) -> [usize; 3] {
    let mut v = values;
    let mut order = [0usize, 1, 2];
    let swap = |v: &mut [f32; 3], o: &mut [usize; 3], a: usize, b: usize| {
        v.swap(a, b);
        o.swap(a, b);
    };
    if v[0] < v[1] {
        if v[2] < v[0] {
            swap(&mut v, &mut order, 0, 2);
        }
    } else if v[1] < v[2] {
        swap(&mut v, &mut order, 0, 1);
    } else {
        swap(&mut v, &mut order, 0, 2);
    }
    if v[2] < v[1] {
        swap(&mut v, &mut order, 1, 2);
    }
    order
}

/// `mat3_to_size`: the column lengths.
pub fn mat3_to_size(m: &Mat3) -> Vec3 {
    Vec3::new(len(m.x_axis), len(m.y_axis), len(m.z_axis))
}

/// `transform_orientations_create_from_axis` (with `None` for a missing axis); `None` when no
/// usable matrix can be built (Blender then writes the identity and returns false).
pub fn create_from_axis(x: Option<Vec3>, y: Option<Vec3>, z: Option<Vec3>) -> (Mat3, bool) {
    let mut cols = [Vec3::ZERO; 3];
    let mut zero = [true; 3];
    for (i, a) in [x, y, z].into_iter().enumerate() {
        if let Some(a) = a {
            let (n, l) = normalize(a);
            cols[i] = n;
            zero[i] = l == 0.0;
        }
    }
    let zero_axis = zero.iter().filter(|z| **z).count();
    if zero_axis == 0 {
        return (Mat3::from_cols(cols[0], cols[1], cols[2]), true);
    }
    if zero_axis == 1 {
        let axis = zero.iter().position(|z| *z).unwrap_or(2);
        let (n, l) = normalize(cross(cols[(axis + 1) % 3], cols[(axis + 2) % 3]));
        cols[axis] = n;
        if l != 0.0 {
            return (Mat3::from_cols(cols[0], cols[1], cols[2]), true);
        }
    } else if zero_axis == 2 {
        let axis = zero.iter().position(|z| !*z).unwrap_or(2);
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        cols[a][a] = 1.0;
        cols[b][b] = 1.0;
        cols[a] = project_plane(cols[a], cols[axis]);
        cols[b] = project_plane(cols[b], cols[axis]);
        let (na, la) = normalize(cols[a]);
        cols[a] = na;
        if la != 0.0 {
            let (nb, lb) = normalize(cols[b]);
            cols[b] = nb;
            if lb != 0.0 {
                return (Mat3::from_cols(cols[0], cols[1], cols[2]), true);
            }
        }
    }
    (Mat3::IDENTITY, false)
}

/// `createSpaceNormal`: Z along `normal`, X from `Z × (0, 0, 1)` (or `(1, 0, 0) × Z`).
pub fn create_space_normal(normal: Vec3) -> Option<Mat3> {
    let (z, l) = normalize(normal);
    if l == 0.0 {
        return None;
    }
    let mut x = cross(z, Vec3::Z);
    if is_zero(x) {
        x = cross(Vec3::X, z);
    }
    let y = cross(z, x);
    Some(normalize_m3(&Mat3::from_cols(x, y, z)))
}

/// `createSpaceNormalTangent` (`normal`, `tangent` expected unit length).
pub fn create_space_normal_tangent(normal: Vec3, tangent: Vec3) -> Option<Mat3> {
    if is_zero(normal) {
        return None;
    }
    let z = normal;
    let mut y = -tangent;
    if is_zero(y) {
        y.z = 1.0;
    }
    let (x, lx) = normalize(cross(z, y));
    if lx == 0.0 {
        return None;
    }
    let (y, ly) = normalize(cross(z, x));
    if ly == 0.0 {
        return None;
    }
    Some(Mat3::from_cols(x, y, z))
}

/// `createSpaceNormalTangent_or_fallback`.
pub fn create_space_normal_tangent_or_fallback(normal: Vec3, tangent: Vec3) -> Mat3 {
    if let Some(m) = create_space_normal_tangent(normal, tangent) {
        return m;
    }
    if !is_zero(normal) {
        let m = axis_dominant_v3_to_m3(normal);
        return invert_m3(&m).unwrap_or(Mat3::ZERO);
    }
    Mat3::IDENTITY
}

/// `invert_m4_m4_fallback`: Eigen's inverse for an invertible matrix, otherwise Gauss-Jordan
/// with partial pivoting (which stops at the first zero pivot, leaving a partial result, as
/// Blender's does).
pub fn invert_m4_fallback(mat: &Mat4) -> Mat4 {
    if mat.determinant() != 0.0 {
        return mat.inverse();
    }
    // Blender's `float m[4][4]` is column-major like glam: `m[i][j]` = column i, row j.
    let mut inv = [[0.0f32; 4]; 4];
    for (i, r) in inv.iter_mut().enumerate() {
        r[i] = 1.0;
    }
    let mut t = mat.to_cols_array_2d();
    for i in 0..4 {
        let mut max = t[i][i].abs();
        let mut maxj = i;
        for (j, tj) in t.iter().enumerate().skip(i + 1) {
            if tj[i].abs() > max {
                max = tj[i].abs();
                maxj = j;
            }
        }
        if maxj != i {
            t.swap(i, maxj);
            inv.swap(i, maxj);
        }
        if t[i][i] == 0.0 {
            break;
        }
        let temp = f64::from(t[i][i]);
        for k in 0..4 {
            t[i][k] = (f64::from(t[i][k]) / temp) as f32;
            inv[i][k] = (f64::from(inv[i][k]) / temp) as f32;
        }
        for j in 0..4 {
            if j != i {
                let temp = f64::from(t[j][i]);
                for k in 0..4 {
                    t[j][k] -= (f64::from(t[i][k]) * temp) as f32;
                    inv[j][k] -= (f64::from(inv[i][k]) * temp) as f32;
                }
            }
        }
    }
    Mat4::from_cols_array_2d(&inv)
}

/// `pseudoinverse_m3_m3(.., PSEUDOINVERSE_EPSILON)`: the inverse, or the truncated-SVD
/// pseudo-inverse of a singular matrix (an object scaled to zero along one axis can still
/// move along the others).
pub fn pseudoinverse_m3(mat: &Mat3) -> Mat3 {
    if let Some(inv) = invert_m3(mat) {
        return inv;
    }
    let m4 = Mat4::from_mat3(*mat);
    let inv = pseudoinverse_m4(&m4, PSEUDOINVERSE_EPSILON);
    Mat3::from_mat4(inv)
}

/// `PSEUDOINVERSE_EPSILON`.
pub const PSEUDOINVERSE_EPSILON: f32 = 1e-8;

type M4 = [[f32; 4]; 4];

fn transpose4(m: &M4) -> M4 {
    let mut r = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = m[j][i];
        }
    }
    r
}

fn mul_m4(a: &M4, b: &M4) -> M4 {
    // `mul_m4_m4m4(R, A, B)`: R = A * B with column-major `m[col][row]`.
    let mut r = [[0.0f32; 4]; 4];
    for (i, ri) in r.iter_mut().enumerate() {
        for (j, rij) in ri.iter_mut().enumerate() {
            *rij = b[i][0] * a[0][j] + b[i][1] * a[1][j] + b[i][2] * a[2][j] + b[i][3] * a[3][j];
        }
    }
    r
}

/// `pseudoinverse_m4_m4`.
fn pseudoinverse_m4(mat: &Mat4, epsilon: f32) -> Mat4 {
    let a = transpose4(&mat.to_cols_array_2d());
    let (v, w, u) = svd_m4(&a);
    let u = transpose4(&u);
    let v = transpose4(&v);
    let mut wm = [[0.0f32; 4]; 4];
    for i in 0..4 {
        wm[i][i] = if w[i] < epsilon { 0.0 } else { 1.0 / w[i] };
    }
    let v = transpose4(&v);
    let r = mul_m4(&mul_m4(&u, &wm), &v);
    Mat4::from_cols_array_2d(&r)
}

// Ported from blender/source/blender/blenlib/intern/math_matrix_c.cc (svd_m4, from TNT).
/// `svd_m4(U, s, V, A)`: returns `(U, s, V)`. Indices are `A[i][j]` as in the C source.
#[allow(clippy::needless_range_loop, clippy::too_many_lines)]
fn svd_m4(a_in: &M4) -> (M4, [f32; 4], M4) {
    let mut a = *a_in;
    let mut u = [[0.0f32; 4]; 4];
    let mut v = [[0.0f32; 4]; 4];
    let mut s = [0.0f32; 4];
    let mut work = [0.0f32; 4];
    let mut e = [0.0f32; 4];
    let m = 4usize;
    let n = 4usize;
    let mut maxiter = 200;
    let nu = m.min(n);
    let nct = (m - 1).min(n);
    let nrt = (n - 2).min(m);

    for k in 0..nct.max(nrt) {
        if k < nct {
            s[k] = 0.0;
            for i in k..m {
                s[k] = libm::hypotf(s[k], a[i][k]);
            }
            if s[k] != 0.0 {
                if a[k][k] < 0.0 {
                    s[k] = -s[k];
                }
                let invsk = 1.0 / s[k];
                for i in k..m {
                    a[i][k] *= invsk;
                }
                a[k][k] += 1.0;
            }
            s[k] = -s[k];
        }
        for j in (k + 1)..n {
            if k < nct && s[k] != 0.0 {
                let mut t = 0.0f32;
                for i in k..m {
                    t += a[i][k] * a[i][j];
                }
                t = -t / a[k][k];
                for i in k..m {
                    a[i][j] += t * a[i][k];
                }
            }
            e[j] = a[k][j];
        }
        if k < nct {
            for i in k..m {
                u[i][k] = a[i][k];
            }
        }
        if k < nrt {
            e[k] = 0.0;
            for i in (k + 1)..n {
                e[k] = libm::hypotf(e[k], e[i]);
            }
            if e[k] != 0.0 {
                if e[k + 1] < 0.0 {
                    e[k] = -e[k];
                }
                let invek = 1.0 / e[k];
                for i in (k + 1)..n {
                    e[i] *= invek;
                }
                e[k + 1] += 1.0;
            }
            e[k] = -e[k];
            if k + 1 < m && e[k] != 0.0 {
                for i in (k + 1)..m {
                    work[i] = 0.0;
                }
                for j in (k + 1)..n {
                    for i in (k + 1)..m {
                        work[i] += e[j] * a[i][j];
                    }
                }
                let invek1 = 1.0 / e[k + 1];
                for j in (k + 1)..n {
                    let t = -e[j] * invek1;
                    for i in (k + 1)..m {
                        a[i][j] += t * work[i];
                    }
                }
            }
            for i in (k + 1)..n {
                v[i][k] = e[i];
            }
        }
    }

    let mut p = n.min(m + 1);
    if nct < n {
        s[nct] = a[nct][nct];
    }
    if m < p {
        s[p - 1] = 0.0;
    }
    if nrt + 1 < p {
        e[nrt] = a[nrt][p - 1];
    }
    e[p - 1] = 0.0;

    for j in nct..nu {
        for i in 0..m {
            u[i][j] = 0.0;
        }
        u[j][j] = 1.0;
    }
    for k in (0..nct).rev() {
        if s[k] != 0.0 {
            for j in (k + 1)..nu {
                let mut t = 0.0f32;
                for i in k..m {
                    t += u[i][k] * u[i][j];
                }
                t = -t / u[k][k];
                for i in k..m {
                    u[i][j] += t * u[i][k];
                }
            }
            for i in k..m {
                u[i][k] = -u[i][k];
            }
            u[k][k] += 1.0;
            // `for (i = 0; i < k - 1; i++)` with a signed `k`.
            for i in 0..k.saturating_sub(1) {
                u[i][k] = 0.0;
            }
        } else {
            for i in 0..m {
                u[i][k] = 0.0;
            }
            u[k][k] = 1.0;
        }
    }

    for k in (0..n).rev() {
        if k < nrt && e[k] != 0.0 {
            for j in (k + 1)..nu {
                let mut t = 0.0f32;
                for i in (k + 1)..n {
                    t += v[i][k] * v[i][j];
                }
                t = -t / v[k + 1][k];
                for i in (k + 1)..n {
                    v[i][j] += t * v[i][k];
                }
            }
        }
        for i in 0..n {
            v[i][k] = 0.0;
        }
        v[k][k] = 1.0;
    }

    let pp = p - 1;
    let eps = libm::powf(2.0, -52.0);
    while p > 0 {
        if maxiter == 0 {
            break;
        }
        maxiter -= 1;
        // `k` runs from `p - 2` down to -1 (signed).
        let mut k: isize = p as isize - 2;
        while k >= -1 {
            if k == -1 {
                break;
            }
            let ku = k as usize;
            if e[ku].abs() <= eps * (s[ku].abs() + s[ku + 1].abs()) {
                e[ku] = 0.0;
                break;
            }
            k -= 1;
        }
        let kase;
        if k == p as isize - 2 {
            kase = 4;
        } else {
            let mut ks: isize = p as isize - 1;
            while ks >= k {
                if ks == k {
                    break;
                }
                let ksu = ks as usize;
                let t = (if ks != p as isize { e[ksu].abs() } else { 0.0 })
                    + (if ks != k + 1 { e[ksu - 1].abs() } else { 0.0 });
                if s[ksu].abs() <= eps * t {
                    s[ksu] = 0.0;
                    break;
                }
                ks -= 1;
            }
            if ks == k {
                kase = 3;
            } else if ks == p as isize - 1 {
                kase = 1;
            } else {
                kase = 2;
                k = ks;
            }
        }
        k += 1;
        let k = k as usize;
        match kase {
            1 => {
                let mut f = e[p - 2];
                e[p - 2] = 0.0;
                let mut j = p as isize - 2;
                while j >= k as isize {
                    let ju = j as usize;
                    let mut t = libm::hypotf(s[ju], f);
                    let invt = 1.0 / t;
                    let cs = s[ju] * invt;
                    let sn = f * invt;
                    s[ju] = t;
                    if ju != k {
                        f = -sn * e[ju - 1];
                        e[ju - 1] *= cs;
                    }
                    for i in 0..n {
                        t = cs * v[i][ju] + sn * v[i][p - 1];
                        v[i][p - 1] = -sn * v[i][ju] + cs * v[i][p - 1];
                        v[i][ju] = t;
                    }
                    j -= 1;
                }
            }
            2 => {
                let mut f = e[k - 1];
                e[k - 1] = 0.0;
                for j in k..p {
                    let mut t = libm::hypotf(s[j], f);
                    let invt = 1.0 / t;
                    let cs = s[j] * invt;
                    let sn = f * invt;
                    s[j] = t;
                    f = -sn * e[j];
                    e[j] *= cs;
                    for i in 0..m {
                        t = cs * u[i][j] + sn * u[i][k - 1];
                        u[i][k - 1] = -sn * u[i][j] + cs * u[i][k - 1];
                        u[i][j] = t;
                    }
                }
            }
            3 => {
                let scale = s[p - 1]
                    .abs()
                    .max(s[p - 2].abs())
                    .max(e[p - 2].abs())
                    .max(s[k].abs())
                    .max(e[k].abs());
                let invscale = 1.0 / scale;
                let sp = s[p - 1] * invscale;
                let spm1 = s[p - 2] * invscale;
                let epm1 = e[p - 2] * invscale;
                let sk = s[k] * invscale;
                let ek = e[k] * invscale;
                let b = ((spm1 + sp) * (spm1 - sp) + epm1 * epm1) * 0.5;
                let c = (sp * epm1) * (sp * epm1);
                let mut shift = 0.0f32;
                if b != 0.0 || c != 0.0 {
                    shift = (b * b + c).sqrt();
                    if b < 0.0 {
                        shift = -shift;
                    }
                    shift = c / (b + shift);
                }
                let mut f = (sk + sp) * (sk - sp) + shift;
                let mut g = sk * ek;
                for j in k..(p - 1) {
                    let mut t = libm::hypotf(f, g);
                    let mut cs = if t == 0.0 { 0.0 } else { f / t };
                    let mut sn = if t == 0.0 { 0.0 } else { g / t };
                    if j != k {
                        e[j - 1] = t;
                    }
                    f = cs * s[j] + sn * e[j];
                    e[j] = cs * e[j] - sn * s[j];
                    g = sn * s[j + 1];
                    s[j + 1] *= cs;
                    for i in 0..n {
                        t = cs * v[i][j] + sn * v[i][j + 1];
                        v[i][j + 1] = -sn * v[i][j] + cs * v[i][j + 1];
                        v[i][j] = t;
                    }
                    t = libm::hypotf(f, g);
                    cs = if t == 0.0 { 0.0 } else { f / t };
                    sn = if t == 0.0 { 0.0 } else { g / t };
                    s[j] = t;
                    f = cs * e[j] + sn * s[j + 1];
                    s[j + 1] = -sn * e[j] + cs * s[j + 1];
                    g = sn * e[j + 1];
                    e[j + 1] *= cs;
                    if j < m - 1 {
                        for i in 0..m {
                            t = cs * u[i][j] + sn * u[i][j + 1];
                            u[i][j + 1] = -sn * u[i][j] + cs * u[i][j + 1];
                            u[i][j] = t;
                        }
                    }
                }
                e[p - 2] = f;
            }
            _ => {
                let mut k = k;
                if s[k] <= 0.0 {
                    s[k] = if s[k] < 0.0 { -s[k] } else { 0.0 };
                    for i in 0..=pp {
                        v[i][k] = -v[i][k];
                    }
                }
                while k < pp {
                    if s[k] >= s[k + 1] {
                        break;
                    }
                    s.swap(k, k + 1);
                    if k < n - 1 {
                        for i in 0..n {
                            v[i].swap(k + 1, k);
                        }
                    }
                    if k < m - 1 {
                        for i in 0..m {
                            u[i].swap(k + 1, k);
                        }
                    }
                    k += 1;
                }
                p -= 1;
            }
        }
    }
    (u, s, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudoinverse_of_a_flattened_matrix() {
        // Scale (2, 0, 1): the pseudo-inverse keeps (0.5, 0, 1).
        let m = Mat3::from_diagonal(Vec3::new(2.0, 0.0, 1.0));
        let p = pseudoinverse_m3(&m);
        assert!(
            p.abs_diff_eq(Mat3::from_diagonal(Vec3::new(0.5, 0.0, 1.0)), 1e-6),
            "{p:?}"
        );
        let r = Mat3::from_rotation_z(0.7) * m;
        let p = pseudoinverse_m3(&r);
        // A · A⁺ · A = A.
        assert!((r * p * r).abs_diff_eq(r, 1e-5), "{p:?}");
        let inv = pseudoinverse_m3(&Mat3::from_rotation_x(0.3));
        assert!(inv.abs_diff_eq(Mat3::from_rotation_x(-0.3), 1e-6));
    }

    #[test]
    fn geodesic_matches_edge_fallback() {
        // Without a valid virtual source the distance is the Dijkstra fallback.
        let d = geodesic_distance_propagate_across_triangle(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
            Vec3::X,
            0.0,
            1.0,
        );
        assert!((d - 1.0).abs() < 1e-6);
        // A source far along -Y gives a straight-line distance across the triangle.
        let d = geodesic_distance_propagate_across_triangle(
            Vec3::new(0.5, 1.0, 0.0),
            Vec3::ZERO,
            Vec3::X,
            (0.25f32 + 1.0).sqrt(),
            (0.25f32 + 1.0).sqrt(),
        );
        assert!((d - 2.0).abs() < 1e-5, "{d}");
    }

    #[test]
    fn space_normal() {
        let m = create_space_normal(Vec3::Z).expect("normal");
        assert!(m.z_axis.abs_diff_eq(Vec3::Z, 1e-6));
        // Z × Z is zero, so X = (1, 0, 0) × Z = -Y (`createSpaceNormal`).
        assert!(m.x_axis.abs_diff_eq(-Vec3::Y, 1e-6), "{m:?}");
        let m = create_space_normal(Vec3::new(0.0, 0.0, 2.0)).expect("normal");
        assert!((m.determinant() - 1.0).abs() < 1e-6);
        assert!(create_space_normal(Vec3::ZERO).is_none());
        let m = create_space_normal_tangent(Vec3::Z, Vec3::Y).expect("tangent");
        // Y starts as -tangent and is re-made orthogonal as Z × X: (+Y here).
        assert!(m.abs_diff_eq(Mat3::IDENTITY, 1e-6), "{m:?}");
        assert_eq!(axis_sort_v3([3.0, 1.0, 2.0]), [1, 2, 0]);
        let (n, ok) = create_from_axis(Some(Vec3::X * 2.0), None, Some(Vec3::Z));
        assert!(ok && n.abs_diff_eq(Mat3::IDENTITY, 1e-6), "{n:?}");
    }

    #[test]
    fn fallback_inverse() {
        let m = Mat4::from_scale(Vec3::new(2.0, 4.0, 1.0));
        assert!(invert_m4_fallback(&m).abs_diff_eq(m.inverse(), 1e-6));
    }
}
