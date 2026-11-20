// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.extrude(forked=False)`, tier T0 (WP 6.2). Port of
//! `armature_extrude_exec` (blender/source/blender/editors/armature/armature_add.cc), Blender
//! 5.2.4, GPL-2.0-or-later (`EditArmature::extrude`).
//!
//! Every visible bone with a selected tail (or body) gets a new zero-length child at its tail,
//! connected, named after it (`Bone.001`); a bone with only its head selected (and no selected
//! parent tip) gets a new sibling at its head. The new bones take the envelope settings, the
//! tail radius and the bone collections of their source; only their tails are selected, the
//! sources are deselected; the new bone is active when there is one, else the last one. With
//! `use_mirror_x` the mirror bones are extruded too (`forked` makes `_L`/`_R` pairs when a bone
//! has no mirror). `CANCELLED` when nothing was extruded.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::Extrude) -> OpResult {
    let mut changed = false;
    for (_, arm) in common::edit_armatures(cx) {
        let (a, mut ea) = match common::snapshot(cx, arm) {
            Some(x) => x,
            None => continue,
        };
        let n = ea.extrude(&a, p.forked.value);
        // The root-selection fix-up runs even when nothing is extruded.
        common::store(cx, arm, ea);
        changed |= n > 0;
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
