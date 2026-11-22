// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.select_mirror(only_active=False, extend=False)`, tier T0 (WP 8.3.10).
//! Port of `armature_select_mirror_exec`
//! (blender/source/blender/editors/armature/armature_select.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::select_mirror`).
//!
//! On every armature in Edit Mode, each selectable bone takes the head/body/tail selection of
//! its visible X mirror (the bone with the flipped side name), added to its own with `extend`;
//! a bone without a visible mirror is deselected (kept with `extend`); `only_active` limits
//! the mirrored pairs to the active bone and its mirror. The active bone moves to its mirror.
//! Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SELECT_MIRROR
)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureSelectMirror) -> OpResult {
    let (only_active, extend) = (p.only_active.value, p.extend.value);
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.select_mirror(a, only_active, extend));
    }
    Ok(OpStatus::Finished)
}
