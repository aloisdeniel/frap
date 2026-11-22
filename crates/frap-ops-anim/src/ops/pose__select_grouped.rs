// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.select_grouped(extend=False, type='COLLECTION')`, tier T0 (WP 8.3.11). Port of
//! `pose_select_grouped_exec`, `pose_select_same_collection`, `pose_select_same_color`,
//! `pose_select_same_keyingset`, `pose_select_children`, `pose_select_parents` and
//! `pose_select_siblings` (blender/source/blender/editors/armature/pose_select.cc), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! - `COLLECTION`: the visible bones (of every armature in Pose Mode) in any bone collection,
//!   by name, of the active pose bone; `CANCELLED` without an active pose bone.
//! - `COLOR`: the visible bones with the active bone's colour. frap models no bone colours
//!   (deviations §2, WP 8.3.10), so every bone has the default colour and every visible bone
//!   matches, which is what Blender does on armatures without colours.
//! - `KEYINGSET`: scenes have no keying sets in frap (deviations §2, WP 6.4), so there is never
//!   an active one: `RuntimeError: Error: No active Keying Set to use`.
//! - `CHILDREN` / `CHILDREN_IMMEDIATE`: every descendant / the direct children of the selected
//!   bones; `PARENT`: their parents (the active bone stays); `SIBLINGS`: the other children of
//!   their parents (without `extend` Blender deselects the parents, not the bones).
//!
//! Without `extend` the selected bones are deselected first (`SIBLINGS`: their parents).
//! `COLLECTION` and `COLOR` skip a bone that is both selected and `hide_select` but select an
//! unselected `hide_select` bone (Blender's test). `CANCELLED` when nothing changed (the
//! statement is rolled back; Blender keeps a deselect that preceded the cancel, D-159).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose::{self, select_grouped::Type};
use smol_str::SmolStr;

use super::pose__select_hierarchy::chans::{Chans, load_all};
use super::pose__select_hierarchy::family;

/// The active pose bone's channel and its object's channels (`CTX_data_active_pose_bone`).
fn active_bone_collections(cx: &OpCtx) -> Option<Vec<SmolStr>> {
    let c = cx.ctx();
    let b = c.active_pose_bone()?;
    let o = c.main.objects.get(b.object)?;
    let arm = c.main.armatures.get(o.data?.typed()?)?;
    Some(arm.bone(&b.bone)?.collections.clone())
}

/// Deselects the selected visible bones of every pose object (`selected_pose_bones`).
fn deselect_selected(all: &mut [Chans]) -> bool {
    let mut changed = false;
    for ch in all.iter_mut() {
        for c in ch.selected_chans() {
            ch.deselect(c);
            changed = true;
        }
    }
    changed
}

/// `pose_select_same_collection` / `pose_select_same_color` (`colls = None`: every bone
/// matches).
fn select_same(cx: &mut OpCtx, extend: bool, colls: Option<&[SmolStr]>) -> Result<bool, OpError> {
    let mut all = load_all(cx, false);
    let mut changed = false;
    if !extend {
        changed |= deselect_selected(&mut all);
    }
    for ch in all.iter_mut() {
        for c in ch.visible_chans() {
            if ch.pb(c).select && ch.unselectable(c) {
                continue;
            }
            let hit = match colls {
                None => true,
                Some(names) => ch.bone(c).is_some_and(|b| {
                    ch.arm.bones[b]
                        .collections
                        .iter()
                        .any(|n| names.contains(n))
                }),
            };
            if hit {
                ch.select(c);
                changed = true;
            }
        }
    }
    for ch in all {
        ch.store(cx)?;
    }
    Ok(changed)
}

#[operator(
    poll = poll::POSEMODE,
    tier = T0,
    notes = [
        "COLOR: bone colours are not modelled, so every visible bone has the active bone's colour",
        "KEYINGSET always fails: keying sets are not bound (No active Keying Set to use)",
    ]
)]
fn exec(cx: &mut OpCtx, p: pose::SelectGrouped) -> OpResult {
    let extend = p.extend.value;
    let changed = match p.r#type.value {
        Type::Collection => match active_bone_collections(cx) {
            Some(colls) => select_same(cx, extend, Some(&colls))?,
            None => false,
        },
        Type::Color => {
            if cx.ctx().active_pose_bone().is_none() {
                // Blender dereferences the missing active bone; frap cancels.
                false
            } else {
                select_same(cx, extend, None)?
            }
        }
        Type::Keyingset => {
            return Err(OpError::Runtime("No active Keying Set to use".into()));
        }
        Type::Children => family::select_children(cx, true, extend, false, false)?,
        Type::ChildrenImmediate => family::select_children(cx, false, extend, false, false)?,
        Type::Parent => family::select_parents(cx, extend, false, false)?,
        Type::Siblings => family::select_siblings(cx, extend)?,
    };
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
