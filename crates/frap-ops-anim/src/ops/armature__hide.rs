// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.hide(unselected=False)`, tier T0 (WP 8.3.10). Port of
//! `armature_hide_exec` (blender/source/blender/editors/armature/armature_edit.cc), Blender
//! 5.2.4, GPL-2.0-or-later (`EditArmature::hide`).
//!
//! On every armature in Edit Mode, the visible selected bones (the unselected ones with
//! `unselected`) get `EditBone.hide` and lose their selection. `CANCELLED` when no bone is
//! selected (`context.selected_bones` empty), otherwise `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_HIDE
)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureHide) -> OpResult {
    if common::selected_count(cx) == 0 {
        return Ok(OpStatus::Cancelled);
    }
    let unselected = p.unselected.value;
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.hide(a, unselected));
    }
    Ok(OpStatus::Finished)
}
