// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.switch_direction()`, tier T0 (WP 6.2). Port of `chains_find_tips` and
//! `armature_switch_direction_exec` (blender/source/blender/editors/armature/armature_edit.cc),
//! Blender 5.2.4, GPL-2.0-or-later (`EditArmature::switch_direction`).
//!
//! Along each bone chain (from its tip), the selected, unlocked bones swap head and tail and
//! the chain's parent links are reversed (connected where the ends meet); an unselected bone
//! whose parent was flipped is unparented. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: armature::SwitchDirection) -> OpResult {
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.switch_direction(a));
    }
    Ok(OpStatus::Finished)
}
