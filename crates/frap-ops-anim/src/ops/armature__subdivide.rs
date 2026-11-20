// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.subdivide(number_cuts=1)`, tier T0 (WP 6.2). Port of
//! `armature_subdivide_exec` (blender/source/blender/editors/armature/armature_add.cc),
//! Blender 5.2.4, GPL-2.0-or-later (`EditArmature::subdivide`).
//!
//! Each of `context.selected_editable_bones` is cut into `number_cuts + 1` equal, connected
//! bones (copies of it, so selected); its children move to the last piece, radii are
//! interpolated, and the new bones are named `Bone.001`, `Bone.002`, … along the chain.
//! Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::Subdivide) -> OpResult {
    for (_, arm) in common::edit_armatures(cx) {
        common::with_edit(cx, arm, |ea, a| {
            let bones = ea.selected_bones(a, true);
            ea.subdivide(&bones, p.number_cuts.value);
        });
    }
    Ok(OpStatus::Finished)
}
