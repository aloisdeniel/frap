// SPDX-License-Identifier: GPL-3.0-or-later
//! Object-mode transform handler (`TRANSFORM_MODE_HANDLERS`, WP 1.21): `transform.translate`,
//! `rotate` and `resize` on the selected objects, executed non-interactively the way Blender's
//! `exec` runs them (`T_INPUT_IS_VALUES_FINAL`). Ports of `applyTranslation`,
//! `applyTranslationValue` (`transform_mode_translate.cc`), `applyRotation`,
//! `applyRotationValue`, `ElementRotation_ex` (`transform_mode_rotate.cc`,
//! `transform_mode.cc`), `applyResize`, `ElementResize` (`transform_mode_resize.cc`) and the
//! 3D cursor conversion (`transform_convert_cursor.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (besides the orientation, constraint and pivot rules of `core`):
//! - an unset `value` is zero: `translate()` and `rotate()` change nothing, `resize()` scales
//!   to zero;
//! - `rotate` without a constraint turns about the **negated** orientation axis
//!   (`rotate(value=0.5, orient_axis='Z')` gives `rotation_euler.z == -0.5`), with a
//!   constraint about the axis itself (`+0.5`);
//! - rotation and resize never shear an object: the rotation channels take the rotated
//!   rotation (compatible Euler), the scale channels the new size measured along the object's
//!   own axes (`resize(value=(2, 1, 1))` of a cube turned 0.5 rad gives scale
//!   `(1.8195, 1.2998, 1.0)`);
//! - nothing to transform → `CANCELLED`;
//! - `tool_settings.use_transform_pivot_point_align` moves locations only;
//! - `snap`/`snap_point` change nothing without an interactive session; `cursor_transform`
//!   moves the 3D cursor instead of the objects.

pub mod data;

use frap_math::euler::{eul_o_to_mat3, mat3_to_compatible_eul_o};
use frap_math::matrix::RotationMode;
use frap_math::quat::{
    axis_angle_to_quat, mat3_to_quat, mul_qt_qtqt, quat_to_axis_angle, quat_to_mat3,
};
use frap_op::prelude::*;
use frap_op::{TRANSFORM_MODE_HANDLERS, TransformModeHandler, TransformOp};
use frap_rna::OpArgs;
use frap_scene::generated::plain::View3DCursor;
use frap_scene::id::transform::world_matrix;
use frap_scene::{Context, ContextMode};
use glam::{Mat3, Vec3};

use crate::core::constraint::{Constraint, ConstraintSpace};
use crate::core::math::{axis_angle_normalized_to_mat3, size_to_mat3, trans_mat3_to_size};
use crate::core::orient::{OrientSource, OrientType, Orientation, gimbal_axis_object};
use crate::core::params::TransformParams;
use crate::core::pivot::{self, PivotPoint};
use crate::core::proportional::Proportional;
use data::{Channels, ObjectTd};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(TRANSFORM_MODE_HANDLERS)]
static OBJECT: TransformModeHandler = TransformModeHandler {
    mode: ContextMode::Object,
    apply,
};

/// `BKE_scene_cursor_rot_to_mat3` (the quaternion is used as stored).
pub fn cursor_rot_mat3(cursor: &View3DCursor) -> Mat3 {
    match RotationMode::from_blender(&cursor.rotation_mode) {
        Some(RotationMode::AxisAngle) => {
            let a = cursor.rotation_axis_angle;
            frap_math::quat::axis_angle_to_mat3([a[1], a[2], a[3]], a[0])
        }
        Some(RotationMode::Quaternion) => frap_math::quat::quat_to_mat3(cursor.rotation_quaternion),
        Some(RotationMode::Euler(order)) => eul_o_to_mat3(cursor.rotation_euler, order),
        None => eul_o_to_mat3(cursor.rotation_euler, frap_math::EulerOrder::XYZ),
    }
}

/// Stores `View3DCursor.matrix` (rows) from its location and rotation, as Blender's getter
/// computes it (`BKE_scene_cursor_to_mat4`).
pub fn sync_cursor_matrix(cursor: &mut View3DCursor) {
    let r = cursor_rot_mat3(cursor);
    let l = cursor.location;
    for (row, out) in cursor.matrix.iter_mut().enumerate().take(3) {
        *out = [r.col(0)[row], r.col(1)[row], r.col(2)[row], l[row]];
    }
    cursor.matrix[3] = [0.0, 0.0, 0.0, 1.0];
}

/// The orientation inputs of the context (`OrientSource`), from the active object.
pub fn orient_source(c: &Context) -> OrientSource {
    let objects = &c.main.objects;
    let active = c
        .active_object()
        .and_then(|r| objects.get(r).map(|o| (r, o)));
    let parent_world = active
        .and_then(|(_, o)| o.parent)
        .filter(|p| objects.contains(*p))
        .map(|p| Mat3::from_mat4(world_matrix(objects, p)));
    OrientSource {
        active_world: active.map(|(r, _)| Mat3::from_mat4(world_matrix(objects, r))),
        active_gimbal: active.and_then(|(_, o)| {
            gimbal_axis_object(
                o.rotation_mode_enum(),
                o.rotation_euler,
                o.rotation_axis_angle,
                parent_world.as_ref(),
            )
        }),
        active_parent_world: parent_world,
        cursor: cursor_rot_mat3(c.cursor()),
        view: Mat3::from_mat4(c.win.view3d.view_matrix),
    }
}

/// What one transform does: the orientation, constraint, pivot and the shared operator
/// matrix (`mat` of rotate/resize, `vec` of translate).
struct Setup {
    op: TransformOp,
    orient: Orientation,
    con: Option<Constraint>,
    pivot: PivotPoint,
    center: Vec3,
    /// Translate: the global vector; rotate: the axis; resize: unused.
    vec: Vec3,
    /// Rotate: the angle; others unused.
    angle: f32,
    /// Resize: the size matrix after the shared constraint.
    mat: Mat3,
    align: bool,
    proportional: bool,
}

impl Setup {
    /// The per-element axes of a per-element constraint (`td->axismtx`, gimbal axes for
    /// GIMBAL).
    fn elem_axes(&self, td: &ObjectTd) -> Mat3 {
        if self.orient.kind == OrientType::Gimbal {
            td.gimbal
        } else {
            td.axismtx
        }
    }
}

/// `transdata_elem_translate`.
fn translate_td(s: &Setup, td: &ObjectTd, out: &mut Channels) {
    let mut tvec = match s.con {
        Some(con) => con.apply_vec_elem(&s.orient, &s.elem_axes(td), s.vec),
        None => s.vec,
    };
    tvec = td.smtx * tvec;
    tvec *= td.factor;
    out.location = td.init.location + tvec;
}

/// `ElementRotation_ex` for an object.
fn rotate_td(s: &Setup, td: &ObjectTd, mat: &Mat3, out: &mut Channels) {
    let center = if s.pivot.is_local() {
        td.center
    } else {
        s.center
    };
    if !td.no_loc {
        let mut vec = *mat * (td.center - center) + center;
        vec -= td.center;
        vec = td.smtx * vec;
        out.location = td.init.location + vec;
    }
    if s.align {
        return;
    }
    // Blender 5.2.4 rotates the delta-rotated orientation and takes the delta back out
    // (verified with delta rotations in Euler and quaternion mode): for a quaternion
    // `q' = conj(dquat) · quat(smtx · mat · mtx · mat3(dquat)) · iquat` (the delta used as
    // stored, unnormalized), for Euler angles `R' = D⁻¹ · smtx · mat · mtx · D · R` with `D`
    // the delta Euler matrix.
    let smat = td.smtx * (*mat * td.mtx);
    match td.init.mode {
        RotationMode::Quaternion => {
            let d = td.init.dquat;
            let quat = mat3_to_quat(&(smat * quat_to_mat3(d)));
            let conj = [d[0], -d[1], -d[2], -d[3]];
            out.quat = mul_qt_qtqt(mul_qt_qtqt(conj, quat), td.init.quat);
        }
        RotationMode::AxisAngle => {
            // RNA has no delta axis-angle: DNA's default is the identity.
            let iquat = axis_angle_to_quat(td.init.axis, td.init.angle);
            let quat = mat3_to_quat(&smat);
            let tquat = mul_qt_qtqt(quat, iquat);
            let (axis, angle) = quat_to_axis_angle(tquat);
            out.axis = axis;
            out.angle = angle;
        }
        RotationMode::Euler(order) => {
            let d = eul_o_to_mat3(td.init.drot, order);
            let obmat = eul_o_to_mat3(td.init.euler, order);
            let f = d.transpose() * (smat * (d * obmat));
            out.euler = mat3_to_compatible_eul_o(&f, td.init.euler, order);
        }
    }
}

/// `ElementResize` for an object.
fn resize_td(s: &Setup, td: &ObjectTd, out: &mut Channels) {
    let mut tmat = s.mat;
    if let Some(con) = s.con {
        tmat = con.apply_size_elem(&s.elem_axes(td), tmat);
    }
    let center = if s.pivot.is_local() {
        td.center
    } else {
        s.center
    };
    if !s.align {
        let fscale = trans_mat3_to_size(&(tmat * td.axismtx), &td.axismtx);
        out.scale = td.init.scale * (Vec3::ONE + (fscale - Vec3::ONE) * td.factor);
    }
    let mut vec = tmat * (td.center - center) + center;
    vec -= td.center;
    vec = td.smtx * vec;
    vec *= td.factor;
    out.location = td.init.location + vec;
}

fn element(s: &Setup, td: &ObjectTd) -> Channels {
    let mut out = td.init;
    match s.op {
        TransformOp::Rotate => {
            let mut axis = s.vec;
            let mut angle = s.angle;
            if let Some(con) = s.con {
                if con.space == ConstraintSpace::PerElement {
                    axis = con.rot_axis(&s.elem_axes(td));
                }
                angle *= td.factor;
            } else if s.proportional {
                angle *= td.factor;
            }
            let mat = axis_angle_normalized_to_mat3(axis, angle);
            rotate_td(s, td, &mat, &mut out);
        }
        TransformOp::Resize => resize_td(s, td, &mut out),
        _ => translate_td(s, td, &mut out),
    }
    out
}

/// Refuses options frap does not implement in object mode (they change the result).
fn unsupported(p: &TransformParams, c: &Context) -> Result<(), OpError> {
    let ts = c.tool_settings();
    let what = if p.texture_space {
        "texture_space=True"
    } else if p.translate_origin {
        "translate_origin=True"
    } else if ts.use_transform_data_origin {
        "tool_settings.use_transform_data_origin (Affect Only Origins)"
    } else if ts.use_transform_skip_children {
        "tool_settings.use_transform_skip_children (Affect Only Parents)"
    } else {
        return Ok(());
    };
    Err(OpError::NotImplemented(format!(
        "frap: object-mode transforms with {what} are not implemented yet"
    )))
}

/// Builds the setup shared by every element.
fn setup(c: &Context, p: &TransformParams, tds: &[ObjectTd]) -> Setup {
    let src = orient_source(c);
    let orient = Orientation::from_props(
        p.orient_type.as_deref(),
        p.orient_matrix,
        &p.orient_matrix_type,
        &src,
    );
    let con = Constraint::from_props(
        p.value_is_set_array(),
        p.constraint_axis,
        &orient,
        tds.len(),
    );
    let pivot = PivotPoint::from_identifier(&c.tool_settings().transform_pivot_point);
    let centers: Vec<Vec3> = tds
        .iter()
        .filter(|t| t.selected && !t.no_center)
        .map(|t| t.center)
        .collect();
    let active_center = c
        .active_object()
        .filter(|a| c.main.objects.contains(*a))
        .map(|a| world_matrix(&c.main.objects, a).w_axis.truncate());
    let center = pivot::center(
        pivot,
        &centers,
        Vec3::from_array(c.cursor().location),
        active_center,
        p.center_override,
    );
    let values = p.value.unwrap_or(Vec3::ZERO);
    let (mut vec, mut angle, mut mat) = (Vec3::ZERO, 0.0, Mat3::IDENTITY);
    match p.op {
        TransformOp::Rotate => {
            angle = p.angle();
            vec = match con {
                Some(con) => {
                    let first = tds.first();
                    let axes = match (con.space, first) {
                        (ConstraintSpace::PerElement, Some(td))
                            if orient.kind == OrientType::Gimbal =>
                        {
                            td.gimbal
                        }
                        (ConstraintSpace::PerElement, Some(td)) => td.axismtx,
                        _ => orient.matrix,
                    };
                    con.rot_axis(&axes)
                }
                None => -orient.matrix.col(p.orient_axis),
            };
        }
        TransformOp::Resize => {
            mat = size_to_mat3(values);
            if let Some(con) = con {
                mat = con.apply_size(&orient, mat);
            }
        }
        _ => {
            vec = orient.matrix * values;
            if let Some(con) = con {
                vec = con.apply_vec(&orient, vec);
            }
        }
    }
    Setup {
        op: p.op,
        orient,
        con,
        pivot,
        center,
        vec,
        angle,
        mat,
        align: c.tool_settings().use_transform_pivot_point_align,
        proportional: p.proportional.is_some(),
    }
}

/// Proportional factors (`set_prop_dist` + `calculatePropRatio`): the distance of each
/// unselected element to the nearest selected one, both centers first mapped by their own
/// `td->mtx` as `set_prop_dist` does for every mode (`mul_v3_m3v3(vec, td->mtx, td->center)`).
/// For an object `td->mtx` is the parent's world 3×3 (identity without a parent) while the
/// center is already global, so a child is measured from a displaced point (verified: a cube
/// at local (0, 3, 0) under a parent at (1, 0, 0) turned 90° about Z sits at (-2, 0, 0) but is
/// measured from (0, -2, 0); with the parent scaled 2 from twice its world location).
fn set_factors(tds: &mut [ObjectTd], prop: &mut Proportional) {
    let point = |t: &ObjectTd| t.mtx * t.center;
    let sel: Vec<Vec3> = tds.iter().filter(|t| t.selected).map(point).collect();
    for td in tds.iter_mut().filter(|t| !t.selected) {
        let p = point(td);
        let rdist = sel
            .iter()
            .map(|c| (*c - p).length())
            .fold(f32::MAX, f32::min);
        td.factor = prop.factor(rdist);
    }
}

/// `transform.translate(cursor_transform=True)`: moves the 3D cursor.
fn translate_cursor(cx: &mut OpCtx, p: &TransformParams) -> OpResult {
    let (scene, vec) = {
        let c = cx.ctx();
        let src = orient_source(&c);
        let orient = Orientation::from_props(
            p.orient_type.as_deref(),
            p.orient_matrix,
            &p.orient_matrix_type,
            &src,
        );
        let mut vec = orient.matrix * p.value.unwrap_or(Vec3::ZERO);
        if let Some(con) =
            Constraint::from_props(p.value_is_set_array(), p.constraint_axis, &orient, 1)
        {
            vec = con.apply_vec(&orient, vec);
        }
        (c.win.scene, vec)
    };
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        let l = Vec3::from_array(s.cursor.location) + vec;
        s.cursor.location = l.to_array();
        sync_cursor_matrix(&mut s.cursor);
    }
    Ok(OpStatus::Finished)
}

/// The object-mode handler.
fn apply(cx: &mut OpCtx, op: TransformOp, args: &OpArgs) -> OpResult {
    let p = TransformParams::from_args(op, args);
    unsupported(&p, &cx.ctx())?;
    if p.cursor_transform {
        return translate_cursor(cx, &p);
    }
    let results: Vec<(ObjectTd, Channels)> = {
        let c = cx.ctx();
        let pivot = PivotPoint::from_identifier(&c.tool_settings().transform_pivot_point);
        let in_place = pivot.is_local() && op == TransformOp::Rotate;
        let mut tds = data::create(&c, in_place, p.proportional.is_some());
        if !tds.iter().any(|t| t.selected) {
            cx.report(
                ReportType::Warning,
                "frap: nothing to transform: no visible object is selected",
            );
            return Ok(OpStatus::Cancelled);
        }
        if let Some((falloff, size)) = p.proportional {
            set_factors(&mut tds, &mut Proportional::new(falloff, size));
        }
        let s = setup(&c, &p, &tds);
        tds.into_iter()
            .filter(|t| !t.skip)
            .map(|t| {
                let out = element(&s, &t);
                (t, out)
            })
            .collect()
    };
    for (td, out) in results {
        if out != td.init {
            out.write(cx.object_mut(td.ob)?);
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn td(center: Vec3, axes: Mat3, ch: Channels) -> ObjectTd {
        ObjectTd {
            ob: frap_rna::IdRef::from_key(frap_rna::IdKey::next()),
            selected: true,
            skip: false,
            no_center: false,
            no_loc: false,
            factor: 1.0,
            center,
            axismtx: axes,
            gimbal: axes,
            mtx: Mat3::IDENTITY,
            smtx: Mat3::IDENTITY,
            init: ch,
        }
    }

    fn ch(euler: [f32; 3]) -> Channels {
        Channels {
            location: Vec3::ZERO,
            mode: RotationMode::Euler(frap_math::EulerOrder::XYZ),
            euler,
            quat: [1.0, 0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            angle: 0.0,
            scale: Vec3::ONE,
            drot: [0.0; 3],
            dquat: [1.0, 0.0, 0.0, 0.0],
        }
    }

    fn setup(op: TransformOp, vec: Vec3, angle: f32, mat: Mat3, con: Option<Constraint>) -> Setup {
        Setup {
            op,
            orient: Orientation::GLOBAL,
            con,
            pivot: PivotPoint::MedianPoint,
            center: Vec3::ZERO,
            vec,
            angle,
            mat,
            align: false,
            proportional: false,
        }
    }

    #[test]
    fn rotate_without_constraint_is_negative() {
        // `rotate(value=0.5, orient_axis='Z')` gives rotation_euler.z == -0.5 (verified).
        let s = setup(TransformOp::Rotate, -Vec3::Z, 0.5, Mat3::IDENTITY, None);
        let out = element(&s, &td(Vec3::ZERO, Mat3::IDENTITY, ch([0.0; 3])));
        assert!((out.euler[2] + 0.5).abs() < 1e-6, "{:?}", out.euler);
    }

    #[test]
    fn rotate_euler_with_delta() {
        // drot (0.2, 0, 0), rot (0, 0.3, 0), rotate 0.5 about -Z: (0.0243, 0.2045, -0.4843)
        // (verified on 5.2.4).
        let mut c = ch([0.0, 0.3, 0.0]);
        c.drot = [0.2, 0.0, 0.0];
        let s = setup(TransformOp::Rotate, -Vec3::Z, 0.5, Mat3::IDENTITY, None);
        let out = element(&s, &td(Vec3::ZERO, Mat3::IDENTITY, c));
        let want = [0.0243, 0.2045, -0.4843];
        for (got, want) in out.euler.iter().zip(want) {
            assert!((got - want).abs() < 1e-4, "{:?}", out.euler);
        }
    }

    #[test]
    fn rotate_quaternion_about_center() {
        // Quaternion cube at (1, 0, 0), rotate 0.5 about -Y around the origin: location
        // (0.8776, 0, 0.4794), quaternion (0.9689, 0, -0.2474, 0) (verified).
        let mut c = ch([0.0; 3]);
        c.mode = RotationMode::Quaternion;
        c.location = Vec3::X;
        let s = setup(TransformOp::Rotate, -Vec3::Y, 0.5, Mat3::IDENTITY, None);
        let out = element(&s, &td(Vec3::X, Mat3::IDENTITY, c));
        assert!(
            out.location
                .abs_diff_eq(Vec3::new(0.8776, 0.0, 0.4794), 1e-4),
            "{:?}",
            out.location
        );
        let q = out.quat;
        assert!(
            (q[0] - 0.9689).abs() < 1e-4 && (q[2] + 0.2474).abs() < 1e-4,
            "{q:?}"
        );
    }

    #[test]
    fn rotate_quaternion_with_delta() {
        // quaternion (0.9, 0.1, 0.2, 0.3), delta (0.9, 0.3, 0.2, 0.1), rotate 0.5 about -X:
        // Blender 5.2.4 gives (0.900987, -0.090231, 0.252345, 0.137104).
        let mut c = ch([0.0; 3]);
        c.mode = RotationMode::Quaternion;
        c.quat = [0.9, 0.1, 0.2, 0.3];
        c.dquat = [0.9, 0.3, 0.2, 0.1];
        let s = setup(TransformOp::Rotate, -Vec3::X, 0.5, Mat3::IDENTITY, None);
        let out = element(&s, &td(Vec3::ZERO, Mat3::IDENTITY, c));
        let want = [0.900987, -0.090231, 0.252345, 0.137104];
        for (got, want) in out.quat.iter().zip(want) {
            assert!((got - want).abs() < 1e-5, "{:?}", out.quat);
        }
    }

    #[test]
    fn resize_rotated_object_keeps_its_rotation() {
        let axes = Mat3::from_rotation_z(0.5);
        let s = setup(
            TransformOp::Resize,
            Vec3::ZERO,
            0.0,
            Mat3::from_diagonal(Vec3::new(2.0, 1.0, 1.0)),
            None,
        );
        let out = element(&s, &td(Vec3::ZERO, axes, ch([0.0, 0.0, 0.5])));
        assert!(
            out.scale.abs_diff_eq(Vec3::new(1.8195, 1.2998, 1.0), 1e-4),
            "{:?}",
            out.scale
        );
        assert_eq!(out.euler, [0.0, 0.0, 0.5]);
    }

    #[test]
    fn xyz_rotation_constraint_scales_positions_by_cos() {
        let con = Constraint {
            axes: [true; 3],
            space: ConstraintSpace::Orientation,
        };
        let s = setup(
            TransformOp::Rotate,
            con.rot_axis(&Mat3::IDENTITY),
            0.5,
            Mat3::IDENTITY,
            Some(con),
        );
        let mut c = ch([0.0; 3]);
        c.location = Vec3::new(2.0, 0.0, 0.0);
        let out = element(&s, &td(Vec3::new(2.0, 0.0, 0.0), Mat3::IDENTITY, c));
        assert!((out.location.x - 2.0 * libm::cosf(0.5)).abs() < 1e-6);
        assert!(out.euler.iter().all(|e| e.abs() < 1e-6), "{:?}", out.euler);
    }

    #[test]
    fn cursor_matrix_rows() {
        let mut cur = View3DCursor {
            location: [1.0, 2.0, 3.0],
            rotation_euler: [0.0, 0.0, 0.5],
            ..View3DCursor::default()
        };
        sync_cursor_matrix(&mut cur);
        assert!((cur.matrix[0][3] - 1.0).abs() < 1e-7 && (cur.matrix[2][3] - 3.0).abs() < 1e-7);
        assert!(
            (cur.matrix[1][0] - libm::sinf(0.5)).abs() < 1e-6,
            "{:?}",
            cur.matrix
        );
        assert_eq!(cur.matrix[3], [0.0, 0.0, 0.0, 1.0]);
    }
}
