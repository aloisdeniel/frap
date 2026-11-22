// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.collection_add()`, tier T0 (WP 8.3.10). Port of
//! `bone_collection_add_exec` and `bone_collection_add_poll`
//! (blender/source/blender/editors/armature/bone_collections.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Adds a bone collection `Bones` (`Bones.001`, … when taken) to the active armature, in any
//! mode: a sibling of the active collection placed right after it (a root at the end of the
//! roots without one), and makes it active. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__collection_assign::bcoll;

#[operator(
    poll = bcoll::ADD_POLL,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_COLLECTION_ADD
)]
fn exec(cx: &mut OpCtx, _p: armature::CollectionAdd) -> OpResult {
    let Some((_, arm)) = bcoll::armature_context(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some((_, mut arr)) = bcoll::array(cx, arm) else {
        return Ok(OpStatus::Cancelled);
    };
    // A sibling of the active collection.
    let parent = arr.find_parent_index(arr.active().map_or(-1, |a| a as i64));
    let mut new = arr.new_collection("", parent);
    if let Some(a) = arr.active() {
        let n = arr.child_number_find(a);
        let to = arr.child_number_set(new, n + 1);
        if to >= 0 {
            new = to as usize;
        }
    }
    arr.active_set(Some(new));
    bcoll::store_array(cx, arm, &arr);
    Ok(OpStatus::Finished)
}
