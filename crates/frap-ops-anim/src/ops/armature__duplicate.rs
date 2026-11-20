// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.duplicate(do_flip_names=False)`, tier T0 (WP 6.2). Port of
//! `armature_duplicate_selected` and `copy_pchan`
//! (blender/source/blender/editors/armature/armature_add.cc), Blender 5.2.4, GPL-2.0-or-later
//! (`EditArmature::duplicate_selected`).
//!
//! Each selected, visible bone is copied (appended, unique name `Bone.001`, or the flipped
//! side name with `do_flip_names` when free); a copy's parent is the copy of its parent, or
//! its original parent (disconnected) when that was not copied. The copies keep the
//! selection, the originals are deselected, the active bone moves to its copy. The pose
//! channel of a copy takes the source channel's locks, rotation mode and custom shape.
//! `CANCELLED` without a selected bone.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0, spec = armature::OPSPEC_ARMATURE_OT_DUPLICATE)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureDuplicate) -> OpResult {
    if common::selected_count(cx) == 0 {
        return Ok(OpStatus::Cancelled);
    }
    for (ob, arm) in common::edit_armatures(cx) {
        let Some((a, mut ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        let pairs = ea.duplicate_selected(&a, p.do_flip_names.value);
        let names: Vec<(String, String)> = pairs
            .iter()
            .map(|&(s, d)| {
                (
                    ea.edit_bones[s].name.to_string(),
                    ea.edit_bones[d].name.to_string(),
                )
            })
            .collect();
        common::store(cx, arm, ea);
        for (s, d) in names {
            common::copy_pchan(cx, ob, &s, &d)?;
        }
    }
    Ok(OpStatus::Finished)
}
