// SPDX-License-Identifier: GPL-3.0-or-later
//! The armature Edit Mode handler of `MODE_HANDLERS` (ARCHITECTURE §8; WP 6.2, M6). Port of
//! the armature branches of `editmode_enter_ex` / `editmode_load_free_ex`
//! (blender/source/blender/editors/object/object_edit.cc: `flush_pose_selection_to_bone`,
//! `flush_bone_selection_to_pose`) and of `ED_armature_to_edit` / `ED_armature_from_edit`
//! (blender/source/blender/editors/armature/armature_utils.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! - **Enter** (the active object, then every selected armature with other data): the pose
//!   selection of the object is copied onto its rest bones (pose bones carry the selection
//!   since Blender 5.0), then the armature gets edit bones (`EditState::armatures`). An object
//!   whose armature is already being edited (a second user) is skipped; linked objects or
//!   armatures report `Unable to enter edit-mode on library data for object '…'`; when the
//!   active object cannot enter, nothing does and the operator is cancelled.
//! - **Exit**: the edit bones are written back (`ED_armature_from_edit`: depth-first bone
//!   order, rest matrices from head/tail/roll, near-zero bones elongated), every object using
//!   the armature has its pose rebuilt (`BKE_pose_rebuild`: channels kept by name, new ones
//!   with default values, orphans removed), the leaving object's pose takes the bones'
//!   selection, and `Object.mode` goes back to `OBJECT`.

use std::sync::Arc;

use frap_op::prelude::*;
use frap_op::{MODE_HANDLERS, ModeHandler};
use frap_rna::{ArmatureId, IdRef, ObjectId};
use frap_scene::ObjectMode;
use frap_scene::edit_armature::EditArmature;
use frap_scene::id::Pose;

/// Why an object cannot enter.
enum Skip {
    Silently,
    AlreadyEditing,
    Library(String),
}

fn can_enter(cx: &mut OpCtx, ob: IdRef<ObjectId>) -> Result<IdRef<ArmatureId>, Skip> {
    let (c, edit) = cx.split_edit();
    let o = c.main.objects.get(ob).ok_or(Skip::Silently)?;
    let arm = o
        .data
        .and_then(|d| d.typed::<ArmatureId>())
        .ok_or(Skip::Silently)?;
    let a = c.main.armatures.get(arm).ok_or(Skip::Silently)?;
    if o.header.library.is_some() || a.header.library.is_some() {
        return Err(Skip::Library(o.header.name.to_string()));
    }
    if edit.armatures.contains_key(&arm.key()) {
        let users_editing = c
            .main
            .objects
            .iter()
            .any(|(r, x)| r != ob && x.mode == ObjectMode::Edit && x.data == Some(arm.any()));
        if users_editing {
            return Err(Skip::AlreadyEditing);
        }
        edit.armatures.remove(&arm.key());
    }
    Ok(arm)
}

/// `flush_pose_selection_to_bone` + `ED_armature_to_edit` + `Object.mode = 'EDIT'`.
fn enter_one(cx: &mut OpCtx, ob: IdRef<ObjectId>, arm: IdRef<ArmatureId>) -> Result<(), OpError> {
    let pose = cx.main().objects.get(ob).and_then(|o| o.pose.clone());
    if let Some(pose) = pose {
        let (main, log) = cx.main_mut();
        if let Some(a) = main.armatures.get_mut(arm, log) {
            for pb in &pose.bones {
                if let Some(b) = a.bones.iter_mut().find(|b| b.name == pb.name) {
                    b.select = [pb.select, pb.select_head, pb.select_tail];
                }
            }
        }
    }
    let ea = {
        let a = cx
            .main()
            .armatures
            .get(arm)
            .ok_or_else(|| OpError::Runtime("armature no longer exists".into()))?;
        EditArmature::from_armature(a)
    };
    let (_, edit) = cx.split_edit();
    edit.armatures.insert(arm.key(), Arc::new(ea));
    if cx
        .main()
        .objects
        .get(ob)
        .is_some_and(|o| o.mode != ObjectMode::Edit)
    {
        cx.object_mut(ob)?.mode = ObjectMode::Edit;
        cx.main_mut().1.context_changed = true;
    }
    Ok(())
}

/// Enters armature Edit Mode for `objects` (the active object first).
fn enter(cx: &mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult {
    for (i, &ob) in objects.iter().enumerate() {
        match can_enter(cx, ob) {
            Ok(arm) => enter_one(cx, ob, arm)?,
            Err(Skip::AlreadyEditing) => {}
            Err(Skip::Silently) if i > 0 => {}
            Err(Skip::Silently) => return Ok(OpStatus::Cancelled),
            Err(Skip::Library(name)) => {
                cx.report(
                    ReportType::Warning,
                    format!("Unable to enter edit-mode on library data for object '{name}'"),
                );
                if i == 0 {
                    return Ok(OpStatus::Cancelled);
                }
            }
        }
    }
    Ok(OpStatus::Finished)
}

/// `ED_armature_from_edit` + `BKE_pose_rebuild` of every user, then drops the edit bones.
/// Returns whether the armature had edit bones.
pub(crate) fn load(cx: &mut OpCtx, arm: IdRef<ArmatureId>) -> Result<bool, OpError> {
    let Some(ea) = cx.split_edit().1.armatures.remove(&arm.key()) else {
        return Ok(false);
    };
    let users: Vec<IdRef<ObjectId>> = cx
        .main()
        .objects
        .iter()
        .filter(|(_, o)| o.data == Some(arm.any()))
        .map(|(r, _)| r)
        .collect();
    let (main, log) = cx.main_mut();
    let Some(a) = main.armatures.get_mut(arm, log) else {
        return Ok(true);
    };
    ea.apply_to(a);
    let a = a.clone();
    for ob in users {
        if let Some(o) = main.objects.get_mut(ob, log) {
            Pose::rebuild(&mut o.pose, &a);
        }
    }
    Ok(true)
}

/// `flush_bone_selection_to_pose` for one object.
pub(crate) fn flush_selection_to_pose(
    cx: &mut OpCtx,
    ob: IdRef<ObjectId>,
    arm: IdRef<ArmatureId>,
) -> Result<(), OpError> {
    let Some(a) = cx.main().armatures.get(arm).cloned() else {
        return Ok(());
    };
    let o = cx.object_mut(ob)?;
    if let Some(p) = o.pose.as_mut() {
        p.flush_selection_from(&a);
    }
    Ok(())
}

/// Leaves armature Edit Mode for `objects`; orphaned edit armatures (no object in Edit Mode
/// uses them any more) are written back too.
fn exit(cx: &mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult {
    for &ob in objects {
        let Some(o) = cx.main().objects.get(ob) else {
            continue;
        };
        if o.mode != ObjectMode::Edit {
            continue;
        }
        if let Some(arm) = o.data.and_then(|d| d.typed::<ArmatureId>())
            && load(cx, arm)?
        {
            flush_selection_to_pose(cx, ob, arm)?;
        }
        cx.object_mut(ob)?.mode = ObjectMode::Object;
        cx.main_mut().1.context_changed = true;
    }
    let orphans: Vec<IdRef<ArmatureId>> = {
        let (c, edit) = cx.split_edit();
        edit.armatures
            .keys()
            .map(|k| IdRef::<ArmatureId>::from_key(*k))
            .filter(|a| {
                !c.main
                    .objects
                    .iter()
                    .any(|(_, o)| o.mode == ObjectMode::Edit && o.data == Some(a.any()))
            })
            .collect()
    };
    for arm in orphans {
        load(cx, arm)?;
    }
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(MODE_HANDLERS)]
static EDIT_ARMATURE: ModeHandler = ModeHandler {
    mode: ObjectMode::Edit,
    object_type: "ARMATURE",
    enter,
    exit,
};
