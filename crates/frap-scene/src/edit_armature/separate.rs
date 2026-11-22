// SPDX-License-Identifier: GPL-3.0-or-later
//! `armature.separate` (WP 8.3.10, M8): the selected bones of an armature in Edit Mode move
//! to a new armature object.
//!
//! Blender (`separate_armature_exec`, verified on 5.2.4) writes the edit bones back, duplicates
//! the object (`ed::object::add_duplicate` with `USER_DUP_ARM | USER_DUP_ACT`: the object and
//! its armature are copied, `Armature.001`, with their active actions; the copy is linked to the
//! collections of the original, keeps its transform, pose channels and base selection, and is in
//! Object Mode), removes the selected bones from the original and the unselected ones from the
//! copy (children of removed bones lose their parent, pose channels go with their bones, both
//! armatures end deselected), redirects objects bone-parented to a moved bone to the copy, and
//! re-enters Edit Mode on the original, deselected. The original stays active.
//!
//! Ported from blender/source/blender/editors/armature/armature_relations.cc
//! (`separate_armature_exec`, `separate_armature_bones`, `separated_armature_fix_links`) and
//! blender/source/blender/editors/object/object_add.cc (`add_duplicate`,
//! `object_add_sync_base_collection`), Blender 5.2.4, GPL-2.0-or-later. frap has no
//! constraints or drivers, so their target fix-ups have no counterpart.

use std::collections::HashMap;

use frap_rna::{ActionId, ArmatureId, IdAny, IdRef, ObjectId, SceneId};

use super::EditArmature;
use crate::id::{Armature, Pose, layers};
use crate::{BaseFlag, ChangeLog, Main, ObjectMode};

/// `has_selected_bone` / `has_selected_any` of `separate_armature_exec`: whether a visible
/// bone is selected, and whether a visible bone has only a selected head or tail.
pub fn selection_state(ea: &EditArmature, arm: &Armature) -> (bool, bool) {
    let mut any = false;
    for i in 0..ea.edit_bones.len() {
        if !ea.is_visible(arm, i) {
            continue;
        }
        let s = ea.edit_bones[i].select;
        if s[super::SEL] {
            return (true, any);
        }
        if s[super::TIP] || s[super::ROOT] {
            any = true;
        }
    }
    (false, any)
}

/// `ED_armature_from_edit` + `BKE_pose_rebuild` of every object using `arm`.
fn from_edit(main: &mut Main, log: &mut ChangeLog, arm: IdRef<ArmatureId>, ea: &EditArmature) {
    let Some(a) = main.armatures.get_mut(arm, log) else {
        return;
    };
    ea.apply_to(a);
    let a = a.clone();
    let users: Vec<IdRef<ObjectId>> = main
        .objects
        .iter()
        .filter(|(_, o)| o.data == Some(arm.any()))
        .map(|(r, _)| r)
        .collect();
    for ob in users {
        if let Some(o) = main.objects.get_mut(ob, log) {
            Pose::rebuild(&mut o.pose, &a);
        }
    }
}

/// `separate_armature_bones(ob, is_select)`: removes the selected (or unselected) bones of the
/// armature of `ob` (out of Edit Mode) with their pose channels; children of a removed bone
/// lose their parent; the remaining bones are deselected.
fn separate_bones(
    main: &mut Main,
    log: &mut ChangeLog,
    ob: IdRef<ObjectId>,
    arm: IdRef<ArmatureId>,
    is_select: bool,
) {
    let Some(a) = main.armatures.get(arm).cloned() else {
        return;
    };
    let mut ea = EditArmature::from_armature(&a);
    let channels: Vec<smol_str::SmolStr> = main
        .objects
        .get(ob)
        .and_then(|o| o.pose.as_ref())
        .map(|p| p.bones.iter().map(|pb| pb.name.clone()).collect())
        .unwrap_or_default();
    let mut removed: Vec<smol_str::SmolStr> = Vec::new();
    for name in channels {
        let Some(cur) = ea.find(&name) else {
            continue;
        };
        if is_select != ea.is_selected(&a, cur) {
            continue;
        }
        for b in &mut ea.edit_bones {
            if b.parent == Some(cur) {
                b.parent = None;
                b.use_connect = false;
            }
        }
        ea.remove_bone(cur, false);
        removed.push(name);
    }
    if !removed.is_empty()
        && let Some(o) = main.objects.get_mut(ob, log)
        && let Some(p) = o.pose.as_mut()
    {
        p.bones.retain(|pb| !removed.contains(&pb.name));
    }
    ea.deselect_all();
    from_edit(main, log, arm, &ea);
}

/// A copy of action `act` (`BKE_id_copy_for_duplicate`: same requested name, uniquified).
fn copy_action(
    main: &mut Main,
    log: &mut ChangeLog,
    act: IdRef<ActionId>,
    map: &mut HashMap<IdAny, IdAny>,
) -> Option<IdRef<ActionId>> {
    if let Some(n) = map.get(&act.any()) {
        return n.typed();
    }
    let mut v = main.actions.get(act)?.clone();
    let name = v.header.name.clone();
    v.header.use_fake_user = false;
    v.header.library = None;
    let n = main.actions.insert(&name, v, log);
    map.insert(act.any(), n.any());
    Some(n)
}

/// `separate_armature_exec` for one object in Edit Mode whose edit bones are `ea` (already
/// taken out of the edit state, with a selected visible bone): returns the new object and the
/// edit bones the original re-enters Edit Mode with. `scene` is the context scene.
pub fn separate(
    main: &mut Main,
    log: &mut ChangeLog,
    scene: IdRef<SceneId>,
    ob_old: IdRef<ObjectId>,
    arm_old: IdRef<ArmatureId>,
    ea: &EditArmature,
) -> Option<(IdRef<ObjectId>, EditArmature)> {
    // 1) Leave Edit Mode on the original (data only; its mode stays EDIT).
    from_edit(main, log, arm_old, ea);
    // 2) `add_duplicate(USER_DUP_ARM | USER_DUP_ACT)`.
    let mut map: HashMap<IdAny, IdAny> = HashMap::new();
    let arm_new = {
        let mut a = main.armatures.get(arm_old)?.clone();
        let name = a.header.name.clone();
        a.header.use_fake_user = false;
        a.header.library = None;
        main.armatures.insert(&name, a, log)
    };
    map.insert(arm_old.any(), arm_new.any());
    let ob_new = {
        let mut o = main.objects.get(ob_old)?.clone();
        let name = o.header.name.clone();
        o.header.use_fake_user = false;
        o.header.library = None;
        o.mode = ObjectMode::Object;
        main.objects.insert(&name, o, log)
    };
    map.insert(ob_old.any(), ob_new.any());
    // `BKE_animdata_duplicate_id_action`: the active actions of the copies.
    let ob_act = main
        .objects
        .get(ob_new)
        .and_then(|o| o.animation_data.as_ref())
        .and_then(|ad| ad.action);
    if let Some(act) = ob_act
        && let Some(n) = copy_action(main, log, act, &mut map)
        && let Some(ad) = main
            .objects
            .get_mut(ob_new, log)
            .and_then(|o| o.animation_data.as_mut())
    {
        ad.action = Some(n);
    }
    let arm_act = main
        .armatures
        .get(arm_new)
        .and_then(|a| a.animation_data.as_ref())
        .and_then(|ad| ad.action);
    if let Some(act) = arm_act
        && let Some(n) = copy_action(main, log, act, &mut map)
        && let Some(ad) = main
            .armatures
            .get_mut(arm_new, log)
            .and_then(|a| a.animation_data.as_mut())
    {
        ad.action = Some(n);
    }
    // `BKE_libblock_relink_to_newid`: the copy uses the copied armature (and actions).
    crate::scenes::relink_ids(main, &[ob_new.any(), arm_new.any()], &map, log);
    if let Some(o) = main.objects.get_mut(ob_new, log) {
        o.data = Some(arm_new.any());
    }
    // `object_add_sync_base_collection` → `BKE_collection_object_add_from`.
    let colls: Vec<_> = main
        .collections
        .iter()
        .filter(|(_, c)| c.objects.contains(&ob_old))
        .map(|(r, _)| r)
        .collect();
    let in_master = main
        .scenes
        .get(scene)
        .is_some_and(|s| s.collection.objects.contains(&ob_old));
    for c in &colls {
        if let Some(c) = main.collections.get_mut(*c, log) {
            c.objects.push(ob_new);
        }
    }
    if (in_master || colls.is_empty())
        && let Some(s) = main.scenes.get_mut(scene, log)
    {
        s.collection.objects.push(ob_new);
    }
    layers::sync_all(main, log);
    // The copy's base keeps the original's selection.
    let scenes: Vec<IdRef<SceneId>> = main.scenes.iter().map(|(r, _)| r).collect();
    for s in scenes {
        let Some(sc) = main.scenes.get(s) else {
            continue;
        };
        let wanted: Vec<bool> = sc
            .view_layers
            .iter()
            .map(|vl| vl.base(ob_old).is_some_and(|b| b.selected()))
            .collect();
        if let Some(sc) = main.scenes.get_mut(s, log) {
            for (vl, sel) in sc.view_layers.iter_mut().zip(wanted) {
                if let Some(b) = vl.base_mut(ob_new) {
                    b.flag.set(BaseFlag::SELECTED, sel);
                }
            }
        }
    }
    log.context_changed = true;
    // 3) Remove the bones that should not stay on either armature.
    separate_bones(main, log, ob_old, arm_old, true);
    separate_bones(main, log, ob_new, arm_new, false);
    // 4) `separated_armature_fix_links`: bone children follow their bone.
    let new_channels: Vec<smol_str::SmolStr> = main
        .objects
        .get(ob_new)
        .and_then(|o| o.pose.as_ref())
        .map(|p| p.bones.iter().map(|pb| pb.name.clone()).collect())
        .unwrap_or_default();
    let children: Vec<IdRef<ObjectId>> = main
        .objects
        .iter()
        .filter(|(_, o)| {
            o.parent == Some(ob_old)
                && o.parent_type == "BONE"
                && !o.parent_bone.is_empty()
                && new_channels.contains(&o.parent_bone)
        })
        .map(|(r, _)| r)
        .collect();
    for c in children {
        if let Some(o) = main.objects.get_mut(c, log) {
            o.parent = Some(ob_new);
        }
    }
    // 5) Re-enter Edit Mode on the original, deselected.
    let mut back = EditArmature::from_armature(main.armatures.get(arm_old)?);
    back.deselect_all();
    Some((ob_new, back))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_state_reads_visible_bones() {
        let arm = Armature::new("A");
        let mut ea = EditArmature::default();
        let a = ea.add_bone("A");
        ea.edit_bones[a].select = [false, false, true];
        assert_eq!(selection_state(&ea, &arm), (false, true));
        ea.edit_bones[a].select = [true; 3];
        assert_eq!(selection_state(&ea, &arm), (true, false));
        ea.edit_bones[a].hide = true;
        assert_eq!(selection_state(&ea, &arm), (false, false));
    }
}
