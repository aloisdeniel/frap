// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.collection_remove()`, tier T0 (WP 8.3.10). Port of
//! `bone_collection_remove_exec` and `active_bone_collection_poll`
//! (blender/source/blender/editors/armature/bone_collections.cc) and
//! `ANIM_armature_bonecoll_remove` (blender/source/blender/animrig/intern/bone_collections.cc),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Removes the active bone collection of the active armature (in any mode; the poll fails with
//! `Armature has no active bone collection, select one first` without one): its child
//! collections take its place under its parent, its bones (rest and edit bones) leave it, and
//! the next sibling becomes active (else the previous one, else the parent). Always
//! `FINISHED`.

use std::sync::Arc;

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__collection_assign::bcoll;

#[operator(
    poll = bcoll::ACTIVE_POLL,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_COLLECTION_REMOVE
)]
fn exec(cx: &mut OpCtx, _p: armature::CollectionRemove) -> OpResult {
    let Some((_, arm)) = bcoll::armature_context(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some((_, mut arr)) = bcoll::array(cx, arm) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(active) = arr.active() else {
        return Ok(OpStatus::Cancelled);
    };
    let name = arr.remove(active);
    bcoll::store_array(cx, arm, &arr);
    // `bonecoll_unassign_and_free`: rest bones and edit bones leave the collection.
    let (main, log) = cx.main_mut();
    if main
        .armatures
        .get(arm)
        .is_some_and(|a| a.bones.iter().any(|b| b.collections.contains(&name)))
        && let Some(a) = main.armatures.get_mut(arm, log)
    {
        for b in &mut a.bones {
            b.collections.retain(|c| *c != name);
        }
    }
    let edited = {
        let (_, edit) = cx.split_edit();
        match edit.armatures.get_mut(&arm.key()) {
            Some(ea) if ea.edit_bones.iter().any(|b| b.collections.contains(&name)) => {
                for b in &mut Arc::make_mut(ea).edit_bones {
                    b.collections.retain(|c| *c != name);
                }
                true
            }
            _ => false,
        }
    };
    if edited {
        cx.main_mut().1.modified.insert(arm.any());
    }
    Ok(OpStatus::Finished)
}
