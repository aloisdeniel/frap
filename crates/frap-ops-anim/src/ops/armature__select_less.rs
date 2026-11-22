// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.select_less()`, tier T0 (WP 8.3.10). Port of
//! `armature_de_select_less_exec` and `armature_select_more_less`
//! (blender/source/blender/editors/armature/armature_select.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::select_more_less`).
//!
//! On every armature in Edit Mode, each visible bone without both joints selected is
//! deselected, and across connected joints a bone whose neighbour was not selected loses the
//! shared joint; the remaining selected bones are then fully selected again. Always
//! `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SELECT_LESS
)]
fn exec(cx: &mut OpCtx, _p: armature::SelectLess) -> OpResult {
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| ea.select_more_less(a, false));
    }
    Ok(OpStatus::Finished)
}
