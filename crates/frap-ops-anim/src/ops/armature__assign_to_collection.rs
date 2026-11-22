// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.assign_to_collection(collection_index=-1, new_collection_name='')`, tier
//! T0 (WP 8.3.10). Port of `assign_to_collection_exec`, `add_or_move_to_collection_exec`,
//! `add_or_move_to_collection_bcoll` and `move_to_collection_poll`
//! (blender/source/blender/editors/armature/bone_collections.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Adds the selected bones (Pose Mode: selected visible pose bones; armature Edit Mode:
//! selected, unlocked, visible edit bones) of the active armature to the bone collection at
//! `collection_index` (`collections_all` order). With `new_collection_name` set (and an index
//! in range), or with a negative index, a new collection is created first, named
//! `new_collection_name` (`Bones` when empty), as a child of `collection_index` (a root for
//! -1), and made active. An index out of range is an ERROR `Bone collection with index N not
//! found on Armature A`; another mode an ERROR (the new collection stays); without selected
//! bones, or when all were members already, a WARNING and `CANCELLED`.

pub(crate) mod shared;

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__collection_assign::bcoll;

#[operator(
    poll = bcoll::ASSIGN_TO_POLL,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_ASSIGN_TO_COLLECTION
)]
fn exec(cx: &mut OpCtx, p: armature::AssignToCollection) -> OpResult {
    shared::add_or_move(
        cx,
        p.collection_index.value,
        &p.new_collection_name.value,
        p.new_collection_name.is_set,
        frap_scene::edit_armature::bone_collections::assign,
    )
}
