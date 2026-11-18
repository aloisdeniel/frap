// SPDX-License-Identifier: GPL-3.0-or-later
//! Float helpers shared by the WP 2.10 bmesh ops (triangulate, join_triangles, poke, spin,
//! solidify, wireframe, symmetrize): exact ports of the `BLI_math_*` and `bmesh_query.cc`
//! functions they call, written in Blender's operation order without `mul_add`
//! (ARCHITECTURE §21.3). Transcendentals come from `libm`.
//!
//! Ported from blender/source/blender/blenlib/intern/{math_vector.cc,math_geom.cc,
//! math_geom_inline.cc,math_rotation_c.cc,math_base_safe_inline.cc} and
//! blender/source/blender/bmesh/intern/{bmesh_query.cc,bmesh_polygon.cc} (Blender 5.2.4).

use crate::query::geom::{self, V3};
use crate::{BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};

pub(crate) use geom::{add, cross, dot, normalize_v3, scale, sub};

/// `SMALL_NUMBER` of `BLI_math_geom.h`.
pub(crate) const SMALL_NUMBER: f32 = 1.0e-8;

/// `len_v3`.
#[inline]
pub(crate) fn len_v3(a: V3) -> f32 {
    dot(a, a).sqrt()
}

/// `len_v3v3(a, b)` (`sub_v3_v3v3(d, a, b)` then `len_v3`).
#[inline]
pub(crate) fn len_v3v3(a: V3, b: V3) -> f32 {
    len_v3(sub(a, b))
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

/// `interp_v3_v3v3(r, a, b, t)`: `s * a + t * b` with `s = 1 - t`.
#[inline]
pub(crate) fn interp(a: V3, b: V3, t: f32) -> V3 {
    let s = 1.0 - t;
    [
        s * a[0] + t * b[0],
        s * a[1] + t * b[1],
        s * a[2] + t * b[2],
    ]
}

/// `mid_v3_v3v3`: `(a + b) * 0.5`.
#[inline]
pub(crate) fn mid(a: V3, b: V3) -> V3 {
    [
        (a[0] + b[0]) * 0.5,
        (a[1] + b[1]) * 0.5,
        (a[2] + b[2]) * 0.5,
    ]
}

/// `compare_v3v3(a, b, limit)`: every component within `limit`.
#[inline]
pub(crate) fn compare_v3v3(a: V3, b: V3, limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit && (a[2] - b[2]).abs() <= limit
}

/// `is_zero_v3`.
#[inline]
pub(crate) fn is_zero_v3(a: V3) -> bool {
    a[0] == 0.0 && a[1] == 0.0 && a[2] == 0.0
}

/// `safe_asinf`.
#[inline]
pub(crate) fn safe_asinf(a: f32) -> f32 {
    libm::asinf(a.clamp(-1.0, 1.0))
}

/// `angle_normalized_v3v3`.
pub(crate) fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    std::f32::consts::PI - 2.0 * safe_asinf(len_v3v3(v1, negate(v2)) / 2.0)
}

/// `angle_v3v3`.
pub(crate) fn angle_v3v3(a: V3, b: V3) -> f32 {
    angle_normalized_v3v3(normalize_v3(a).0, normalize_v3(b).0)
}

/// `angle_v3v3v3(a, b, c)`: the angle at `b`.
pub(crate) fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    angle_normalized_v3v3(normalize_v3(sub(b, a)).0, normalize_v3(sub(b, c)).0)
}

/// `project_plane_normalized_v3_v3v3`.
#[inline]
pub(crate) fn project_plane_normalized(p: V3, v_plane: V3) -> V3 {
    let mul = dot(p, v_plane);
    madd(p, v_plane, -mul)
}

/// `angle_on_axis_v3v3v3_v3`.
pub(crate) fn angle_on_axis_v3v3v3_v3(v1: V3, v2: V3, v3: V3, axis: V3) -> f32 {
    let vec1 = sub(v1, v2);
    let vec2 = sub(v3, v2);
    angle_v3v3(
        project_plane_normalized(vec1, axis),
        project_plane_normalized(vec2, axis),
    )
}

/// `angle_signed_on_axis_v3v3_v3`.
pub(crate) fn angle_signed_on_axis_v3v3_v3(v1: V3, v2: V3, axis: V3) -> f32 {
    let v1_proj = project_plane_normalized(v1, axis);
    let v2_proj = project_plane_normalized(v2, axis);
    let mut angle = angle_v3v3(v1_proj, v2_proj);
    let tproj = cross(v2_proj, v1_proj);
    if dot(tproj, axis) < 0.0 {
        angle = (std::f64::consts::PI * 2.0) as f32 - angle;
    }
    angle
}

/// `rotate_normalized_v3_v3v3fl`.
pub(crate) fn rotate_normalized_v3_v3v3fl(p: V3, axis: V3, angle: f32) -> V3 {
    let c = libm::cosf(angle);
    let s = libm::sinf(angle);
    [
        ((c + (1.0 - c) * axis[0] * axis[0]) * p[0])
            + (((1.0 - c) * axis[0] * axis[1] - axis[2] * s) * p[1])
            + (((1.0 - c) * axis[0] * axis[2] + axis[1] * s) * p[2]),
        (((1.0 - c) * axis[0] * axis[1] + axis[2] * s) * p[0])
            + ((c + (1.0 - c) * axis[1] * axis[1]) * p[1])
            + (((1.0 - c) * axis[1] * axis[2] - axis[0] * s) * p[2]),
        (((1.0 - c) * axis[0] * axis[2] - axis[1] * s) * p[0])
            + (((1.0 - c) * axis[1] * axis[2] + axis[0] * s) * p[1])
            + ((c + (1.0 - c) * axis[2] * axis[2]) * p[2]),
    ]
}

/// `cross_tri_v3`.
#[inline]
pub(crate) fn cross_tri_v3(v1: V3, v2: V3, v3: V3) -> V3 {
    let n1 = sub(v1, v2);
    let n2 = sub(v2, v3);
    [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ]
}

/// `area_tri_v3`.
#[inline]
pub(crate) fn area_tri_v3(v1: V3, v2: V3, v3: V3) -> f32 {
    len_v3(cross_tri_v3(v1, v2, v3)) * 0.5
}

/// `normal_tri_v3`: the unit normal (and the length before normalising).
#[inline]
pub(crate) fn normal_tri_v3(v1: V3, v2: V3, v3: V3) -> (V3, f32) {
    geom::normal_tri_v3(v1, v2, v3)
}

/// `normal_quad_v3`.
#[inline]
pub(crate) fn normal_quad_v3(v1: V3, v2: V3, v3: V3, v4: V3) -> (V3, f32) {
    geom::normal_quad_v3(v1, v2, v3, v4)
}

/// `cross_tri_v2`.
#[inline]
pub(crate) fn cross_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0])
}

/// `len_v2v2`.
#[inline]
pub(crate) fn len_v2v2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1]];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

/// `shell_angle_to_dist`.
#[inline]
pub(crate) fn shell_angle_to_dist(angle: f32) -> f32 {
    if angle < SMALL_NUMBER {
        1.0
    } else {
        (1.0 / libm::cosf(angle)).abs()
    }
}

/// `shell_v3v3_normalized_to_dist`.
#[inline]
pub(crate) fn shell_v3v3_normalized_to_dist(a: V3, b: V3) -> f32 {
    let angle_cos = dot(a, b).abs();
    if angle_cos < SMALL_NUMBER {
        1.0
    } else {
        1.0 / angle_cos
    }
}

/// `angle_poly_v3`: the interior angle at every corner of a polygon.
pub(crate) fn angle_poly_v3(verts: &[V3]) -> Vec<f32> {
    let len = verts.len();
    let mut angles = vec![0.0f32; len];
    let mut vec = [[0.0f32; 3]; 3];
    vec[2] = normalize_v3(sub(verts[len - 1], verts[0])).0;
    for i in 0..len {
        vec[i % 3] = normalize_v3(sub(verts[i % len], verts[(i + 1) % len])).0;
        angles[i] = std::f32::consts::PI - angle_normalized_v3v3(vec[(i + 2) % 3], vec[i % 3]);
    }
    angles
}

/// `is_quad_flip_v3`: bit 0 when the 1–3 split folds, bit 1 when the 2–4 split folds.
pub(crate) fn is_quad_flip_v3(v1: V3, v2: V3, v3: V3, v4: V3) -> i32 {
    let d_12 = sub(v1, v2);
    let d_23 = sub(v2, v3);
    let d_34 = sub(v3, v4);
    let d_41 = sub(v4, v1);
    let mut ret = 0;
    if dot(cross(d_12, d_23), cross(d_34, d_41)) < 0.0 {
        ret |= 1;
    }
    if dot(cross(d_23, d_34), cross(d_41, d_12)) < 0.0 {
        ret |= 2;
    }
    ret
}

/// `axis_angle_normalized_to_mat3` (`m[col][row]`).
pub(crate) fn axis_angle_normalized_to_mat3(axis: V3, angle: f32) -> [[f32; 3]; 3] {
    let angle_sin = libm::sinf(angle);
    let angle_cos = libm::cosf(angle);
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

/// `mul_v3_m3v3(r, m, a)` with `m[col][row]`.
#[inline]
pub(crate) fn mul_m3_v3(m: &[[f32; 3]; 3], a: V3) -> V3 {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
        m[0][2] * a[0] + m[1][2] * a[1] + m[2][2] * a[2],
    ]
}

/// A 4×4 matrix in Blender's layout (`m[col][row]`).
pub(crate) type M4 = [[f32; 4]; 4];

/// `unit_m4`.
pub(crate) const UNIT_M4: M4 = crate::IDENTITY;

/// `copy_m4_m3`.
pub(crate) fn m4_from_m3(m: &[[f32; 3]; 3]) -> M4 {
    [
        [m[0][0], m[0][1], m[0][2], 0.0],
        [m[1][0], m[1][1], m[1][2], 0.0],
        [m[2][0], m[2][1], m[2][2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

/// `mul_m4_m4m4(r, a, b)`: `r = a · b`.
pub(crate) fn mul_m4_m4m4(a: &M4, b: &M4) -> M4 {
    let mut r = [[0.0f32; 4]; 4];
    for (j, rj) in r.iter_mut().enumerate() {
        for (i, rji) in rj.iter_mut().enumerate() {
            *rji = b[j][0] * a[0][i] + b[j][1] * a[1][i] + b[j][2] * a[2][i] + b[j][3] * a[3][i];
        }
    }
    r
}

/// `mul_m4_v3`: `m · (a, 1)` (the translation is added last).
#[inline]
pub(crate) fn mul_m4_v3(m: &M4, a: V3) -> V3 {
    [
        a[0] * m[0][0] + a[1] * m[1][0] + m[2][0] * a[2] + m[3][0],
        a[0] * m[0][1] + a[1] * m[1][1] + m[2][1] * a[2] + m[3][1],
        a[0] * m[0][2] + a[1] * m[1][2] + m[2][2] * a[2] + m[3][2],
    ]
}

/// `invert_m4_m4` (Gauss-Jordan with partial pivoting, `invert_m4_m4_fallback`); `None` for
/// a singular matrix.
pub(crate) fn invert_m4(m: &M4) -> Option<M4> {
    let mut tempmat = *m;
    let mut inverse = UNIT_M4;
    for i in 0..4 {
        // Look for the row with the max pivot.
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
        if tempmat[i][i] == 0.0 {
            return None;
        }
        let temp = tempmat[i][i];
        for k in 0..4 {
            tempmat[i][k] /= temp;
            inverse[i][k] /= temp;
        }
        for j in 0..4 {
            if j != i {
                let temp = tempmat[j][i];
                for k in 0..4 {
                    tempmat[j][k] -= tempmat[i][k] * temp;
                    inverse[j][k] -= inverse[i][k] * temp;
                }
            }
        }
    }
    Some(inverse)
}

/// `is_zero_m4`.
pub(crate) fn is_zero_m4(m: &M4) -> bool {
    m.iter().all(|c| c.iter().all(|&x| x == 0.0))
}

/// `transform_pivot_set_m4(mat, pivot)`: `T(pivot) · mat · T(-pivot)`.
pub(crate) fn transform_pivot_set_m4(mat: &M4, pivot: V3) -> M4 {
    let mut tmat = UNIT_M4;
    tmat[3][0] = pivot[0];
    tmat[3][1] = pivot[1];
    tmat[3][2] = pivot[2];
    let r = mul_m4_m4m4(&tmat, mat);
    tmat[3][0] = -pivot[0];
    tmat[3][1] = -pivot[1];
    tmat[3][2] = -pivot[2];
    mul_m4_m4m4(&r, &tmat)
}

// Ported from blender/source/blender/bmesh/operators/bmo_utils.cc (bmo_transform_exec).
/// `bmesh.ops.transform(verts, matrix, space, use_shapekey)`: `matrix` conjugated by `space`
/// (`space⁻¹ · matrix · space`) when `space` is not zero, applied to every vertex (and its
/// shape-key coordinates with `use_shapekey`).
pub(crate) fn bmo_transform(
    bm: &mut BMesh,
    verts: &[VertId],
    matrix: &M4,
    space: &M4,
    use_shapekey: bool,
) {
    let mut mat = *matrix;
    if !is_zero_m4(space) {
        let imat_space = invert_m4(space).unwrap_or(UNIT_M4);
        // mul_m4_series(mat, imat_space, mat, mat_space).
        mat = mul_m4_m4m4(&mul_m4_m4m4(&imat_space, &mat), space);
    }
    for &v in verts {
        if !bm.verts.contains(v.0) {
            continue;
        }
        let co = bm.vert(v).co;
        bm.vert_mut(v).co = mul_m4_v3(&mat, co);
        if use_shapekey {
            for s in &mut bm.shape_layers {
                if let Some(c) = s.co.get_mut(v.slot()) {
                    *c = mul_m4_v3(&mat, *c);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- BMesh queries

/// `BM_edge_calc_length`.
#[inline]
pub(crate) fn edge_calc_length(bm: &BMesh, e: EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    len_v3v3(bm.vert(a).co, bm.vert(b).co)
}

/// `BM_loop_calc_face_angle`.
pub(crate) fn loop_calc_face_angle(bm: &BMesh, l: LoopId) -> f32 {
    let ll = bm.lp(l);
    angle_v3v3v3(
        bm.vert(bm.lp(ll.prev).v).co,
        bm.vert(ll.v).co,
        bm.vert(bm.lp(ll.next).v).co,
    )
}

/// `BM_loop_calc_face_tangent`.
pub(crate) fn loop_calc_face_tangent(bm: &BMesh, l: LoopId) -> V3 {
    let ll = bm.lp(l);
    let co = bm.vert(ll.v).co;
    let v_prev = normalize_v3(sub(bm.vert(bm.lp(ll.prev).v).co, co)).0;
    let v_next = normalize_v3(sub(co, bm.vert(bm.lp(ll.next).v).co)).0;
    let dir = add(v_prev, v_next);
    let fno = bm.face(ll.f).no;
    let t = if !compare_v3v3(v_prev, v_next, f32::EPSILON * 10.0) {
        let mut nor = cross(v_prev, v_next);
        if dot(nor, fno) < 0.0 {
            nor = negate(nor);
        }
        cross(dir, nor)
    } else {
        cross(dir, fno)
    };
    normalize_v3(t).0
}

/// `BM_edge_calc_face_tangent(e, e_loop)`.
pub(crate) fn edge_calc_face_tangent(bm: &BMesh, e_loop: LoopId) -> V3 {
    let ll = bm.lp(e_loop);
    let v1 = bm.vert(ll.v).co;
    let v2 = bm.vert(bm.lp(ll.next).v).co;
    normalize_v3(cross(sub(v1, v2), bm.face(ll.f).no)).0
}

/// `BM_vert_calc_median_tagged_edge_length`.
pub(crate) fn vert_calc_median_tagged_edge_length(bm: &BMesh, v: VertId) -> f32 {
    let mut length = 0.0f32;
    let edges = bm.vert_edges(v);
    for &e in &edges {
        let other = bm.edge_other_vert(e, v);
        if bm.vert(other).flag.contains(ElemFlag::TAG) {
            length += edge_calc_length(bm, e);
        }
    }
    if edges.is_empty() {
        0.0
    } else {
        length / edges.len() as f32
    }
}

/// `BM_face_calc_center_median_weighted`.
pub(crate) fn face_calc_center_median_weighted(bm: &BMesh, f: FaceId) -> V3 {
    let loops = bm.face_loops(f);
    let mut r = [0.0f32; 3];
    let mut totw = 0.0f32;
    let mut w_prev = edge_calc_length(bm, bm.lp(bm.lp(loops[0]).prev).e);
    for &l in &loops {
        let w_curr = edge_calc_length(bm, bm.lp(l).e);
        let w = w_curr + w_prev;
        geom::madd_v3_v3fl(&mut r, bm.vert(bm.lp(l).v).co, w);
        totw += w;
        w_prev = w_curr;
    }
    if totw != 0.0 {
        r = scale(r, 1.0 / totw);
    }
    r
}

/// `BM_face_calc_center_bounds`.
pub(crate) fn face_calc_center_bounds(bm: &BMesh, f: FaceId) -> V3 {
    let mut min = [f32::MAX; 3];
    let mut max = [-f32::MAX; 3];
    for v in bm.face_verts(f) {
        let co = bm.vert(v).co;
        for i in 0..3 {
            if co[i] < min[i] {
                min[i] = co[i];
            }
            if co[i] > max[i] {
                max[i] = co[i];
            }
        }
    }
    mid(min, max)
}

/// `BM_face_calc_center_median`.
pub(crate) fn face_calc_center_median(bm: &BMesh, f: FaceId) -> V3 {
    let mut r = [0.0f32; 3];
    let verts = bm.face_verts(f);
    for &v in &verts {
        r = add(r, bm.vert(v).co);
    }
    scale(r, 1.0 / verts.len() as f32)
}

/// `bmesh_face_swap_data(f_a, f_b)`: swaps everything but the custom data and the index:
/// the loop cycles (and their `f` back-pointers), `l_first`, `len`, the normal, the material
/// and the header flags.
pub(crate) fn face_swap_data(bm: &mut BMesh, f_a: FaceId, f_b: FaceId) {
    for l in bm.face_loops(f_a) {
        bm.lp_mut(l).f = f_b;
    }
    for l in bm.face_loops(f_b) {
        bm.lp_mut(l).f = f_a;
    }
    let a = bm.face(f_a).clone();
    let b = bm.face(f_b).clone();
    *bm.face_mut(f_a) = b;
    *bm.face_mut(f_b) = a;
}
