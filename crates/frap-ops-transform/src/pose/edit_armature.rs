// SPDX-License-Identifier: GPL-3.0-or-later
//! Armature Edit Mode `TRANSFORM_MODE_HANDLERS` entry (WP 6.2): `transform.translate`,
//! `rotate` and `resize` on the selected heads and tails of the edit bones of every armature
//! in Edit Mode (what `armature.extrude_move` and `armature.duplicate_move` run). Ports of
//! `createTransArmatureVerts` and `recalcData_edit_armature`
//! (blender/source/blender/editors/transform/transform_convert_armature.cc) and the `T_POINTS`
//! branches of `ElementRotation_ex` / `ElementResize`
//! (blender/source/blender/editors/transform/transform_mode.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! - Every visible, unlocked bone contributes its selected tail and head (`BONE_TIPSEL`,
//!   `BONE_ROOTSEL`); with `INDIVIDUAL_ORIGINS` a tail turns about its head when rotating or
//!   when the head is selected too.
//! - After the move, connected bones follow (a moved parent tip drags the child's head, a
//!   moved child head drags the parent's tail), lengths are re-measured: a bone whose stored
//!   length was 0 (just extruded or added) gets radii and envelope from its new length
//!   (`rad_head = 0.25·len`, at most the parent's tail radius, `rad_tail = 0.1·len`,
//!   `dist = 0.25·len`), others scale them with the length; rolls are recomputed so the bone's
//!   Z axis follows the rotation (`ED_armature_ebone_roll_to_vector`, `angle_compat_rad`);
//!   `use_mirror_x` mirrors the result.
//!
//! - With one element (only a tip selected, as after `armature.extrude_move`), a rotate or
//!   resize about the median, bounds or active element uses local origins
//!   (`transform_around_single_fallback`): the tip turns about its bone's head.
//!
//! Proportional editing is ignored: every element is selected (`TD_SELECTED`), so the
//! falloff never reaches anything. `cursor_transform=True` is not implemented.

use std::sync::Arc;

use frap_math::quat::{angle_wrap_rad, axis_angle_normalized_to_quat};
use frap_op::prelude::*;
use frap_op::{TRANSFORM_MODE_HANDLERS, TransformModeHandler, TransformOp};
use frap_rna::{ArmatureId, IdRef, ObjectId, OpArgs};
use frap_scene::edit_armature::tools::roll_to_vector;
use frap_scene::edit_armature::{EditArmature, ROOT, TIP};
use frap_scene::id::transform::world_matrix;
use frap_scene::{Context, ContextMode, ObjectMode};
use glam::{Mat3, Vec3};

use crate::core::constraint::{Constraint, ConstraintSpace};
use crate::core::math::{
    axis_angle_normalized_to_mat3, invert_m3_safe_ortho, normalize_m3, size_to_mat3,
};
use crate::core::orient::{OrientType, Orientation};
use crate::core::params::TransformParams;
use crate::core::pivot::{self, PivotPoint};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(TRANSFORM_MODE_HANDLERS)]
static EDIT_ARMATURE: TransformModeHandler = TransformModeHandler {
    mode: ContextMode::EditArmature,
    apply,
};

/// Which end of a bone a transform element moves.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum End {
    Head,
    Tail,
}

/// One transformed point (`TransData` of `createTransArmatureVerts`).
#[derive(Clone, Debug)]
struct Td {
    arm: IdRef<ArmatureId>,
    bone: usize,
    end: End,
    /// `td->iloc` (armature space).
    iloc: Vec3,
    /// `td->center` (armature space).
    center: Vec3,
    /// `td->axismtx` (`ED_armature_ebone_to_mat3`, armature space).
    axismtx: Mat3,
    /// The roll is recomputed from this element (`td->extra`).
    fix_roll: bool,
    /// `td->ival`: the roll before the transform.
    iroll: f32,
    /// Object 3×3 world matrix (`td->mtx`) and its inverse (`td->smtx`), and the full object
    /// matrix (for centers).
    mtx: Mat3,
    smtx: Mat3,
    obmat: glam::Mat4,
}

/// The armatures in Edit Mode of the context (active first, one per armature).
fn edit_objects(c: &Context) -> Vec<(IdRef<ObjectId>, IdRef<ArmatureId>)> {
    c.objects_in_mode_unique_data()
        .into_iter()
        .filter_map(|ob| {
            let o = c.main.objects.get(ob)?;
            if o.mode != ObjectMode::Edit {
                return None;
            }
            Some((ob, o.data?.typed::<ArmatureId>()?))
        })
        .collect()
}

fn create(
    c: &Context,
    ea: &EditArmature,
    ob: IdRef<ObjectId>,
    arm: IdRef<ArmatureId>,
    op: TransformOp,
    local_origins: bool,
) -> Vec<Td> {
    let Some(a) = c.main.armatures.get(arm) else {
        return Vec::new();
    };
    let obmat = world_matrix(&c.main.objects, ob);
    let mtx = Mat3::from_mat4(obmat);
    let smtx = invert_m3_safe_ortho(&mtx);
    let mut out = Vec::new();
    for (i, b) in ea.edit_bones.iter().enumerate() {
        if !ea.is_visible(a, i) || b.lock {
            continue;
        }
        let axismtx = ea.bone_mat3(i);
        if b.select[TIP] {
            let center = if local_origins && (op == TransformOp::Rotate || b.select[ROOT]) {
                b.head_v()
            } else {
                b.tail_v()
            };
            out.push(Td {
                arm,
                bone: i,
                end: End::Tail,
                iloc: b.tail_v(),
                center,
                axismtx,
                fix_roll: !b.select[ROOT],
                iroll: b.roll,
                mtx,
                smtx,
                obmat,
            });
        }
        if b.select[ROOT] {
            out.push(Td {
                arm,
                bone: i,
                end: End::Head,
                iloc: b.head_v(),
                center: b.head_v(),
                axismtx,
                fix_roll: true,
                iroll: b.roll,
                mtx,
                smtx,
                obmat,
            });
        }
    }
    out
}

/// `rotation_between_vecs_to_quat`.
fn rotation_between_vecs_to_quat(v1: Vec3, v2: Vec3) -> [f32; 4] {
    let (axis, len) = frap_scene::edit_armature::math::normalize(v1.cross(v2));
    if len > f32::EPSILON {
        let angle = angle_normalized(v1, v2);
        axis_angle_normalized_to_quat(axis.to_array(), angle)
    } else if v1.dot(v2) > 0.0 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        let axis = ortho(v1);
        frap_math::quat::axis_angle_to_quat(axis.to_array(), std::f32::consts::PI)
    }
}

/// `angle_normalized_v3v3`.
fn angle_normalized(a: Vec3, b: Vec3) -> f32 {
    if a.dot(b) >= 0.0 {
        2.0 * ((b - a).length() / 2.0).clamp(-1.0, 1.0).asin()
    } else {
        std::f32::consts::PI - 2.0 * ((a + b).length() / 2.0).clamp(-1.0, 1.0).asin()
    }
}

/// `ortho_v3_v3`.
fn ortho(v: Vec3) -> Vec3 {
    let axis = if v.x.abs() > v.y.abs() {
        if v.x.abs() > v.z.abs() { 0 } else { 2 }
    } else if v.y.abs() > v.z.abs() {
        1
    } else {
        2
    };
    match axis {
        0 => Vec3::new(-v.y - v.z, v.x, v.x),
        1 => Vec3::new(v.y, -v.x - v.z, v.y),
        _ => Vec3::new(v.z, v.z, -v.x - v.y),
    }
}

/// `mul_qt_v3`.
fn mul_qt_v3(q: [f32; 4], r: Vec3) -> Vec3 {
    let mut r = r.to_array();
    let t0 = -q[1] * r[0] - q[2] * r[1] - q[3] * r[2];
    let mut t1 = q[0] * r[0] + q[2] * r[2] - q[3] * r[1];
    let mut t2 = q[0] * r[1] + q[3] * r[0] - q[1] * r[2];
    r[2] = q[0] * r[2] + q[1] * r[1] - q[2] * r[0];
    r[0] = t1;
    r[1] = t2;
    t1 = t0 * -q[1] + r[0] * q[0] - r[1] * q[3] + r[2] * q[2];
    t2 = t0 * -q[2] + r[1] * q[0] - r[2] * q[1] + r[0] * q[3];
    r[2] = t0 * -q[3] + r[2] * q[0] - r[0] * q[2] + r[1] * q[1];
    r[0] = t1;
    r[1] = t2;
    Vec3::from_array(r)
}

/// `recalcData_edit_armature` (not cancelled).
fn recalc(ea: &mut EditArmature, a: &frap_scene::id::Armature, tds: &[Td], old_lengths: &[f32]) {
    for i in 0..ea.edit_bones.len() {
        let parent = ea.edit_bones[i]
            .parent
            .filter(|_| ea.edit_bones[i].use_connect);
        if let Some(p) = parent {
            if ea.is_visible(a, p) && ea.edit_bones[p].select[TIP] {
                ea.edit_bones[i].head = ea.edit_bones[p].tail;
            } else {
                ea.edit_bones[p].tail = ea.edit_bones[i].head;
            }
        }
        let length = ea.edit_bones[i].length();
        let oldlength = old_lengths.get(i).copied().unwrap_or(0.0);
        ea.edit_bones[i].length_stored = length;
        if oldlength == 0.0 {
            let b = &mut ea.edit_bones[i];
            b.head_radius = 0.25 * length;
            b.tail_radius = 0.10 * length;
            b.envelope_distance = 0.25 * length;
            if let Some(p) = b.parent {
                let prad = ea.edit_bones[p].tail_radius;
                let b = &mut ea.edit_bones[i];
                b.head_radius = b.head_radius.min(prad);
            }
        } else {
            let f = length / oldlength;
            let b = &mut ea.edit_bones[i];
            b.envelope_distance *= f;
            b.head_radius *= f;
            b.tail_radius *= f;
            let rh = b.head_radius;
            if let Some(p) = parent {
                ea.edit_bones[p].tail_radius = rh;
            }
        }
    }
    for td in tds.iter().filter(|t| t.fix_roll) {
        let b = &ea.edit_bones[td.bone];
        let up = td.axismtx.z_axis;
        let vec = frap_scene::edit_armature::math::normalize(b.tail_v() - b.head_v()).0;
        let q = rotation_between_vecs_to_quat(td.axismtx.y_axis, vec);
        let up = mul_qt_v3(q, up);
        let roll = roll_to_vector(b, up, false);
        ea.edit_bones[td.bone].roll = td.iroll + angle_wrap_rad(roll - td.iroll);
    }
    if a.use_mirror_x {
        ea.transform_mirror_update_all(a);
    }
}

fn apply(cx: &mut OpCtx, op: TransformOp, args: &OpArgs) -> OpResult {
    let p = TransformParams::from_args(op, args);
    if p.cursor_transform {
        return Err(OpError::NotImplemented(
            "frap: cursor_transform=True in armature Edit Mode is not implemented yet".into(),
        ));
    }
    // Per armature: the new head/tail of each element.
    let (objects, results) = {
        let (c, edit) = cx.split_edit();
        let objects = edit_objects(&c);
        let mut pivot = PivotPoint::from_identifier(&c.tool_settings().transform_pivot_point);
        let collect = |local_origins: bool| {
            let mut tds: Vec<Td> = Vec::new();
            for &(ob, arm) in &objects {
                if let Some(ea) = edit.armatures.get(&arm.key()) {
                    tds.extend(create(&c, ea, ob, arm, op, local_origins));
                }
            }
            tds
        };
        let mut tds = collect(pivot.is_local());
        // `transform_around_single_fallback` (transform_convert.cc): one element transformed
        // about the median, bounds or active element falls back to local origins (a lone
        // selected tip then rotates about its bone's head).
        if tds.len() == 1
            && matches!(
                pivot,
                PivotPoint::BoundingBoxCenter | PivotPoint::MedianPoint | PivotPoint::ActiveElement
            )
            && matches!(op, TransformOp::Rotate | TransformOp::Resize)
            && p.center_override.is_none()
        {
            pivot = PivotPoint::IndividualOrigins;
            tds = collect(true);
        }
        let local_origins = pivot.is_local();
        if tds.is_empty() {
            (objects, None)
        } else {
            let mut src = crate::object::orient_source(&c);
            // LOCAL in armature Edit Mode: the object's axes; NORMAL: the active bone's axes.
            if let Some((ob, arm)) = objects.first()
                && let Some(ea) = edit.armatures.get(&arm.key())
            {
                let obm = Mat3::from_mat4(world_matrix(&c.main.objects, *ob));
                if p.orient_type.as_deref() == Some("NORMAL")
                    && let Some(act) = ea.active.filter(|&i| i < ea.edit_bones.len())
                {
                    src.active_world = Some(normalize_m3(&(obm * ea.bone_mat3(act))));
                }
            }
            let orient = Orientation::from_props(
                p.orient_type.as_deref(),
                p.orient_matrix,
                &p.orient_matrix_type,
                &src,
            );
            // Edit Mode: per-element axes only for NORMAL about individual origins.
            let con = Constraint::from_props(
                p.value_is_set_array(),
                p.constraint_axis,
                &orient,
                tds.len(),
            )
            .map(|mut con| {
                con.space = if orient.kind == OrientType::Normal && local_origins {
                    ConstraintSpace::PerElement
                } else {
                    ConstraintSpace::Orientation
                };
                con
            });
            let world_centers: Vec<Vec3> = tds
                .iter()
                .map(|t| t.obmat.transform_point3(t.center))
                .collect();
            let active_center = objects.first().and_then(|(ob, arm)| {
                let ea = edit.armatures.get(&arm.key())?;
                let b = ea.edit_bones.get(ea.active?)?;
                (b.select[0] || b.select[ROOT])
                    .then(|| world_matrix(&c.main.objects, *ob).transform_point3(b.head_v()))
            });
            let center = pivot::center(
                pivot,
                &world_centers,
                Vec3::from_array(c.cursor().location),
                active_center,
                p.center_override,
            );
            let values = p.value.unwrap_or(Vec3::ZERO);
            let mut out: Vec<(Td, Vec3)> = Vec::with_capacity(tds.len());
            for td in tds {
                let center_local = if pivot.is_local() {
                    td.center
                } else {
                    let im = td.obmat.inverse();
                    im.transform_point3(center)
                };
                let elem_axes = || normalize_m3(&(td.mtx * td.axismtx));
                let new = match op {
                    TransformOp::Rotate => {
                        let axis = match con {
                            Some(con) if con.space == ConstraintSpace::PerElement => {
                                con.rot_axis(&elem_axes())
                            }
                            Some(con) => con.rot_axis(&orient.matrix),
                            None => -orient.matrix.col(p.orient_axis),
                        };
                        let mat = axis_angle_normalized_to_mat3(axis, p.angle());
                        let smat = td.smtx * (mat * td.mtx);
                        smat * (td.iloc - center_local) + center_local
                    }
                    TransformOp::Resize => {
                        let mut mat = size_to_mat3(values);
                        if let Some(con) = con {
                            mat = if con.space == ConstraintSpace::PerElement {
                                con.apply_size_elem(&elem_axes(), mat)
                            } else {
                                con.apply_size(&orient, mat)
                            };
                        }
                        let tmat = td.smtx * (mat * td.mtx);
                        let vec = tmat * (td.iloc - center_local) + center_local - td.iloc;
                        td.iloc + vec
                    }
                    _ => {
                        let mut vec = orient.matrix * values;
                        if let Some(con) = con {
                            vec = if con.space == ConstraintSpace::PerElement {
                                con.apply_vec_elem(&orient, &elem_axes(), vec)
                            } else {
                                con.apply_vec(&orient, vec)
                            };
                        }
                        td.iloc + td.smtx * vec
                    }
                };
                out.push((td, new));
            }
            (objects, Some(out))
        }
    };
    let Some(results) = results else {
        cx.report(
            ReportType::Warning,
            "frap: nothing to transform: no bone head or tail is selected",
        );
        return Ok(OpStatus::Cancelled);
    };
    for &(_, arm) in &objects {
        let Some(a) = cx.main().armatures.get(arm).cloned() else {
            continue;
        };
        let (_, edit) = cx.split_edit();
        let Some(ea) = edit.armatures.get_mut(&arm.key()) else {
            continue;
        };
        let ea = Arc::make_mut(ea);
        let old_lengths: Vec<f32> = ea.edit_bones.iter().map(|b| b.length_stored).collect();
        let mine: Vec<&(Td, Vec3)> = results.iter().filter(|(td, _)| td.arm == arm).collect();
        for (td, new) in &mine {
            let b = &mut ea.edit_bones[td.bone];
            match td.end {
                End::Head => b.head = new.to_array(),
                End::Tail => b.tail = new.to_array(),
            }
        }
        let tds: Vec<Td> = mine.iter().map(|(td, _)| td.clone()).collect();
        recalc(ea, &a, &tds, &old_lengths);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quaternion_rotates_vector() {
        let q = rotation_between_vecs_to_quat(Vec3::Y, Vec3::Z);
        let v = mul_qt_v3(q, Vec3::Y);
        assert!(v.abs_diff_eq(Vec3::Z, 1e-6), "{v:?}");
    }
}
