// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.extrude_move(ARMATURE_OT_extrude={…}, TRANSFORM_OT_translate={…})`, a
//! macro (`ED_operatormacros_armature`, blender/source/blender/editors/armature/armature_ops.cc,
//! Blender 5.2.4, GPL-2.0-or-later): `armature.extrude` then `transform.translate` with their
//! nested arguments. The framework runs the steps (§10.2); this exec is the macro's post-step
//! hook and changes nothing. The translation runs through the armature Edit Mode transform
//! handler (WP 6.2), which gives the new bones radii from their length.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

#[operator(poll = poll::EDITARMATURE, tier = T0, kind = Macro)]
fn exec(_cx: &mut OpCtx, _p: armature::ExtrudeMove) -> OpResult {
    Ok(OpStatus::Finished)
}
