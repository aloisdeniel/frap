// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.select_all(action='TOGGLE')`, tier T0 (WP 6.2). Port of
//! `pose_de_select_all_exec` and `pose_do_bone_select`
//! (blender/source/blender/editors/armature/pose_select.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Over the visible pose bones of every armature in Pose Mode: `TOGGLE` deselects when any is
//! selected (`selected_pose_bones`), else selects; `SELECT` selects the selectable ones (not
//! `hide_select`), with head and tail; `DESELECT` clears all three flags; `INVERT` swaps.
//! Always `FINISHED`.
//!
//! [`common`] holds what the `pose.*` operators share: the armatures in Pose Mode and their
//! visible / selected channels.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::tools::SelectAction;
use frap_spec_ops_anim::pose;

/// Shared by the `pose.*` operators.
pub(crate) mod common {
    use frap_op::prelude::*;
    use frap_rna::{ArmatureId, IdRef, ObjectId};
    use frap_scene::ObjectMode;

    /// The armature objects in Pose Mode (`FOREACH_OBJECT_IN_MODE_BEGIN(…, OB_MODE_POSE)`),
    /// active first; empty when the active object is not in Pose Mode.
    pub(crate) fn pose_objects(cx: &OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<ArmatureId>)> {
        let c = cx.ctx();
        if c.pose_object().is_none() {
            return Vec::new();
        }
        c.objects_in_mode()
            .into_iter()
            .filter_map(|ob| {
                let o = c.main.objects.get(ob)?;
                if o.mode != ObjectMode::Pose || o.pose.is_none() {
                    return None;
                }
                Some((ob, o.data?.typed::<ArmatureId>()?))
            })
            .collect()
    }

    /// The channels of `ob` that are visible (`!hide`, bone in a visible collection) and,
    /// with `selected`, selected (`FOREACH_PCHAN_*_IN_OBJECT`), by name.
    pub(crate) fn channels(cx: &OpCtx, ob: IdRef<ObjectId>, arm: IdRef<ArmatureId>, selected: bool) -> Vec<String> {
        let main = cx.main();
        let (Some(o), Some(a)) = (main.objects.get(ob), main.armatures.get(arm)) else {
            return Vec::new();
        };
        let Some(pose) = o.pose.as_ref() else {
            return Vec::new();
        };
        pose.bones
            .iter()
            .filter(|pb| {
                a.bone(&pb.name)
                    .is_some_and(|b| !pb.hide && a.bone_collections_visible(b) && (!selected || pb.select))
            })
            .map(|pb| pb.name.to_string())
            .collect()
    }
}

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_SELECT_ALL)]
fn exec(cx: &mut OpCtx, p: pose::PoseSelectAll) -> OpResult {
    let mut action = SelectAction::from_identifier(p.action.value.identifier());
    let objects = common::pose_objects(cx);
    if action == SelectAction::Toggle {
        let any = objects
            .iter()
            .any(|&(ob, arm)| !common::channels(cx, ob, arm, true).is_empty());
        action = if any {
            SelectAction::Deselect
        } else {
            SelectAction::Select
        };
    }
    for (ob, arm) in objects {
        let visible = common::channels(cx, ob, arm, false);
        let unselectable: Vec<String> = cx
            .main()
            .armatures
            .get(arm)
            .map(|a| a.bones.iter().filter(|b| b.hide_select).map(|b| b.name.to_string()).collect())
            .unwrap_or_default();
        let o = cx.object_mut(ob)?;
        let Some(pose) = o.pose.as_mut() else {
            continue;
        };
        for pb in pose.bones.iter_mut().filter(|pb| visible.iter().any(|n| pb.name == n.as_str())) {
            let selectable = !unselectable.iter().any(|n| pb.name == n.as_str());
            let set = |pb: &mut frap_scene::id::PoseBone, on: bool| {
                pb.select = on;
                pb.select_head = on;
                pb.select_tail = on;
            };
            match action {
                SelectAction::Select | SelectAction::Toggle => {
                    if selectable {
                        set(pb, true);
                    }
                }
                SelectAction::Deselect => set(pb, false),
                SelectAction::Invert => {
                    if pb.select {
                        set(pb, false);
                    } else if selectable {
                        set(pb, true);
                    }
                }
            }
        }
    }
    Ok(OpStatus::Finished)
}
