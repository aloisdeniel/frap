// SPDX-License-Identifier: GPL-3.0-or-later
//! The Pose Mode handler of `MODE_HANDLERS` (ARCHITECTURE §8; WP 6.2, M6). Port of
//! `ED_object_posemode_enter_ex` / `ED_object_posemode_exit_ex`
//! (blender/source/blender/editors/armature/pose_edit.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Entering sets `Object.mode = 'POSE'` on armature objects (a linked one reports
//! `Cannot pose libdata`; when it is the active object nothing changes and the operator is
//! cancelled); leaving sets it back to `OBJECT`. Pose Mode keeps no data of its own: the pose
//! channels live on the object. An armature object whose pose was never built (made with
//! `bpy.data.objects.new`) gets it here, as Blender's depsgraph would build it
//! (`BKE_pose_ensure`), with the rest bones' selection.

use frap_op::prelude::*;
use frap_op::{MODE_HANDLERS, ModeHandler};
use frap_rna::{ArmatureId, IdRef, ObjectId};
use frap_scene::ObjectMode;
use frap_scene::id::Pose;

/// Builds the pose of `ob` when it has none (or when its channels no longer match the bones).
fn ensure_pose(cx: &mut OpCtx, ob: IdRef<ObjectId>, arm: IdRef<ArmatureId>) -> Result<(), OpError> {
    let Some(a) = cx.main().armatures.get(arm).cloned() else {
        return Ok(());
    };
    let needs = cx.main().objects.get(ob).is_some_and(|o| match &o.pose {
        None => true,
        Some(p) => {
            p.bones.len() != a.bones.len()
                || p.bones.iter().zip(&a.bones).any(|(c, b)| c.name != b.name)
        }
    });
    if needs {
        let o = cx.object_mut(ob)?;
        let fresh = o.pose.is_none();
        Pose::rebuild(&mut o.pose, &a);
        if fresh && let Some(p) = o.pose.as_mut() {
            p.flush_selection_from(&a);
        }
    }
    Ok(())
}

fn enter(cx: &mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult {
    for (i, &ob) in objects.iter().enumerate() {
        let Some(o) = cx.main().objects.get(ob) else {
            continue;
        };
        let Some(arm) = o.data.and_then(|d| d.typed::<ArmatureId>()) else {
            if i == 0 {
                return Ok(OpStatus::Cancelled);
            }
            continue;
        };
        if o.header.library.is_some() {
            cx.report(ReportType::Warning, "Cannot pose libdata");
            if i == 0 {
                return Ok(OpStatus::Cancelled);
            }
            continue;
        }
        ensure_pose(cx, ob, arm)?;
        if cx
            .main()
            .objects
            .get(ob)
            .is_some_and(|o| o.mode != ObjectMode::Pose)
        {
            cx.object_mut(ob)?.mode = ObjectMode::Pose;
            cx.main_mut().1.context_changed = true;
        }
    }
    Ok(OpStatus::Finished)
}

fn exit(cx: &mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult {
    for &ob in objects {
        if cx
            .main()
            .objects
            .get(ob)
            .is_some_and(|o| o.mode == ObjectMode::Pose)
        {
            cx.object_mut(ob)?.mode = ObjectMode::Object;
            cx.main_mut().1.context_changed = true;
        }
    }
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(MODE_HANDLERS)]
static POSE: ModeHandler = ModeHandler {
    mode: ObjectMode::Pose,
    object_type: "ARMATURE",
    enter,
    exit,
};
