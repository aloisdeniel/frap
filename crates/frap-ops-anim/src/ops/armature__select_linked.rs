// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.select_linked(all_forks=False)`, tier T0 (WP 8.3.10). Port of
//! `armature_select_linked_exec` and `armature_select_linked_impl`
//! (blender/source/blender/editors/armature/armature_select.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::select_linked`).
//!
//! On every armature in Edit Mode, starting from the visible bones with any selection flag,
//! selects the chain of connected parents (up to a bone whose parent is unselectable or not
//! connected) and the connected children leading back to it; `all_forks` also selects the
//! other connected branches of those parents. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SELECT_LINKED
)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureSelectLinked) -> OpResult {
    let all_forks = p.all_forks.value;
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.select_linked(a, all_forks));
    }
    Ok(OpStatus::Finished)
}
