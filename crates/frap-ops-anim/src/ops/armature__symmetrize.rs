// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.symmetrize(direction='NEGATIVE_X', copy_bone_colors=False)`, tier T0
//! (WP 6.2). Port of `armature_symmetrize_exec`, `mirror_bone_collection_assignments` and
//! `copy_pchan` (blender/source/blender/editors/armature/armature_add.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::symmetrize`).
//!
//! Each selected bone with a side name (`.L`, `_R`, `Left`, …) gets an X-mirrored copy named
//! with the other side, or updates the existing mirror bone (when both sides are selected,
//! `direction` picks the source by the heads' X). Mirrors are parented to the mirror of the
//! parent (or the same centre parent), take the flags and bone collections (mirrored
//! collection names, created when missing), positions and roll mirrored in X, and are selected
//! instead of the sources. Bones without a side name are deselected. Their pose channels take
//! the source channel's locks, rotation mode and custom shape. `CANCELLED` without a selected
//! bone. Bone colours are not modelled by frap (`copy_bone_colors` has no effect).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature::{self, symmetrize::Direction};

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0)]
fn exec(cx: &mut OpCtx, p: armature::Symmetrize) -> OpResult {
    if common::selected_count(cx) == 0 {
        return Ok(OpStatus::Cancelled);
    }
    let direction = match p.direction.value {
        Direction::NegativeX => -1,
        Direction::PositiveX => 1,
    };
    for (ob, arm) in common::edit_armatures(cx) {
        let Some((a, mut ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        let mut collections = a.collections.clone();
        let pairs = ea.symmetrize(&a, &mut collections, direction);
        let names: Vec<(String, String)> = pairs
            .iter()
            .map(|&(s, d, _)| {
                (
                    ea.edit_bones[s].name.to_string(),
                    ea.edit_bones[d].name.to_string(),
                )
            })
            .collect();
        common::store(cx, arm, ea);
        if collections != a.collections {
            let (main, log) = cx.main_mut();
            if let Some(m) = main.armatures.get_mut(arm, log) {
                m.collections = collections;
            }
        }
        for (s, d) in names {
            common::copy_pchan(cx, ob, &s, &d)?;
        }
    }
    Ok(OpStatus::Finished)
}
