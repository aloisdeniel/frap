// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.fill()`, tier T0 (WP 8.3.10). Port of `armature_fill_bones_exec`
//! (blender/source/blender/editors/armature/armature_edit.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::fill_joints`, `EditArmature::fill`).
//!
//! The selected joints of the visible bones (heads of unconnected bones, tails) decide: one
//! joint adds a bone `Bone` from it to the 3D cursor; two add a bone between them (head and
//! tail from the joint kinds, the active bone or the cursor distance), parented to the bone
//! owning the head joint and connected when a tail joint is involved. The new bone takes the
//! joint owners' bone collections and becomes active with only its tail selected. ERRORs: `No
//! joints selected`, `Bones for different objects selected`, `Same bone selected...`, `Too
//! many points selected: N`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{ArmatureId, IdRef, ObjectId};
use frap_scene::edit_armature::chains::JointPoint;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_anim::armature;
use glam::Vec3;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0, spec = armature::OPSPEC_ARMATURE_OT_FILL)]
fn exec(cx: &mut OpCtx, _p: armature::Fill) -> OpResult {
    let mut count = 0usize;
    let mut chosen: Option<(IdRef<ObjectId>, IdRef<ArmatureId>, Vec<JointPoint>)> = None;
    let mut mixed = false;
    for (ob, arm) in common::edit_armatures(cx) {
        let Some((a, ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        let mut points = Vec::new();
        if ea.fill_joints(&a, &mut points) {
            if chosen.as_ref().is_some_and(|(_, c, _)| *c != arm) {
                mixed = true;
            }
            count += points.len();
            chosen = Some((ob, arm, points));
        } else {
            count += points.len();
        }
    }
    if count == 0 {
        return Err(OpError::Runtime("No joints selected".into()));
    }
    if mixed {
        return Err(OpError::Runtime("Bones for different objects selected".into()));
    }
    let Some((ob, arm, points)) = chosen else {
        return Err(OpError::Runtime("No joints selected".into()));
    };
    if points.len() > 2 {
        return Err(OpError::Runtime(format!(
            "Too many points selected: {}",
            points.len()
        )));
    }
    // The cursor in the edit object's space (`world_to_object * cursor`).
    let obmat = world_matrix_in(cx.main(), ob);
    let cursor = Vec3::from_array(cx.ctx().cursor().location);
    let curs = frap_scene::edit_armature::math::invert_m4(&obmat).transform_point3(cursor);
    let r = common::with_edit(cx, arm, |ea, _| ea.fill(&points, curs));
    match r {
        Some(Err(msg)) => Err(OpError::Runtime(msg)),
        _ => Ok(OpStatus::Finished),
    }
}
