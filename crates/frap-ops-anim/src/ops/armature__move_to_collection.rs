// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.move_to_collection(collection_index=-1, new_collection_name='')`, tier T0
//! (WP 8.3.10). Port of `move_to_collection_exec` (`add_or_move_to_collection_exec` with
//! `ANIM_armature_bonecoll_assign_and_move`,
//! blender/source/blender/editors/armature/bone_collections.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Like `armature.assign_to_collection`, but each selected bone first leaves every bone
//! collection, so it ends up in the target collection only; this always counts as a change.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__assign_to_collection::shared;
use super::armature__collection_assign::bcoll;

#[operator(
    poll = bcoll::MOVE_TO_POLL,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_MOVE_TO_COLLECTION
)]
fn exec(cx: &mut OpCtx, p: armature::MoveToCollection) -> OpResult {
    shared::add_or_move(
        cx,
        p.collection_index.value,
        &p.new_collection_name.value,
        p.new_collection_name.is_set,
        frap_scene::edit_armature::bone_collections::assign_and_move,
    )
}
