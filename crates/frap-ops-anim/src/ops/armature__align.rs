// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.align()`, tier T0 (WP 8.3.10). Port of `armature_align_bones_exec`,
//! `bone_align_to_bone` and `fix_editbone_connected_children`
//! (blender/source/blender/editors/armature/armature_edit.cc), Blender 5.2.4,
//! GPL-2.0-or-later (`EditArmature::align_to`).
//!
//! The selected editable bones (`context.selected_editable_bones`, of every armature in Edit
//! Mode) keep their head and length and take the direction and roll of the active bone (with
//! `Armature.use_mirror_x`, mirror bones align to the active bone's mirror); with at most one
//! such bone, the active bone aligns to its parent instead. Connected descendants follow the
//! moved tails. Reports INFO `N bones aligned to bone 'B'` / `Aligned bone 'B' to parent`;
//! without an active bone an ERROR `Operation requires an active bone`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

#[operator(poll = poll::EDITARMATURE, tier = T0, spec = armature::OPSPEC_ARMATURE_OT_ALIGN)]
fn exec(cx: &mut OpCtx, _p: armature::Align) -> OpResult {
    let arms = common::edit_armatures(cx);
    let Some(&(_, act_arm)) = arms.first() else {
        return Ok(OpStatus::Cancelled);
    };
    let Some((a, mut ea)) = common::snapshot(cx, act_arm) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(act) = ea.active.filter(|&i| i < ea.edit_bones.len()) else {
        return Err(OpError::Runtime("Operation requires an active bone".into()));
    };
    let mirror = a.use_mirror_x;
    let actmirb = mirror.then(|| ea.mirrored(act).unwrap_or(act));
    let num_selected: usize = arms
        .iter()
        .filter_map(|&(_, arm)| common::snapshot(cx, arm))
        .map(|(a, ea)| ea.selected_bones(&a, true).len())
        .sum();
    let act_name = ea.edit_bones[act].name.clone();
    if num_selected <= 1 {
        if let Some(p) = ea.edit_bones[act].parent {
            let pb = ea.edit_bones[p].clone();
            ea.align_to(act, &pb, true);
            if let Some(m) = actmirb
                && let Some(mp) = ea.edit_bones[m].parent
            {
                let mpb = ea.edit_bones[mp].clone();
                ea.align_to(m, &mpb, true);
            }
            cx.report(
                ReportType::Info,
                format!("Aligned bone '{act_name}' to parent"),
            );
        }
        common::store(cx, act_arm, ea);
        return Ok(OpStatus::Finished);
    }
    // The active armature first (its bones fix their connected children), then the others,
    // which read the active bone as it is then.
    for i in ea.selected_bones(&a, true) {
        if i == act || Some(i) == actmirb {
            continue;
        }
        if ea.edit_bones[i].select[frap_scene::edit_armature::SEL] {
            let ab = ea.edit_bones[act].clone();
            ea.align_to(i, &ab, true);
        } else if let Some(m) = actmirb {
            let mb = ea.edit_bones[m].clone();
            ea.align_to(i, &mb, true);
        }
    }
    let act_bone = ea.edit_bones[act].clone();
    let mir_bone = actmirb.map(|m| ea.edit_bones[m].clone());
    common::store(cx, act_arm, ea);
    for &(_, arm) in arms.iter().skip(1) {
        common::with_edit(cx, arm, |e, oa| {
            for i in e.selected_bones(oa, true) {
                if e.edit_bones[i].select[frap_scene::edit_armature::SEL] {
                    e.align_to(i, &act_bone, false);
                } else if let Some(mb) = &mir_bone {
                    e.align_to(i, mb, false);
                }
            }
        });
    }
    cx.report(
        ReportType::Info,
        format!("{num_selected} bones aligned to bone '{act_name}'"),
    );
    Ok(OpStatus::Finished)
}
