// SPDX-License-Identifier: GPL-3.0-or-later
//! Object-mode snapping shared by the `view3d.snap_*` operators. Ports of
//! `snap_curs_to_sel_ex`, `snap_calc_active_center` / `ED_object_calc_active_center`,
//! `snap_selected_to_location` and the object part of `snap_sel_to_grid_exec`
//! (`editors/space_view3d/view3d_snap.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! The poll of these operators needs a 3D View (`ED_operator_view3d_active`); frap always has
//! its virtual one (D-10), so it passes. Edit Mode (M2) and Pose Mode (M6) snapping arrive
//! with those modes.
//!
//! Verified on 5.2.4: selected objects are moved through their parent space, children of
//! moved parents included (each with the world matrix evaluated before the operator, so a
//! selected child of a selected parent ends up offset twice); `use_offset` keeps the
//! selection's shape around its center (median, bounds or active per the pivot point);
//! `use_rotation` turns the offsets by the cursor rotation and gives every object the
//! cursor's rotation in world space (delta rotations ignored, no compatible Euler).

use frap_math::euler::mat3_to_eul_o;
use frap_math::matrix::RotationMode;
use frap_math::quat::{mat3_to_quat, quat_to_axis_angle};
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_scene::{Context, ContextMode};
use glam::{Mat3, Vec3};

use crate::support::xform::Worlds;

/// Refuses snapping outside Object Mode (Edit Mode M2, Pose Mode M6).
pub fn object_mode_only(c: &Context, idname: &str) -> Result<(), OpError> {
    match c.mode() {
        ContextMode::Object => Ok(()),
        m => Err(OpError::NotImplemented(format!(
            "frap: {idname} in {} mode is not implemented yet",
            m.as_str()
        ))),
    }
}

/// `snap_curs_to_sel_ex` (object mode): the median (or, with the `BOUNDING_BOX_CENTER` pivot,
/// the bounds center) of the selected visible objects' world locations; `None` without one.
pub fn selection_center(c: &Context, worlds: &Worlds) -> Option<Vec3> {
    let locs: Vec<Vec3> = c
        .selected_objects()
        .into_iter()
        .map(|o| worlds.get(o).w_axis.truncate())
        .collect();
    let first = *locs.first()?;
    if c.tool_settings().transform_pivot_point == "BOUNDING_BOX_CENTER" {
        let (mut min, mut max) = (first, first);
        for l in &locs[1..] {
            min = min.min(*l);
            max = max.max(*l);
        }
        // `interp_v3_v3v3(r, min, max, 0.5)`
        Some(min * 0.5 + max * 0.5)
    } else {
        let mut c = Vec3::ZERO;
        for l in &locs {
            c += *l;
        }
        Some(c * (1.0 / locs.len() as f32))
    }
}

/// `snap_calc_active_center`: the active object's world location (`select_only`: only when it
/// is selected).
pub fn active_center(c: &Context, worlds: &Worlds, select_only: bool) -> Option<Vec3> {
    let a = c.active_object()?;
    c.main.objects.get(a)?;
    if select_only
        && !c
            .view_layer()
            .bases
            .iter()
            .any(|b| b.object == a && b.selected())
    {
        return None;
    }
    Some(worlds.get(a).w_axis.truncate())
}

/// One object's new channels from `snap_selected_to_location`.
struct Move {
    ob: IdRef<ObjectId>,
    location: [f32; 3],
    rotation: Option<Mat3>,
}

/// `snap_selected_to_location` (object mode): moves the selected editable objects to `target`
/// (with `use_offset`, keeping their offsets from the selection center), optionally turning the
/// offsets and the objects to `rotation` (the cursor's).
pub fn selected_to_location(
    cx: &mut OpCtx,
    target: Vec3,
    use_offset: bool,
    rotation: Option<Mat3>,
) -> OpResult {
    let moves = {
        let c = cx.ctx();
        let worlds = Worlds::of(c.main);
        let center = if use_offset {
            let active_pivot = c.tool_settings().transform_pivot_point == "ACTIVE_ELEMENT";
            let act = active_pivot
                .then(|| active_center(&c, &worlds, true))
                .flatten();
            act.or_else(|| selection_center(&c, &worlds))
                .unwrap_or(Vec3::ZERO)
        } else {
            Vec3::ZERO
        };
        let mut moves = Vec::new();
        for ob in c.selected_editable_objects() {
            let Some(o) = c.main.objects.get(ob) else {
                continue;
            };
            let world = worlds.get(ob);
            let wloc = world.w_axis.truncate();
            let mut dest = if use_offset {
                match rotation {
                    Some(r) => target + r * (wloc - center),
                    None => wloc + (target - center),
                }
            } else {
                target
            };
            dest -= wloc;
            let mut originmat_inv = Mat3::IDENTITY;
            if let Some(p) = o.parent.filter(|p| c.main.objects.contains(*p)) {
                let pm = worlds
                    .parent_matrix(c.main, o)
                    .unwrap_or_else(|| worlds.get(p));
                let originmat = Mat3::from_mat4(pm * o.matrix_parent_inverse);
                originmat_inv = originmat.inverse();
                dest = originmat_inv * dest;
            }
            let rot = rotation.map(|r| originmat_inv * r);
            moves.push(Move {
                ob,
                location: (Vec3::from_array(o.location) + dest).to_array(),
                rotation: rot,
            });
        }
        moves
    };
    for m in moves {
        let o = cx.object_mut(m.ob)?;
        o.location = m.location;
        if let Some(r) = m.rotation {
            match o.rotation_mode_enum() {
                RotationMode::Quaternion => o.rotation_quaternion = mat3_to_quat(&r),
                RotationMode::AxisAngle => {
                    let (axis, angle) = quat_to_axis_angle(mat3_to_quat(&r));
                    o.rotation_axis_angle = [angle, axis[0], axis[1], axis[2]];
                }
                RotationMode::Euler(order) => o.rotation_euler = mat3_to_eul_o(&r, order),
            }
        }
    }
    Ok(OpStatus::Finished)
}

/// The object part of `snap_sel_to_grid_exec`: each selected editable object's world location
/// rounded to the grid, applied through its parent space.
pub fn selected_to_grid(cx: &mut OpCtx, gridf: f32) -> OpResult {
    let moves: Vec<(IdRef<ObjectId>, [f32; 3])> = {
        let c = cx.ctx();
        let worlds = Worlds::of(c.main);
        c.selected_editable_objects()
            .into_iter()
            .filter_map(|ob| {
                let o = c.main.objects.get(ob)?;
                let wloc = worlds.get(ob).w_axis.truncate();
                let snapped =
                    Vec3::from_array(crate::support::cursor::snap_to_grid(wloc.to_array(), gridf));
                let mut vec = snapped - wloc;
                if let Some(p) = o.parent.filter(|p| c.main.objects.contains(*p)) {
                    let pm = worlds
                        .parent_matrix(c.main, o)
                        .unwrap_or_else(|| worlds.get(p));
                    let originmat = Mat3::from_mat4(pm * o.matrix_parent_inverse);
                    vec = originmat.inverse() * vec;
                }
                Some((ob, (Vec3::from_array(o.location) + vec).to_array()))
            })
            .collect()
    };
    for (ob, l) in moves {
        cx.object_mut(ob)?.location = l;
    }
    Ok(OpStatus::Finished)
}
