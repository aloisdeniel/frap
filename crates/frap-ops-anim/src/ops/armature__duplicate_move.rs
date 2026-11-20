// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.duplicate_move(ARMATURE_OT_duplicate={…}, TRANSFORM_OT_translate={…})`, a
//! macro (`ED_operatormacros_armature`, blender/source/blender/editors/armature/armature_ops.cc,
//! Blender 5.2.4, GPL-2.0-or-later): `armature.duplicate` then `transform.translate` with their
//! nested arguments. The framework runs the steps (§10.2); this exec is the macro's post-step
//! hook and changes nothing.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

#[operator(poll = poll::EDITARMATURE, tier = T0, kind = Macro, spec = armature::OPSPEC_ARMATURE_OT_DUPLICATE_MOVE)]
fn exec(_cx: &mut OpCtx, _p: armature::ArmatureDuplicateMove) -> OpResult {
    Ok(OpStatus::Finished)
}
