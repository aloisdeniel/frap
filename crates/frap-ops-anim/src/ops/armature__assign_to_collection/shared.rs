// SPDX-License-Identifier: GPL-3.0-or-later
//! `add_or_move_to_collection_exec` and `add_or_move_to_collection_bcoll`
//! (blender/source/blender/editors/armature/bone_collections.cc), Blender 5.2.4,
//! GPL-2.0-or-later: the body of `armature.assign_to_collection` and
//! `armature.move_to_collection` (WP 8.3.10).

use frap_op::prelude::*;
use smol_str::SmolStr;

use super::super::armature__collection_assign::bcoll;

/// `add_or_move_to_collection_exec(C, op, f, f)`.
pub(crate) fn add_or_move(
    cx: &mut OpCtx,
    collection_index: i32,
    new_name: &str,
    new_name_set: bool,
    f: fn(&mut Vec<SmolStr>, &SmolStr) -> bool,
) -> OpResult {
    // `ED_object_context` (`ED_pose_object_from_context` in Pose Mode: the active object too).
    let Some((ob, arm)) = bcoll::active_armature(cx) else {
        return Err(OpError::Runtime("No object found to operate on".into()));
    };
    let Some((a, mut arr)) = bcoll::array(cx, arm) else {
        return Ok(OpStatus::Cancelled);
    };
    let index_exceeds = collection_index >= 0 && collection_index as usize >= arr.len();
    let target = if (new_name_set && !index_exceeds) || collection_index < 0 {
        let i = arr.new_collection(new_name, i64::from(collection_index));
        arr.active_set(Some(i));
        bcoll::store_array(cx, arm, &arr);
        arr.name(i).clone()
    } else if index_exceeds {
        return Err(OpError::Runtime(format!(
            "Bone collection with index {collection_index} not found on Armature {}",
            a.header.name
        )));
    } else {
        arr.name(collection_index as usize).clone()
    };
    let r = bcoll::assign_mode_specific(cx, ob, arm, f, &target);
    if !r.supported {
        return Err(bcoll::mode_error());
    }
    if !r.had_bones {
        cx.report(
            ReportType::Warning,
            "No bones selected, nothing to assign to bone collection",
        );
        return Ok(OpStatus::Cancelled);
    }
    if !r.changed {
        cx.report(
            ReportType::Warning,
            "All selected bones were already part of this collection",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
