// SPDX-License-Identifier: GPL-3.0-or-later
//! World matrices (§7.3): `matrix_basis` from the transform channels, the parent matrix of every
//! M1 parent type and `matrix_world = parent · matrix_parent_inverse · matrix_basis`.
//!
//! Provenance: Blender 5.2.4 (GPL-2.0-or-later), `blenkernel/intern/object.cc`
//! (`BKE_object_where_is_calc_ex`, `solve_parenting`, `BKE_object_get_parent_matrix`,
//! `give_parvert`, `ob_parvert3`), `blenlib/intern/math_matrix_c.cc` (`mul_m4_m4m4` in its
//! SSE2 form, which Apple Silicon builds reach through sse2neon; `mul_v3_m4v3`, `mul_m3_v3`,
//! `invert_m3_m3`, `adjoint_m3_m3`), `blenlib/intern/math_rotation.cc` (`tri_to_quat_ex`) and
//! `blenlib/intern/math_geom.cc` (`normal_tri_v3`). Operation order is kept so results match
//! Blender bit for bit (checked against `tests/data/eval_cases.json`); transcendentals come
//! from `libm` (rule 2.2.12), so frap's result is the same on every OS.

use frap_math::euler::EulerOrder;
use frap_math::matrix::{ObjectTransform, RotationMode, RotationValues, normalize_v3};
use frap_math::quat::{mul_qt_qtqt, quat_to_mat3};
use frap_mesh::Mesh;
use frap_scene::id::Object;
use glam::Mat4;

type M4 = [[f32; 4]; 4];
type M3 = [[f32; 3]; 3];

/// The transform channels of an object as a [`ObjectTransform`] (an unknown `rotation_mode`
/// string reads as `'XYZ'`, Blender's default).
pub fn object_transform(ob: &Object) -> ObjectTransform {
    ObjectTransform {
        location: ob.location,
        rotation_mode: RotationMode::from_blender(&ob.rotation_mode)
            .unwrap_or(RotationMode::Euler(EulerOrder::XYZ)),
        rotation: RotationValues {
            quaternion: ob.rotation_quaternion,
            euler: ob.rotation_euler,
            axis_angle: ob.rotation_axis_angle,
        },
        scale: ob.scale,
        delta_location: ob.delta_location,
        delta_rotation_euler: ob.delta_rotation_euler,
        delta_rotation_quaternion: ob.delta_rotation_quaternion,
        delta_scale: ob.delta_scale,
    }
}

/// `Object.matrix_basis` (`BKE_object_to_mat4`): location, rotation and scale with their delta
/// channels.
pub fn matrix_basis(ob: &Object) -> Mat4 {
    object_transform(ob).to_mat4()
}

/// `mul_m4_m4m4(R, A, B)`: `R = A · B`, summed pairwise like Blender's SSE2 code path:
/// `(B[i][0]·A[0] + B[i][1]·A[1]) + (B[i][2]·A[2] + B[i][3]·A[3])`.
pub(crate) fn mul_m4_m4m4(a: &Mat4, b: &Mat4) -> Mat4 {
    let a: M4 = a.to_cols_array_2d();
    let b: M4 = b.to_cols_array_2d();
    let mut r: M4 = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] =
                (b[i][0] * a[0][j] + b[i][1] * a[1][j]) + (b[i][2] * a[2][j] + b[i][3] * a[3][j]);
        }
    }
    Mat4::from_cols_array_2d(&r)
}

/// `mul_v3_m4v3`: `M · (v, 1)`.
fn mul_v3_m4v3(m: &Mat4, v: [f32; 3]) -> [f32; 3] {
    let m: M4 = m.to_cols_array_2d();
    let [x, y, z] = v;
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * z + m[3][0],
        x * m[0][1] + y * m[1][1] + m[2][1] * z + m[3][1],
        x * m[0][2] + y * m[1][2] + m[2][2] * z + m[3][2],
    ]
}

/// `mul_m3_v3` (`mul_v3_m3v3` in place).
fn mul_m3_v3(m: &M3, t: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * t[0] + m[1][0] * t[1] + m[2][0] * t[2],
        m[0][1] * t[0] + m[1][1] * t[1] + m[2][1] * t[2],
        m[0][2] * t[0] + m[1][2] * t[1] + m[2][2] * t[2],
    ]
}

/// `invert_m3_m3`: adjoint divided by the determinant (left as the adjoint when singular).
fn invert_m3(m: &M3) -> M3 {
    let mut r: M3 = [
        [
            m[1][1] * m[2][2] - m[1][2] * m[2][1],
            -m[0][1] * m[2][2] + m[0][2] * m[2][1],
            m[0][1] * m[1][2] - m[0][2] * m[1][1],
        ],
        [
            -m[1][0] * m[2][2] + m[1][2] * m[2][0],
            m[0][0] * m[2][2] - m[0][2] * m[2][0],
            -m[0][0] * m[1][2] + m[0][2] * m[1][0],
        ],
        [
            m[1][0] * m[2][1] - m[1][1] * m[2][0],
            -m[0][0] * m[2][1] + m[0][1] * m[2][0],
            m[0][0] * m[1][1] - m[0][1] * m[1][0],
        ],
    ];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[1][0] * (m[0][1] * m[2][2] - m[0][2] * m[2][1])
        + m[2][0] * (m[0][1] * m[1][2] - m[0][2] * m[1][1]);
    if det != 0.0 {
        let inv = 1.0 / det;
        for col in &mut r {
            for v in col {
                *v *= inv;
            }
        }
    }
    r
}

/// `normal_tri_v3`: the unit normal of a triangle (zero when degenerate).
fn normal_tri_v3(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> [f32; 3] {
    let n1 = [v1[0] - v2[0], v1[1] - v2[1], v1[2] - v2[2]];
    let n2 = [v2[0] - v3[0], v2[1] - v3[1], v2[2] - v3[2]];
    let n = [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ];
    normalize_v3(n).0
}

/// `tri_to_quat`: the rotation taking the XY plane to the triangle's plane and the X axis to
/// the direction `v1 → v2`.
fn tri_to_quat(v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) -> [f32; 4] {
    let no = normal_tri_v3(v1, v2, v3);
    // Move the Z axis to the face normal.
    let (mut n, _) = normalize_v3([no[1], -no[0], 0.0]);
    if n[0] == 0.0 && n[1] == 0.0 {
        n[0] = 1.0;
    }
    let angle = -0.5 * libm::acosf(no[2].clamp(-1.0, 1.0));
    let (si, co) = (libm::sinf(angle), libm::cosf(angle));
    let q1 = [co, n[0] * si, n[1] * si, 0.0];
    // Rotate the line v1-v2 back.
    let mat: M3 = quat_to_mat3(q1).to_cols_array_2d();
    let imat = invert_m3(&mat);
    let vec = mul_m3_v3(&imat, [v2[0] - v1[0], v2[1] - v1[1], v2[2] - v1[2]]);
    // The angle of this line with the X axis.
    let (vec, _) = normalize_v3([vec[0], vec[1], 0.0]);
    let angle = 0.5 * libm::atan2f(vec[1], vec[0]);
    let (si, co) = (libm::sinf(angle), libm::cosf(angle));
    let q2 = [co, 0.0, 0.0, si];
    mul_qt_qtqt(q1, q2)
}

/// `give_parvert`: vertex `nr` of the parent's evaluated mesh, or the origin when the index is
/// out of range or the parent has no mesh (`OB_TYPE_SUPPORT_PARVERT`: frap has only meshes).
fn give_parvert(mesh: Option<&Mesh>, nr: i32) -> [f32; 3] {
    let Some(mesh) = mesh else {
        return [0.0; 3];
    };
    let positions = mesh.positions();
    match usize::try_from(nr).ok().and_then(|i| positions.get(i)) {
        // `add_v3_v3` onto zero, then `mul_v3_fl(vec, 1.0f / count)` with count 1: exact.
        Some(p) => [0.0 + p[0], 0.0 + p[1], 0.0 + p[2]],
        None => [0.0; 3],
    }
}

/// `ob_parvert3`: the frame of the parent triangle `parent_vertices` (rotation from
/// `tri_to_quat`, origin at the triangle's centre); identity for a parent without a mesh.
fn parvert3(ob: &Object, mesh: Option<&Mesh>) -> Mat4 {
    if mesh.is_none() {
        return Mat4::IDENTITY;
    }
    let [a, b, c] = ob.parent_vertices;
    let (v1, v2, v3) = (
        give_parvert(mesh, a),
        give_parvert(mesh, b),
        give_parvert(mesh, c),
    );
    let q = tri_to_quat(v1, v2, v3);
    let m: M3 = quat_to_mat3(q).to_cols_array_2d();
    let mid = [
        (v1[0] + v2[0] + v3[0]) / 3.0,
        (v1[1] + v2[1] + v3[1]) / 3.0,
        (v1[2] + v2[2] + v3[2]) / 3.0,
    ];
    Mat4::from_cols_array_2d(&[
        [m[0][0], m[0][1], m[0][2], 0.0],
        [m[1][0], m[1][1], m[1][2], 0.0],
        [m[2][0], m[2][1], m[2][2], 0.0],
        [mid[0], mid[1], mid[2], 1.0],
    ])
}

/// `BKE_object_get_parent_matrix`: the matrix the child's `matrix_parent_inverse · basis` is
/// multiplied by. `parent_world` is the parent's `matrix_world`, `parent_mesh` its evaluated
/// mesh (vertex parents).
///
/// `OBJECT`, `ARMATURE` and `LATTICE` (`PARSKEL`) use the parent's world matrix. `VERTEX` uses
/// only the world position of the vertex (no parent rotation, as in Blender), `VERTEX_3` the
/// parent's world matrix times the triangle frame. `BONE` needs poses (M6) and uses the
/// parent's world matrix, which is Blender's result for a bone that does not exist.
pub fn parent_matrix(ob: &Object, parent_world: &Mat4, parent_mesh: Option<&Mesh>) -> Mat4 {
    match ob.parent_type.as_str() {
        "VERTEX" => {
            let vec = give_parvert(parent_mesh, ob.parent_vertices[0]);
            let t = mul_v3_m4v3(parent_world, vec);
            Mat4::from_cols_array_2d(&[
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [t[0], t[1], t[2], 1.0],
            ])
        }
        "VERTEX_3" => mul_m4_m4m4(parent_world, &parvert3(ob, parent_mesh)),
        _ => *parent_world,
    }
}

/// `matrix_world` (`BKE_object_where_is_calc`): `matrix_basis` without a parent, otherwise
/// `solve_parenting`'s `parent · matrix_parent_inverse · matrix_basis`.
pub fn world_matrix(ob: &Object, parent: Option<&Mat4>) -> Mat4 {
    let basis = matrix_basis(ob);
    match parent {
        None => basis,
        Some(p) => mul_m4_m4m4(&mul_m4_m4m4(p, &ob.matrix_parent_inverse), &basis),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_matches_glam_on_exact_values() {
        let a = Mat4::from_cols_array(&[
            1.0, 2.0, 0.0, 0.0, 0.0, 1.0, 3.0, 0.0, 4.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0,
        ]);
        let b = Mat4::from_cols_array(&[
            2.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, -1.0, 4.0, 2.0, 1.0,
        ]);
        assert_eq!(mul_m4_m4m4(&a, &b), a * b);
    }

    #[test]
    fn invert_m3_inverts() {
        let m: M3 = [[2.0, 0.0, 1.0], [0.0, 4.0, 0.0], [0.0, 0.0, 0.5]];
        let inv = invert_m3(&m);
        let p = glam::Mat3::from_cols_array_2d(&m) * glam::Mat3::from_cols_array_2d(&inv);
        assert!(p.abs_diff_eq(glam::Mat3::IDENTITY, 1e-6));
    }

    #[test]
    fn tri_to_quat_of_xy_triangle_is_identity() {
        let q = tri_to_quat([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((q[0] - 1.0).abs() < 1e-7 && q[1..].iter().all(|v| v.abs() < 1e-7));
    }

    #[test]
    fn vertex_parent_ignores_parent_rotation() {
        let mut ob = Object::new("C", None);
        ob.parent_type = "VERTEX".into();
        let pw =
            Mat4::from_rotation_z(1.0) * Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
        let m = parent_matrix(&ob, &pw, None);
        assert_eq!(m.x_axis, glam::Vec4::X);
        assert_eq!(m.w_axis.truncate(), pw.w_axis.truncate());
    }
}
