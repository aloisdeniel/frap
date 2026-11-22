// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.select_more()`, tier T0 (WP 8.3.10). Port of
//! `armature_de_select_more_exec` and `armature_select_more_less`
//! (blender/source/blender/editors/armature/armature_select.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::select_more_less`).
//!
//! On every armature in Edit Mode, each visible bone with a selected head or tail becomes
//! fully selected, and selection spreads one bone along connected joints (to the parent of a
//! bone with a selected head, to the child of a bone with a selected tail). Unselectable bones
//! are skipped. Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SELECT_MORE
)]
fn exec(cx: &mut OpCtx, _p: armature::SelectMore) -> OpResult {
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.select_more_less(a, true));
    }
    Ok(OpStatus::Finished)
}
