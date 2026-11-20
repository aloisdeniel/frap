// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.calculate_roll(type='POS_X', axis_flip=False, axis_only=False)`, tier T0
//! (WP 6.2). Port of `armature_calc_roll_exec` and `ED_armature_ebone_roll_to_vector`
//! (blender/source/blender/editors/armature/armature_edit.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::calc_roll`).
//!
//! The selected, unlocked bones get the roll that points their Z axis along: the tangent of
//! the chain (`POS_X`/`POS_Z`/`NEG_X`/`NEG_Z`: from the bone and its parent, a parentless
//! parent too), a global axis (`GLOBAL_*`, in object space), the active bone's Z axis
//! (`ACTIVE`; `No active bone set` without one), the view's Z axis (`VIEW`, the virtual 3D
//! View) or the 3D cursor (`CURSOR`). Negative types and `axis_flip` negate the axis;
//! `axis_only` takes the shorter turn. With `use_mirror_x` unselected mirror bones get the
//! negated roll.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix;
use frap_spec_ops_anim::armature;
use glam::{Mat3, Vec3};

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::CalculateRoll) -> OpResult {
    let ty = p.r#type.value.identifier();
    let arms = common::edit_armatures(cx);
    // The active edit bone's Z axis (`ACTIVE`), from the edit object's armature.
    let active_z = arms.first().and_then(|&(_, arm)| {
        let (_, ea) = common::snapshot(cx, arm)?;
        let i = ea.active.filter(|&i| i < ea.edit_bones.len())?;
        Some(ea.bone_mat3(i).z_axis)
    });
    let (view_z, cursor) = {
        let c = cx.ctx();
        let view = Mat3::from_mat4(c.win.view3d.view_matrix);
        let viewinv = if view.determinant() != 0.0 { view.inverse() } else { Mat3::IDENTITY };
        (viewinv.z_axis, Vec3::from_array(c.cursor().location))
    };
    for (ob, arm) in arms {
        let obmat = world_matrix(&cx.main().objects, ob);
        let ob3 = Mat3::from_mat4(obmat);
        let cursor_local = if obmat.determinant() != 0.0 {
            obmat.inverse().transform_point3(cursor)
        } else {
            cursor
        };
        let r = common::with_edit(cx, arm, |ea, a| {
            ea.calc_roll(
                a,
                ty,
                p.axis_flip.value,
                p.axis_only.value,
                &ob3,
                cursor_local,
                view_z,
                active_z,
            )
        });
        if let Some(Err(msg)) = r {
            cx.report(ReportType::Error, msg);
            return Ok(OpStatus::Cancelled);
        }
    }
    Ok(OpStatus::Finished)
}
