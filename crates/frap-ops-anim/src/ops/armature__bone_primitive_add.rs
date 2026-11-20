// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.bone_primitive_add(name='Bone', space='OBJECT', align='UP', length=1,
//! use_deform=True)`, tier T0 (WP 6.2). Port of `armature_bone_primitive_add_exec`
//! (blender/source/blender/editors/armature/armature_add.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On the edit object's armature: every bone is deselected, a new bone (unique name, active
//! collection, tail selected) starts at the 3D cursor (object space) and points along:
//! `UP` + `OBJECT` the object's +Z, `UP` + `WORLD` the world +Z (rolled so its Z axis faces
//! world -Y), `AXES` + `OBJECT` the object's +Y, `AXES` + `WORLD` the world +Y, `3D_CURSOR` the
//! cursor's axes, `3D_VIEW` the virtual view's; `length` scales it and its display/envelope
//! sizes. The new bone is **not** made active (Blender does not). A bone added to a hidden
//! collection reports a WARNING.

use std::sync::Arc;

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::tools::roll_to_vector;
use frap_scene::id::transform::world_matrix;
use frap_spec_ops_anim::armature::{
    self,
    bone_primitive_add::{Align, Space},
};
use glam::{Mat3, Vec3};

use super::armature__select_all::common;

/// `BKE_scene_cursor_rot_to_mat3` (`View3DCursor::matrix<float3x3>()`): the cursor rotation
/// in its rotation mode (the quaternion used as stored).
fn cursor_rot_mat3(cur: &frap_scene::generated::plain::View3DCursor) -> Mat3 {
    use frap_math::matrix::RotationMode;
    match RotationMode::from_blender(&cur.rotation_mode) {
        Some(RotationMode::AxisAngle) => {
            let a = cur.rotation_axis_angle;
            frap_math::quat::axis_angle_to_mat3([a[1], a[2], a[3]], a[0])
        }
        Some(RotationMode::Quaternion) => frap_math::quat::quat_to_mat3(cur.rotation_quaternion),
        Some(RotationMode::Euler(order)) => frap_math::euler::eul_o_to_mat3(cur.rotation_euler, order),
        None => frap_math::euler::eul_o_to_mat3(cur.rotation_euler, frap_math::EulerOrder::XYZ),
    }
}

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::BonePrimitiveAdd) -> OpResult {
    let Some((ob, arm)) = common::edit_armatures(cx).into_iter().next() else {
        return Ok(OpStatus::Cancelled);
    };
    let obmat = world_matrix(&cx.main().objects, ob);
    let iobmat = if obmat.determinant() != 0.0 {
        obmat.inverse()
    } else {
        glam::Mat4::IDENTITY
    };
    let imat = Mat3::from_mat4(iobmat);
    let (cursor_mat, cursor_loc, view_inv) = {
        let c = cx.ctx();
        let cur = c.cursor();
        let m = cursor_rot_mat3(cur);
        let view = Mat3::from_mat4(c.win.view3d.view_matrix);
        let vinv = if view.determinant() != 0.0 { view.inverse() } else { Mat3::IDENTITY };
        (m, Vec3::from_array(cur.location), vinv)
    };
    let align = p.align.value;
    let space = p.space.value;
    let (orient, roll_vector) = match align {
        Align::V3dView => {
            let m = imat * view_inv;
            (m, m.z_axis)
        }
        Align::V3dCursor => {
            let m = imat * cursor_mat;
            (m, m.z_axis)
        }
        Align::Axes => match space {
            Space::World => (imat, imat.z_axis),
            Space::Object => (
                Mat3::from_cols(Vec3::X, -Vec3::Z, Vec3::Y),
                Vec3::ZERO,
            ),
        },
        Align::Up => match space {
            Space::World => (
                imat * Mat3::from_cols(Vec3::X, Vec3::Z, -Vec3::Y),
                -imat.y_axis,
            ),
            Space::Object => (Mat3::IDENTITY, Vec3::ZERO),
        },
    };
    let length = p.length.value;
    let curs_local = iobmat.transform_point3(cursor_loc);
    let name = p.name.value.to_string();
    let use_deform = p.use_deform.value;
    let needs_roll = matches!(align, Align::V3dCursor | Align::V3dView) || space == Space::World;
    let (hidden_collection, solo_without) = {
        let Some((a, mut ea)) = common::snapshot(cx, arm) else {
            return Ok(OpStatus::Cancelled);
        };
        ea.deselect_all();
        let i = ea.add_bone(&name);
        if let Some(c) = a.active_collection.and_then(|c| a.collections.get(c)) {
            ea.edit_bones[i].collections.push(c.name.clone());
        }
        let b = &mut ea.edit_bones[i];
        b.head_radius = 0.1 * length;
        b.tail_radius = 0.05 * length;
        b.envelope_distance = 0.25 * length;
        b.head = curs_local.to_array();
        b.tail = (curs_local + orient * Vec3::new(0.0, 0.0, length)).to_array();
        if needs_roll {
            let dir = frap_scene::edit_armature::math::normalize(orient.y_axis).0 * length;
            b.tail = (curs_local + dir).to_array();
            let rv = frap_scene::edit_armature::math::normalize(roll_vector).0;
            b.roll = roll_to_vector(b, rv, false);
        }
        if !use_deform {
            b.use_deform = false;
        }
        let solo_without = b.collections.is_empty() && a.collections.iter().any(|c| c.is_solo);
        let hidden = (!solo_without && !ea.is_visible(&a, i))
            .then(|| ea.edit_bones[i].collections.first().cloned())
            .flatten();
        let (_, edit) = cx.split_edit();
        if let Some(slot) = edit.armatures.get_mut(&arm.key()) {
            *slot = Arc::new(ea);
        }
        (hidden, solo_without)
    };
    cx.main_mut().1.modified.insert(arm.any());
    if solo_without {
        cx.report(
            ReportType::Warning,
            "Bone not added to a collection and hidden because solo bone collection(s) exist.",
        );
    } else if let Some(c) = hidden_collection {
        cx.report(
            ReportType::Warning,
            format!("Bone was added to a hidden collection '{c}'"),
        );
    }
    Ok(OpStatus::Finished)
}
