// SPDX-License-Identifier: GPL-3.0-or-later
//! The BLI matrix/vector helpers and BMesh measures the WP 8.3.1 selection operators need, in
//! Blender's operation order (no `mul_add`, ARCHITECTURE §21.3). Matrices are Blender's
//! `float[4][4]` / `float[3][3]`: `m[i]` is column `i` (glam's `to_cols_array_2d`).
//!
//! Ported from blender/source/blender/blenlib/intern/math_matrix_c.cc (`mul_m4_v3`,
//! `mul_v3_m4v3`, `mul_v3_mat3_m4v3`, `mul_m3_v3`, `mul_transposed_m3_v3`,
//! `mul_transposed_mat3_m4_v3`, `copy_m3_m4`, `normalize_m3`), math_geom.cc
//! (`plane_from_point_normal_v3`) and blender/source/blender/bmesh/intern/bmesh_polygon.cc,
//! bmesh_query.cc (`BM_face_calc_area_with_mat3`, `BM_face_calc_perimeter_with_mat3`,
//! `BM_edge_calc_face_angle_with_imat3`), Blender 5.2.4, GPL-2.0-or-later.

use frap_bmesh::ops::contextual_create::bli;
use frap_bmesh::{BMesh, EdgeId, FaceId};
use glam::Mat4;

pub(crate) use bli::{V3, angle_normalized_v3v3, angle_v3v3, cross, dot, len_v3v3, normalize_v3};

/// `float[4][4]`.
pub(crate) type M4 = [[f32; 4]; 4];
/// `float[3][3]`.
pub(crate) type M3 = [[f32; 3]; 3];

/// A glam matrix as Blender's `float[4][4]`.
pub(crate) fn m4(m: &Mat4) -> M4 {
    m.to_cols_array_2d()
}

/// `invert_m4_m4` (the zero matrix when singular, as Blender leaves it after a failed
/// inversion only through `invert_m4_m4_safe`; the callers here never see singular matrices
/// except degenerate scales).
pub(crate) fn invert_m4(m: &M4) -> M4 {
    let g = Mat4::from_cols_array_2d(m);
    if g.determinant() == 0.0 {
        return [[0.0; 4]; 4];
    }
    g.inverse().to_cols_array_2d()
}

/// `copy_m3_m4`.
pub(crate) fn m3_of(m: &M4) -> M3 {
    [
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    ]
}

/// `invert_m3_m3` (zero when singular).
pub(crate) fn invert_m3(m: &M3) -> M3 {
    let g = glam::Mat3::from_cols_array_2d(m);
    if g.determinant() == 0.0 {
        return [[0.0; 3]; 3];
    }
    g.inverse().to_cols_array_2d()
}

/// `mul_v3_m4v3` / `mul_m4_v3`.
pub(crate) fn mul_m4_v3(m: &M4, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * v[2] + m[3][0],
        x * m[0][1] + y * m[1][1] + m[2][1] * v[2] + m[3][1],
        x * m[0][2] + y * m[1][2] + m[2][2] * v[2] + m[3][2],
    ]
}

/// `mul_v3_mat3_m4v3`: the 3×3 part of `m` times `v`.
pub(crate) fn mul_mat3_m4_v3(m: &M4, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * v[2],
        x * m[0][1] + y * m[1][1] + m[2][1] * v[2],
        x * m[0][2] + y * m[1][2] + m[2][2] * v[2],
    ]
}

/// `mul_v3_m3v3` / `mul_m3_v3`.
pub(crate) fn mul_m3_v3(m: &M3, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        m[0][0] * x + m[1][0] * y + m[2][0] * v[2],
        m[0][1] * x + m[1][1] * y + m[2][1] * v[2],
        m[0][2] * x + m[1][2] * y + m[2][2] * v[2],
    ]
}

/// `mul_transposed_m3_v3`.
pub(crate) fn mul_transposed_m3_v3(m: &M3, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        m[0][0] * x + m[0][1] * y + m[0][2] * v[2],
        m[1][0] * x + m[1][1] * y + m[1][2] * v[2],
        m[2][0] * x + m[2][1] * y + m[2][2] * v[2],
    ]
}

/// `mul_transposed_mat3_m4_v3`.
pub(crate) fn mul_transposed_mat3_m4_v3(m: &M4, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        x * m[0][0] + y * m[0][1] + m[0][2] * v[2],
        x * m[1][0] + y * m[1][1] + m[1][2] * v[2],
        x * m[2][0] + y * m[2][1] + m[2][2] * v[2],
    ]
}

/// `normalize_m3`: every column normalised.
pub(crate) fn normalize_m3(m: &M3) -> M3 {
    [
        normalize_v3(m[0]).0,
        normalize_v3(m[1]).0,
        normalize_v3(m[2]).0,
    ]
}

/// `mul_m3_m3m3(r, a, b)`: `a * b`.
pub(crate) fn mul_m3_m3m3(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0f32; 3]; 3];
    for (j, col) in r.iter_mut().enumerate() {
        for (i, x) in col.iter_mut().enumerate() {
            *x = b[j][0] * a[0][i] + b[j][1] * a[1][i] + b[j][2] * a[2][i];
        }
    }
    r
}

/// `plane_from_point_normal_v3`.
pub(crate) fn plane_from_point_normal(co: V3, no: V3) -> [f32; 4] {
    [no[0], no[1], no[2], -dot(no, co)]
}

/// The world-space normal of an object-space normal (`mul_transposed_mat3_m4_v3` by
/// `world_to_object`, then `normalize_v3`).
pub(crate) fn normal_world(world_to_object: &M4, no: V3) -> V3 {
    normalize_v3(mul_transposed_mat3_m4_v3(world_to_object, no)).0
}

/// `BM_face_calc_area_with_mat3`.
pub(crate) fn face_area_with_mat3(bm: &BMesh, f: FaceId, m: &M3) -> f32 {
    let loops = bm.face_loops(f);
    let mut n = [0.0f32; 3];
    let mut co = mul_m3_v3(m, bm.vert(bm.lp(loops[0]).v).co);
    for &l in &loops {
        let next = mul_m3_v3(m, bm.vert(bm.lp(bm.lp(l).next).v).co);
        bli::add_newell_cross_v3_v3v3(&mut n, co, next);
        co = next;
    }
    bli::len_v3(n) * 0.5
}

/// `BM_face_calc_perimeter_with_mat3`.
pub(crate) fn face_perimeter_with_mat3(bm: &BMesh, f: FaceId, m: &M3) -> f32 {
    let loops = bm.face_loops(f);
    let mut perimeter = 0.0f32;
    let mut co = mul_m3_v3(m, bm.vert(bm.lp(loops[0]).v).co);
    for &l in &loops {
        let next = mul_m3_v3(m, bm.vert(bm.lp(bm.lp(l).next).v).co);
        perimeter += len_v3v3(co, next);
        co = next;
    }
    perimeter
}

/// `BM_face_calc_area`.
pub(crate) fn face_area(bm: &BMesh, f: FaceId) -> f32 {
    frap_bmesh::ops::contextual_create::construct::face_calc_area(bm, f)
}

/// `BM_edge_calc_face_angle_with_imat3`: the angle between the two faces' normals mapped by
/// the transpose of `imat3`, 90° for a non-manifold edge.
pub(crate) fn edge_face_angle_with_imat3(bm: &BMesh, e: EdgeId, imat3: &M3) -> f32 {
    if !bm.edge_is_manifold(e) {
        return 90.0f32.to_radians();
    }
    let l1 = bm.edge(e).l.expect("manifold edge has loops");
    let l2 = bm.lp(l1).radial_next;
    let no1 = normalize_v3(mul_transposed_m3_v3(imat3, bm.face(bm.lp(l1).f).no)).0;
    let no2 = normalize_v3(mul_transposed_m3_v3(imat3, bm.face(bm.lp(l2).f).no)).0;
    angle_normalized_v3v3(no1, no2)
}

/// `BM_edge_calc_length`.
pub(crate) fn edge_length(bm: &BMesh, e: EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    len_v3v3(bm.vert(a).co, bm.vert(b).co)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_helpers_follow_blender_layout() {
        // Translation lives in column 3.
        let m = m4(&Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(mul_m4_v3(&m, [1.0, 1.0, 1.0]), [2.0, 3.0, 4.0]);
        assert_eq!(mul_mat3_m4_v3(&m, [1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]);
        let r = m3_of(&m4(&Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2)));
        let x = mul_m3_v3(&r, [1.0, 0.0, 0.0]);
        assert!((x[1] - 1.0).abs() < 1e-6 && x[0].abs() < 1e-6);
        let back = mul_transposed_m3_v3(&r, x);
        assert!((back[0] - 1.0).abs() < 1e-6);
        let id = mul_m3_m3m3(&r, &invert_m3(&r));
        assert!((id[0][0] - 1.0).abs() < 1e-6 && id[1][0].abs() < 1e-6);
        assert_eq!(plane_from_point_normal([0.0, 0.0, 2.0], [0.0, 0.0, 1.0])[3], -2.0);
        assert_eq!(normalize_m3(&[[2.0, 0.0, 0.0]; 3])[1], [1.0, 0.0, 0.0]);
    }
}
