// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.reveal(select=True)`, tier T0 (WP 8.3.10). Port of
//! `armature_reveal_exec` (blender/source/blender/editors/armature/armature_edit.cc), Blender
//! 5.2.4, GPL-2.0-or-later (`EditArmature::reveal`).
//!
//! On every armature in Edit Mode, the hidden bones in visible bone collections are shown and
//! (unless `hide_select`) get their head, body and tail selection set to `select`. Always
//! `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_REVEAL
)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureReveal) -> OpResult {
    let select = p.select.value;
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.reveal(a, select));
    }
    Ok(OpStatus::Finished)
}
