// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.autoside_names(type='XAXIS')`, tier T0 (WP 8.3.10). Port of
//! `armature_autoside_names_exec` (blender/source/blender/editors/armature/armature_naming.cc)
//! and `bone_autoside_name` (blender/source/blender/blenkernel/intern/armature.cc), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! On every armature in Edit Mode, each selected unlocked bone (visible or not; with
//! `Armature.use_mirror_x` its unselected X mirror first) is renamed with the side extension of
//! the axis it lies on: `.L`/`.R` (X), `.Fr`/`.Bk` (Y), `.Top`/`.Bot` (Z), from the head's
//! coordinate (the tail's when the head is on the plane; no extension when both are), after
//! stripping existing extensions. Renames go through `ED_armature_bone_rename`. `CANCELLED`
//! when no bone got an extension.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::SEL;
use frap_scene::edit_armature::names::bone_autoside_name;
use frap_spec_ops_anim::armature::{self, autoside_names::Type};

use super::armature__flip_names::rename_cx;
use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_AUTOSIDE_NAMES
)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureAutosideNames) -> OpResult {
    let axis = match p.r#type.value {
        Type::Xaxis => 0,
        Type::Yaxis => 1,
        Type::Zaxis => 2,
    };
    let mut changed_multi = false;
    for (_, arm) in common::edit_armatures(cx) {
        let Some((a, first)) = common::snapshot(cx, arm) else {
            continue;
        };
        for i in 0..first.edit_bones.len() {
            // The current bones (earlier renames changed names, never indices).
            let Some((_, ea)) = common::snapshot(cx, arm) else {
                break;
            };
            if !ea.is_editable(i) {
                continue;
            }
            if a.use_mirror_x
                && let Some(f) = ea.mirrored(i)
                && !ea.edit_bones[f].select[SEL]
            {
                let fb = &ea.edit_bones[f];
                if let Some(new) = bone_autoside_name(&fb.name, axis, fb.head[axis], fb.tail[axis]) {
                    let old = fb.name.to_string();
                    rename_cx::rename(cx, arm, &old, &new);
                    changed_multi = true;
                }
            }
            let Some((_, ea)) = common::snapshot(cx, arm) else {
                break;
            };
            let b = &ea.edit_bones[i];
            if let Some(new) = bone_autoside_name(&b.name, axis, b.head[axis], b.tail[axis]) {
                let old = b.name.to_string();
                rename_cx::rename(cx, arm, &old, &new);
                changed_multi = true;
            }
        }
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
