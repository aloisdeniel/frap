// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.select_all(action='TOGGLE')`, tier T0 (WP 6.2). Port of
//! `armature_de_select_all_exec` (blender/source/blender/editors/armature/armature_select.cc),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Over `context.visible_bones` of every armature in Edit Mode: `TOGGLE` deselects when any
//! visible bone has any selection flag, else selects; `SELECT` sets the three flags of every
//! selectable bone and the tip of a connected parent; `DESELECT` clears them; `INVERT` swaps
//! the body selection (`BONE_SELECTED`). Always `FINISHED`.
//!
//! [`common`] holds what every `armature.*` operator shares: the armatures in Edit Mode and
//! copy-on-write access to their edit bones.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::tools::SelectAction;
use frap_spec_ops_anim::armature;

/// Shared by the `armature.*` operators.
pub(crate) mod common {
    use std::sync::Arc;

    use frap_op::prelude::*;
    use frap_rna::{ArmatureId, IdRef, ObjectId};
    use frap_scene::ObjectMode;
    use frap_scene::edit_armature::EditArmature;
    use frap_scene::id::Armature;

    /// The armature objects in Edit Mode with unique data, active first
    /// (`BKE_view_layer_array_from_objects_in_edit_mode_unique_data`).
    pub(crate) fn edit_armatures(cx: &mut OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<ArmatureId>)> {
        let (c, edit) = cx.split_edit();
        c.objects_in_mode_unique_data()
            .into_iter()
            .filter_map(|ob| {
                let o = c.main.objects.get(ob)?;
                if o.mode != ObjectMode::Edit {
                    return None;
                }
                let arm = o.data?.typed::<ArmatureId>()?;
                edit.armatures.contains_key(&arm.key()).then_some((ob, arm))
            })
            .collect()
    }

    /// A copy of the armature (for visibility) and its edit bones.
    pub(crate) fn snapshot(cx: &mut OpCtx, arm: IdRef<ArmatureId>) -> Option<(Armature, EditArmature)> {
        let (c, edit) = cx.split_edit();
        let a = c.main.armatures.get(arm)?.clone();
        let ea = (**edit.armatures.get(&arm.key())?).clone();
        Some((a, ea))
    }

    /// Stores edited bones back (copy-on-write; the armature is logged as modified).
    pub(crate) fn store(cx: &mut OpCtx, arm: IdRef<ArmatureId>, ea: EditArmature) {
        let changed = {
            let (_, edit) = cx.split_edit();
            match edit.armatures.get_mut(&arm.key()) {
                Some(slot) if **slot != ea => {
                    *slot = Arc::new(ea);
                    true
                }
                _ => false,
            }
        };
        if changed {
            cx.main_mut().1.modified.insert(arm.any());
        }
    }

    /// Runs `f` on the edit bones of `arm` and stores them when it returns.
    pub(crate) fn with_edit<R>(
        cx: &mut OpCtx,
        arm: IdRef<ArmatureId>,
        f: impl FnOnce(&mut EditArmature, &Armature) -> R,
    ) -> Option<R> {
        let (a, mut ea) = snapshot(cx, arm)?;
        let r = f(&mut ea, &a);
        store(cx, arm, ea);
        Some(r)
    }

    /// Number of `context.selected_bones` over every armature in Edit Mode
    /// (`CTX_DATA_COUNT(C, selected_bones)`).
    pub(crate) fn selected_count(cx: &mut OpCtx) -> usize {
        edit_armatures(cx)
            .into_iter()
            .filter_map(|(_, arm)| snapshot(cx, arm))
            .map(|(a, ea)| ea.selected_bones(&a, false).len())
            .sum()
    }

    /// `BKE_pose_channel_ensure(dst)` + `BKE_pose_channel_copy_data(dst, src)` on the pose of
    /// `ob` (`copy_pchan`): the locks, rotation mode and custom shape follow; a missing source
    /// channel is created with default values first.
    pub(crate) fn copy_pchan(
        cx: &mut OpCtx,
        ob: IdRef<ObjectId>,
        src: &str,
        dst: &str,
    ) -> Result<(), OpError> {
        if cx.main().objects.get(ob).is_none_or(|o| o.pose.is_none()) {
            return Ok(());
        }
        let o = cx.object_mut(ob)?;
        let Some(pose) = o.pose.as_mut() else {
            return Ok(());
        };
        let from = pose.ensure(src).clone();
        pose.ensure(dst).copy_data_from(&from);
        Ok(())
    }
}

#[operator(poll = poll::EDITARMATURE, tier = T0, spec = armature::OPSPEC_ARMATURE_OT_SELECT_ALL)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureSelectAll) -> OpResult {
    let mut action = SelectAction::from_identifier(p.action.value.identifier());
    let arms = common::edit_armatures(cx);
    if action == SelectAction::Toggle {
        let any = arms.iter().any(|&(_, arm)| {
            common::snapshot(cx, arm).is_some_and(|(a, ea)| ea.any_visible_selected(&a))
        });
        action = if any {
            SelectAction::Deselect
        } else {
            SelectAction::Select
        };
    }
    for (_, arm) in arms {
        common::with_edit(cx, arm, |ea, a| ea.select_all(a, action));
    }
    Ok(OpStatus::Finished)
}
