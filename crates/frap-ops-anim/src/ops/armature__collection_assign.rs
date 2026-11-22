// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.collection_assign(name='')`, tier T0 (WP 8.3.10). Port of
//! `bone_collection_assign_exec`, `get_bonecoll_named_or_active`,
//! `bone_collection_assign_mode_specific` and `bone_collection_assign_poll`
//! (blender/source/blender/editors/armature/bone_collections.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Adds the selected bones of the active armature to the bone collection `name` (the active
//! collection when empty; a missing name is an ERROR `No bone collection named '…'`): in Pose
//! Mode the selected visible pose bones, in Edit Mode the selected, unlocked, visible edit
//! bones; another mode is an ERROR `This operator only works in pose mode and armature edit
//! mode`. Without such bones (WARNING `No bones selected, nothing to assign to bone
//! collection`) or when all were members already (WARNING `All selected bones were already part
//! of this collection`) it is `CANCELLED`. The poll needs an active armature object.
//!
//! [`bcoll`] holds what the bone collection operators share: their polls, the armature they
//! work on, Blender's collection array and the per-mode bone loops.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::armature;

/// Shared by the `armature.collection_*`, `assign_to_collection` and `move_to_collection`
/// operators.
pub(crate) mod bcoll {
    use frap_op::prelude::*;
    use frap_op::{Poll, PollFail, PollReq};
    use frap_rna::{ArmatureId, IdRef, ObjectId};
    use frap_scene::edit_armature::bone_collections::CollArray;
    use frap_scene::id::Armature;
    use frap_scene::{Context, ContextMode};
    use smallvec::smallvec;
    use smol_str::SmolStr;

    use super::super::armature__select_all::common;

    /// A poll failure with Blender's message (`CTX_wm_operator_poll_msg_set`).
    fn fail(idname: &str, msg: &str, req: PollReq) -> PollFail {
        PollFail {
            message: Some(format!("Operator bpy.ops.{idname}.poll() {msg}")),
            unmet: smallvec![req],
            hint: None,
        }
    }

    /// No armature in the context (a silent `false` in Blender).
    fn no_armature(c: &Context) -> PollFail {
        let hint = match c.active_object().and_then(|o| c.main.objects.get(o)) {
            Some(_) => {
                "make an armature object active, e.g. bpy.context.view_layer.objects.active = bpy.data.objects['Armature']"
            }
            None => "add one first, e.g. bpy.ops.object.armature_add()",
        };
        PollFail {
            message: None,
            unmet: smallvec![PollReq::ActiveType("ARMATURE")],
            hint: Some(hint.into()),
        }
    }

    /// `ED_armature_context`: the armature of the active object (frap has no pinned
    /// Properties Editor).
    pub(crate) fn armature_context(c: &Context) -> Option<(IdRef<ObjectId>, IdRef<ArmatureId>)> {
        let ob = c.active_object()?;
        let arm = c.main.objects.get(ob)?.data?.typed::<ArmatureId>()?;
        Some((ob, arm))
    }

    /// `bone_collection_add_poll`
    pub(crate) fn add_poll(c: &Context) -> Result<(), PollFail> {
        let Some((_, arm)) = armature_context(c) else {
            return Err(no_armature(c));
        };
        if c.main
            .armatures
            .get(arm)
            .is_some_and(|a| a.header.library.is_some())
        {
            return Err(fail(
                "armature.collection_add",
                "Cannot add bone collections to a linked Armature without an override on the Armature Data",
                PollReq::ActiveEditable,
            ));
        }
        Ok(())
    }
    /// `bone_collection_add_poll`
    pub(crate) const ADD_POLL: Poll = Poll {
        requires: &[PollReq::ActiveType("ARMATURE")],
        check: add_poll,
    };

    /// `active_bone_collection_poll` (`armature.collection_remove`).
    pub(crate) fn active_poll(c: &Context) -> Result<(), PollFail> {
        let Some((_, arm)) = armature_context(c) else {
            return Err(no_armature(c));
        };
        let has_active = c
            .main
            .armatures
            .get(arm)
            .is_some_and(|a| a.active_collection.is_some_and(|i| i < a.collections.len()));
        if !has_active {
            let mut f = fail(
                "armature.collection_remove",
                "Armature has no active bone collection, select one first",
                PollReq::ActiveType("ARMATURE"),
            );
            f.hint = Some(
                "set bpy.data.armatures['Armature'].collections.active_index (or add one with bpy.ops.armature.collection_add())".into(),
            );
            return Err(f);
        }
        Ok(())
    }
    /// `active_bone_collection_poll`
    pub(crate) const ACTIVE_POLL: Poll = Poll {
        requires: &[PollReq::ActiveType("ARMATURE")],
        check: active_poll,
    };

    /// `bone_collection_assign_poll` / `move_to_collection_poll`: the active object is a local
    /// armature (`idname` names the operator in the message).
    fn assign_poll_for(c: &Context, idname: &str, linked_msg: &str) -> Result<(), PollFail> {
        let Some(o) = c.active_object().and_then(|o| c.main.objects.get(o)) else {
            return Err(no_armature(c));
        };
        if o.object_type() != "ARMATURE" {
            let mut f = fail(
                idname,
                "Bone collections can only be edited on an Armature",
                PollReq::ActiveType("ARMATURE"),
            );
            f.hint = no_armature(c).hint;
            return Err(f);
        }
        let linked = o
            .data
            .and_then(|d| c.main.header(d))
            .is_some_and(|h| h.library.is_some());
        if linked {
            return Err(fail(idname, linked_msg, PollReq::ActiveEditable));
        }
        Ok(())
    }

    fn collection_assign_poll(c: &Context) -> Result<(), PollFail> {
        assign_poll_for(
            c,
            "armature.collection_assign",
            "Cannot edit bone collections on linked Armatures without override",
        )
    }
    /// `bone_collection_assign_poll` (`armature.collection_assign`).
    pub(crate) const ASSIGN_POLL: Poll = Poll {
        requires: &[PollReq::ActiveType("ARMATURE")],
        check: collection_assign_poll,
    };

    fn collection_unassign_poll(c: &Context) -> Result<(), PollFail> {
        assign_poll_for(
            c,
            "armature.collection_unassign",
            "Cannot edit bone collections on linked Armatures without override",
        )
    }
    /// `bone_collection_assign_poll` (`armature.collection_unassign`).
    pub(crate) const UNASSIGN_POLL: Poll = Poll {
        requires: &[PollReq::ActiveType("ARMATURE")],
        check: collection_unassign_poll,
    };

    fn assign_to_poll(c: &Context) -> Result<(), PollFail> {
        assign_poll_for(
            c,
            "armature.assign_to_collection",
            "This needs a local Armature or an override",
        )
    }
    /// `move_to_collection_poll` (`armature.assign_to_collection`).
    pub(crate) const ASSIGN_TO_POLL: Poll = Poll {
        requires: &[PollReq::ActiveType("ARMATURE")],
        check: assign_to_poll,
    };

    fn move_to_poll(c: &Context) -> Result<(), PollFail> {
        assign_poll_for(
            c,
            "armature.move_to_collection",
            "This needs a local Armature or an override",
        )
    }
    /// `move_to_collection_poll` (`armature.move_to_collection`).
    pub(crate) const MOVE_TO_POLL: Poll = Poll {
        requires: &[PollReq::ActiveType("ARMATURE")],
        check: move_to_poll,
    };

    /// The armature of `arm` and Blender's collection array of it.
    pub(crate) fn array(cx: &OpCtx, arm: IdRef<ArmatureId>) -> Option<(Armature, CollArray)> {
        let a = cx.main().armatures.get(arm)?.clone();
        let arr = CollArray::from_armature(&a);
        Some((a, arr))
    }

    /// Writes the collection array back (collections and active collection).
    pub(crate) fn store_array(cx: &mut OpCtx, arm: IdRef<ArmatureId>, arr: &CollArray) {
        let Some(mut a) = cx.main().armatures.get(arm).cloned() else {
            return;
        };
        arr.store(&mut a);
        let (main, log) = cx.main_mut();
        let unchanged = main.armatures.get(arm).is_some_and(|old| {
            old.collections == a.collections && old.active_collection == a.active_collection
        });
        if unchanged {
            return;
        }
        if let Some(m) = main.armatures.get_mut(arm, log) {
            m.collections = a.collections;
            m.active_collection = a.active_collection;
        }
    }

    /// `get_bonecoll_named_or_active`: `Ok(None)` without an active collection (silently
    /// cancelled), `Err` for an unknown name.
    pub(crate) fn named_or_active(
        cx: &OpCtx,
        arm: IdRef<ArmatureId>,
        name: &str,
    ) -> Result<Option<SmolStr>, OpError> {
        let Some(a) = cx.main().armatures.get(arm) else {
            return Ok(None);
        };
        if name.is_empty() {
            return Ok(a
                .active_collection
                .and_then(|i| a.collections.get(i))
                .map(|c| c.name.clone()));
        }
        match a.collections.iter().find(|c| c.name == name) {
            Some(c) => Ok(Some(c.name.clone())),
            None => Err(OpError::Runtime(format!(
                "No bone collection named '{name}'"
            ))),
        }
    }

    /// The outcome of `bone_collection_assign_mode_specific`.
    pub(crate) struct Assigned {
        /// The mode is Pose or armature Edit Mode.
        pub(crate) supported: bool,
        /// `had_bones_to_assign`
        pub(crate) had_bones: bool,
        /// `made_any_changes`
        pub(crate) changed: bool,
    }

    /// `bone_collection_assign_mode_specific(C, ob, bcoll, f, f)`: `f` runs on the collection
    /// list of every selected pose bone (Pose Mode, `FOREACH_PCHAN_SELECTED_IN_OBJECT`) or every
    /// selected, unlocked, visible edit bone (armature Edit Mode) of `ob`.
    pub(crate) fn assign_mode_specific(
        cx: &mut OpCtx,
        ob: IdRef<ObjectId>,
        arm: IdRef<ArmatureId>,
        f: fn(&mut Vec<SmolStr>, &SmolStr) -> bool,
        name: &SmolStr,
    ) -> Assigned {
        let mut out = Assigned {
            supported: true,
            had_bones: false,
            changed: false,
        };
        match cx.ctx().mode() {
            ContextMode::Pose => {
                let bones: Vec<SmolStr> = {
                    let main = cx.main();
                    let (Some(o), Some(a)) = (main.objects.get(ob), main.armatures.get(arm)) else {
                        return out;
                    };
                    o.pose
                        .iter()
                        .flat_map(|p| &p.bones)
                        .filter(|pb| {
                            pb.select
                                && !pb.hide
                                && a.bone(&pb.name)
                                    .is_some_and(|b| a.bone_collections_visible(b))
                        })
                        .map(|pb| pb.name.clone())
                        .collect()
                };
                if bones.is_empty() {
                    return out;
                }
                out.had_bones = true;
                let Some(mut a) = cx.main().armatures.get(arm).cloned() else {
                    return out;
                };
                for n in &bones {
                    if let Some(b) = a.bones.iter_mut().find(|b| b.name == *n) {
                        out.changed |= f(&mut b.collections, name);
                    }
                }
                let (main, log) = cx.main_mut();
                if out.changed
                    && let Some(m) = main.armatures.get_mut(arm, log)
                {
                    m.bones = a.bones;
                }
            }
            ContextMode::EditArmature => {
                let r = common::with_edit(cx, arm, |ea, a| {
                    let mut had = false;
                    let mut changed = false;
                    ea.sync_selection();
                    for i in 0..ea.edit_bones.len() {
                        if !ea.is_editable(i) || !ea.is_visible(a, i) {
                            continue;
                        }
                        changed |= f(&mut ea.edit_bones[i].collections, name);
                        had = true;
                    }
                    ea.sync_selection();
                    (had, changed)
                });
                if let Some((had, changed)) = r {
                    out.had_bones = had;
                    out.changed = changed;
                }
            }
            _ => out.supported = false,
        }
        out
    }

    /// The ERROR of an unsupported mode.
    pub(crate) fn mode_error() -> OpError {
        OpError::Runtime("This operator only works in pose mode and armature edit mode".into())
    }

    /// The armature object the assign operators work on (`context_active_object`).
    pub(crate) fn active_armature(cx: &OpCtx) -> Option<(IdRef<ObjectId>, IdRef<ArmatureId>)> {
        let c = cx.ctx();
        let ob = c.active_object()?;
        let o = c.main.objects.get(ob)?;
        Some((ob, o.data?.typed::<ArmatureId>()?))
    }
}

#[operator(
    poll = bcoll::ASSIGN_POLL,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_COLLECTION_ASSIGN
)]
fn exec(cx: &mut OpCtx, p: armature::CollectionAssign) -> OpResult {
    let Some((ob, arm)) = bcoll::active_armature(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(name) = bcoll::named_or_active(cx, arm, &p.name.value)? else {
        return Ok(OpStatus::Cancelled);
    };
    let r = bcoll::assign_mode_specific(
        cx,
        ob,
        arm,
        frap_scene::edit_armature::bone_collections::assign,
        &name,
    );
    if !r.supported {
        return Err(bcoll::mode_error());
    }
    if !r.had_bones {
        cx.report(
            ReportType::Warning,
            "No bones selected, nothing to assign to bone collection",
        );
        return Ok(OpStatus::Cancelled);
    }
    if !r.changed {
        cx.report(
            ReportType::Warning,
            "All selected bones were already part of this collection",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
