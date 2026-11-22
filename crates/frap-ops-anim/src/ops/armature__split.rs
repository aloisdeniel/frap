// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.split()`, tier T0 (WP 8.3.10). Port of `armature_split_exec`
//! (blender/source/blender/editors/armature/armature_edit.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::split`).
//!
//! On every armature in Edit Mode, a bone whose body selection differs from its parent's
//! loses its parent (and connection), splitting the selected bones off their unselected
//! neighbours; every bone's head and tail selection is then set from its body selection.
//! Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SPLIT
)]
fn exec(cx: &mut OpCtx, _p: armature::ArmatureSplit) -> OpResult {
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, _| ea.split());
    }
    Ok(OpStatus::Finished)
}
