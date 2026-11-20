// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.roll_clear(roll=0)`, tier T0 (WP 6.2). Port of `armature_roll_clear_exec`
//! (blender/source/blender/editors/armature/armature_edit.cc), Blender 5.2.4,
//! GPL-2.0-or-later: every visible, selected, unlocked edit bone gets `roll`; with
//! `use_mirror_x` the unselected mirror bones get `-roll`. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::RollClear) -> OpResult {
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.roll_clear(a, p.roll.value));
    }
    Ok(OpStatus::Finished)
}
