// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_axis(orientation='LOCAL', sign='POS', axis='X', threshold=0.0001)`,
//! tier T1 (`tests/golden/cases/edit/select_m8/axis.py`). Port of `edbm_select_axis_exec`
//! (`editors/mesh/editmesh_select.cc`) with the orientation of
//! `calc_orientation_from_type_ex` (pivot `ACTIVE_ELEMENT`), `gimbal_axis_object`,
//! `axis_angle_to_gimbal_axis`, `transform_orientations_create_from_axis`,
//! `createSpaceNormal` and the active-vertex branch of `getTransformOrientation_ex` /
//! `ED_getTransformOrientationMatrix` (`editors/transform/transform_orientations.cc`),
//! `eulO_to_gimbal_axis` and `mul_qt_v3` (`blenlib/intern/math_rotation_c.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! The active vertex (the last selection-history entry, when a vertex) of the context edit
//! mesh gives the reference: `value = dot(axis, world position)` with `axis` the chosen axis
//! of `orientation` (GLOBAL; LOCAL: the object's normalised axes; NORMAL: the active vertex's
//! normal frame; GIMBAL: the object's Euler or axis-angle gimbal axes, else PARENT; PARENT: the
//! parent's normalised axes, else the identity; VIEW: the 3D View's inverse view matrix;
//! CURSOR: the 3D cursor's rotation). Then every unselected visible vertex of every object in
//! Edit Mode with `dot(axis, its world position)` above `value - threshold` (`POS`), below
//! `value + threshold` (`NEG`) or within `threshold` of it (`ALIGN`) is selected, and the
//! selection flushed. Without an active vertex the operator is `CANCELLED` with Blender's
//! WARNING `This operator requires an active vertex (last selected)`.

use frap_bmesh::{BMElem, BMesh, VertId};
use frap_math::EulerOrder;
use frap_math::euler::eul_o_to_mat3;
use frap_math::matrix::RotationMode;
use frap_math::quat::{
    QuatWxyz, axis_angle_to_mat3, axis_angle_to_quat, normalize_qt, quat_to_mat3,
};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{MeshId, ObjectId};
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh::{
    self, select_axis::Axis, select_axis::Orientation, select_axis::Sign,
};
use glam::Mat3;

use super::select_more::common;
use super::select_similar::math::{self, M3, M4, V3, cross, dot, normalize_v3};

/// A glam `Mat3` as Blender's `float[3][3]`.
fn m3(m: Mat3) -> M3 {
    m.to_cols_array_2d()
}

/// `transform_orientations_create_from_axis` with the three axes of `m`.
fn create_from_axis(m: &M3) -> M3 {
    let mut mat = [[0.0f32; 3]; 3];
    let mut is_zero = [true; 3];
    for i in 0..3 {
        let (n, l) = normalize_v3(m[i]);
        mat[i] = n;
        is_zero[i] = l == 0.0;
    }
    let zero_axis = is_zero.iter().filter(|&&z| z).count();
    if zero_axis == 0 {
        return mat;
    }
    if zero_axis == 1 {
        let axis = is_zero.iter().position(|&z| z).unwrap_or(2);
        let c = cross(mat[(axis + 1) % 3], mat[(axis + 2) % 3]);
        let (n, l) = normalize_v3(c);
        mat[axis] = n;
        if l != 0.0 {
            return mat;
        }
    } else if zero_axis == 2 {
        let axis = is_zero.iter().position(|&z| !z).unwrap_or(0);
        let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
        mat[a][a] = 1.0;
        mat[b][b] = 1.0;
        let project = |p: V3, n: V3| -> V3 {
            // `project_plane_v3_v3v3`.
            let mul = dot(p, n) / dot(n, n);
            [p[0] - n[0] * mul, p[1] - n[1] * mul, p[2] - n[2] * mul]
        };
        mat[a] = project(mat[a], mat[axis]);
        mat[b] = project(mat[b], mat[axis]);
        let (na, la) = normalize_v3(mat[a]);
        mat[a] = na;
        if la != 0.0 {
            let (nb, lb) = normalize_v3(mat[b]);
            mat[b] = nb;
            if lb != 0.0 {
                return mat;
            }
        }
    }
    IDENTITY
}

const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `createSpaceNormal` (the identity when the normal is zero).
fn create_space_normal(normal: V3) -> M3 {
    let (n, l) = normalize_v3(normal);
    if l == 0.0 {
        return IDENTITY;
    }
    let mut mat = [[0.0f32; 3]; 3];
    mat[2] = n;
    let mut tangent = [0.0, 0.0, 1.0];
    mat[0] = cross(mat[2], tangent);
    if mat[0] == [0.0; 3] {
        tangent = [1.0, 0.0, 0.0];
        mat[0] = cross(tangent, mat[2]);
    }
    mat[1] = cross(mat[2], mat[0]);
    math::normalize_m3(&mat)
}

/// `mul_qt_v3`.
fn mul_qt_v3(q: QuatWxyz, r: V3) -> V3 {
    let t0 = -q[1] * r[0] - q[2] * r[1] - q[3] * r[2];
    let t1 = q[0] * r[0] + q[2] * r[2] - q[3] * r[1];
    let t2 = q[0] * r[1] + q[3] * r[0] - q[1] * r[2];
    let r2 = q[0] * r[2] + q[1] * r[1] - q[2] * r[0];
    let (r0, r1) = (t1, t2);
    let o0 = t0 * -q[1] + r0 * q[0] - r1 * q[3] + r2 * q[2];
    let o1 = t0 * -q[2] + r1 * q[0] - r2 * q[1] + r0 * q[3];
    let o2 = t0 * -q[3] + r2 * q[0] - r0 * q[2] + r1 * q[1];
    [o0, o1, o2]
}

/// The axis indices of an Euler order, first applied first (`RotOrderInfo::axis`).
fn order_axes(order: EulerOrder) -> [usize; 3] {
    let s = order.as_str().as_bytes();
    let idx = |c: u8| (c - b'X') as usize;
    [idx(s[0]), idx(s[1]), idx(s[2])]
}

/// `eulO_to_gimbal_axis`.
fn eul_o_to_gimbal_axis(eul: [f32; 3], order: EulerOrder) -> M3 {
    let ax = order_axes(order);
    let mut gmat = [[0.0f32; 3]; 3];
    let mat = m3(eul_o_to_mat3(eul, order));
    gmat[ax[0]] = mat[ax[0]];
    let mut teul = eul;
    teul[ax[0]] = 0.0;
    let mat = m3(eul_o_to_mat3(teul, order));
    gmat[ax[1]] = mat[ax[1]];
    gmat[ax[2]] = [0.0; 3];
    gmat[ax[2]][ax[2]] = 1.0;
    gmat
}

/// `axis_angle_to_gimbal_axis`.
fn axis_angle_to_gimbal_axis(axis: V3, angle: f32) -> M3 {
    let cross_vec = [axis[1], axis[2], axis[0]];
    let mut gmat = [[0.0f32; 3]; 3];
    gmat[0] = normalize_v3(cross(cross_vec, axis)).0;
    gmat[0] = mul_qt_v3(axis_angle_to_quat(axis, angle), gmat[0]);
    gmat[1] = mul_qt_v3(
        axis_angle_to_quat(axis, std::f32::consts::FRAC_PI_2),
        gmat[0],
    );
    gmat[2] = axis;
    math::normalize_m3(&gmat)
}

/// The object data the orientation reads.
struct ObInfo {
    obmat: M4,
    rotation_mode: Option<RotationMode>,
    rotation_euler: [f32; 3],
    rotation_axis_angle: [f32; 4],
    parent_mat: Option<M4>,
}

/// `handle_object_parent_orientation`.
fn parent_orientation(o: &ObInfo) -> M3 {
    match &o.parent_mat {
        Some(p) => create_from_axis(&math::m3_of(p)),
        None => IDENTITY,
    }
}

/// `gimbal_axis_object`.
fn gimbal_axis_object(o: &ObInfo) -> Option<M3> {
    let g = match o.rotation_mode? {
        RotationMode::Euler(order) => eul_o_to_gimbal_axis(o.rotation_euler, order),
        RotationMode::AxisAngle => {
            let a = o.rotation_axis_angle;
            axis_angle_to_gimbal_axis([a[1], a[2], a[3]], a[0])
        }
        RotationMode::Quaternion => return None,
    };
    Some(match &o.parent_mat {
        Some(p) => math::mul_m3_m3m3(&math::normalize_m3(&math::m3_of(p)), &g),
        None => g,
    })
}

/// `ED_getTransformOrientationMatrix` for the active vertex (`ORIENTATION_VERT`): the frame of
/// the vertex normal mapped by the inverse transpose of the object matrix.
fn normal_orientation(bm: &BMesh, v: VertId, obmat: &M4) -> M3 {
    let imat = math::invert_m3(&math::m3_of(obmat));
    // `transpose_m3(mat)` of the inverse, then `mul_m3_v3(mat, normal)`.
    let normal = math::mul_transposed_m3_v3(&imat, bm.vert(v).no);
    create_space_normal(normalize_v3(normal).0)
}

/// `calc_orientation_from_type_ex` (Edit Mode, pivot `ACTIVE_ELEMENT`).
fn orientation_matrix(cx: &OpCtx, kind: Orientation, o: &ObInfo, bm: &BMesh, v: VertId) -> M3 {
    let c = cx.ctx();
    match kind {
        Orientation::Global => IDENTITY,
        Orientation::Local => create_from_axis(&math::m3_of(&o.obmat)),
        Orientation::Normal => normal_orientation(bm, v, &o.obmat),
        Orientation::Gimbal => gimbal_axis_object(o).unwrap_or_else(|| parent_orientation(o)),
        Orientation::Parent => parent_orientation(o),
        Orientation::View => {
            let viewinv = c.win.view3d.view_matrix.inverse();
            math::normalize_m3(&math::m3_of(&math::m4(&viewinv)))
        }
        Orientation::Cursor => {
            let cur = &c.scene().cursor;
            let m = match RotationMode::from_blender(&cur.rotation_mode) {
                Some(RotationMode::AxisAngle) => {
                    let a = cur.rotation_axis_angle;
                    axis_angle_to_mat3([a[1], a[2], a[3]], a[0])
                }
                Some(RotationMode::Quaternion) => {
                    quat_to_mat3(normalize_qt(cur.rotation_quaternion).0)
                }
                Some(RotationMode::Euler(order)) => eul_o_to_mat3(cur.rotation_euler, order),
                None => eul_o_to_mat3(cur.rotation_euler, EulerOrder::XYZ),
            };
            m3(m)
        }
    }
}

fn ob_info(cx: &OpCtx, ob: frap_rna::IdRef<ObjectId>) -> ObInfo {
    let main = cx.main();
    let obmat = math::m4(&world_matrix_in(main, ob));
    let o = main.objects.get(ob);
    ObInfo {
        obmat,
        rotation_mode: o.and_then(|o| RotationMode::from_blender(&o.rotation_mode)),
        rotation_euler: o.map_or([0.0; 3], |o| o.rotation_euler),
        rotation_axis_angle: o.map_or([0.0, 0.0, 1.0, 0.0], |o| o.rotation_axis_angle),
        parent_mat: o
            .and_then(|o| o.parent)
            .map(|p| math::m4(&world_matrix_in(main, p))),
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectAxis) -> OpResult {
    let edit = {
        let c = cx.ctx();
        poll::edit_object(&c).and_then(|(r, o)| Some((r, o.data?.typed::<MeshId>()?)))
    };
    let Some((obedit, me_edit)) = edit else {
        return Ok(OpStatus::Cancelled);
    };
    // `BM_mesh_active_vert_get`.
    let v_act = common::read(cx, me_edit, |bm| match bm.select_history.last() {
        Some(BMElem::Vert(v)) => Some(*v),
        _ => None,
    })
    .flatten();
    let Some(v_act) = v_act else {
        cx.report(
            ReportType::Warning,
            "This operator requires an active vertex (last selected)",
        );
        return Ok(OpStatus::Cancelled);
    };
    let limit = p.threshold.value;
    let info = ob_info(cx, obedit);
    let axis = match p.axis.value {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };
    let (axis_vector, mut value) = {
        let (_, edit) = cx.split_edit();
        let bm = edit
            .meshes
            .get(&me_edit.key())
            .map(|em| em.bm.clone())
            .ok_or_else(|| OpError::Runtime("no edit mesh".into()))?;
        let mat = orientation_matrix(cx, p.orientation.value, &info, &bm, v_act);
        let axis_vector = mat[axis];
        let world = math::mul_m4_v3(&info.obmat, bm.vert(v_act).co);
        (axis_vector, dot(axis_vector, world))
    };
    match p.sign.value {
        Sign::Neg => value += limit,
        Sign::Pos => value -= limit,
        Sign::Align => {}
    }
    let objects: Vec<(frap_rna::IdRef<ObjectId>, frap_rna::IdRef<MeshId>)> = {
        let c = cx.ctx();
        c.objects_in_mode()
            .into_iter()
            .filter_map(|r| Some((r, c.main.objects.get(r)?.data?.typed::<MeshId>()?)))
            .collect()
    };
    for (ob, me) in objects {
        let obmat = math::m4(&world_matrix_in(cx.main(), ob));
        let Some(Some(sel)) = common::read(cx, me, |bm| {
            let (tv, _, _) = bm.selected_counts();
            if bm.verts_num() == tv {
                return None;
            }
            Some(
                bm.vert_ids()
                    .into_iter()
                    .filter(|&v| !common::hidden(bm, v) && !common::selected(bm, v))
                    .filter(|&v| {
                        let w = math::mul_m4_v3(&obmat, bm.vert(v).co);
                        let value_iter = dot(axis_vector, w);
                        match p.sign.value {
                            Sign::Align => (value_iter - value).abs() < limit,
                            Sign::Neg => value_iter < value,
                            Sign::Pos => value_iter > value,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        }) else {
            continue;
        };
        if sel.is_empty() {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        for v in sel {
            bm.vert_select_set(v, true);
        }
        common::selectmode_flush(bm);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_normal_of_z_is_identity_like() {
        let m = create_space_normal([0.0, 0.0, 2.0]);
        assert_eq!(m[2], [0.0, 0.0, 1.0]);
        // cross(Z, Z) is zero: the tangent becomes X, and X × Z = -Y.
        assert_eq!(m[0], [0.0, -1.0, 0.0]);
        assert_eq!(m[1], [1.0, 0.0, 0.0]);
        assert_eq!(create_space_normal([0.0; 3]), IDENTITY);
    }

    #[test]
    fn axes_from_matrix_handle_zero_axes() {
        let m = create_from_axis(&[[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]]);
        assert_eq!(m, IDENTITY);
        let g = eul_o_to_gimbal_axis([0.0; 3], EulerOrder::XYZ);
        assert_eq!(g, IDENTITY);
        let q = axis_angle_to_quat([0.0, 0.0, 1.0], 0.0);
        assert_eq!(mul_qt_v3(q, [1.0, 2.0, 3.0]), [1.0, 2.0, 3.0]);
    }
}
