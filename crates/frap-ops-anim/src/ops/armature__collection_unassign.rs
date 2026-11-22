// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.collection_unassign(name='')`, tier T0 (WP 8.3.10). Port of
//! `bone_collection_unassign_exec` (blender/source/blender/editors/armature/bone_collections.cc),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Removes the selected bones of the active armature (Pose Mode: selected visible pose bones;
//! Edit Mode: selected, unlocked, visible edit bones) from the bone collection `name` (the
//! active collection when empty; a missing name is an ERROR `No bone collection named '…'`).
//! Another mode is an ERROR; without selected bones (WARNING `No bones selected, nothing to
//! unassign from bone collection`) or when none was a member (WARNING `None of the selected
//! bones were assigned to this collection`) it is `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__collection_assign::bcoll;

#[operator(
    poll = bcoll::UNASSIGN_POLL,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_COLLECTION_UNASSIGN
)]
fn exec(cx: &mut OpCtx, p: armature::CollectionUnassign) -> OpResult {
    let Some((ob, arm)) = bcoll::active_armature(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(name) = bcoll::named_or_active(cx, arm, &p.name.value)? else {
        return Ok(OpStatus::Cancelled);
    };
    let r = bcoll::assign_mode_specific(
        cx,
        ob,
        arm,
        frap_scene::edit_armature::bone_collections::unassign,
        &name,
    );
    if !r.supported {
        return Err(bcoll::mode_error());
    }
    if !r.had_bones {
        cx.report(
            ReportType::Warning,
            "No bones selected, nothing to unassign from bone collection",
        );
        return Ok(OpStatus::Cancelled);
    }
    if !r.changed {
        cx.report(
            ReportType::Warning,
            "None of the selected bones were assigned to this collection",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
