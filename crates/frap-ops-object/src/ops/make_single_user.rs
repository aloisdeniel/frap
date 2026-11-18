// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.make_single_user(type='SELECTED_OBJECTS', object=False, obdata=False,
//! material=False, animation=False, obdata_animation=False)`, tier T0
//! (`tests/golden/cases/scenes/single_user.py`; M3 WP 3.10, M34W0-4). Port of
//! `make_single_user_exec`, `single_object_users(_collection)`, `libblock_relink_collection`,
//! `single_obdata_users`, `single_mat_users`, `single_object_action_users` and
//! `single_objectdata_action_users` (`editors/object/object_relations.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//! - the objects are the selected bases of the context view layer (`SELECTED_OBJECTS`) or every
//!   object of the context scene (`ALL`, scene order: the scene collection's objects, then each
//!   child collection depth first);
//! - `object` copies the objects that are in more than one scene (with their action) and
//!   relinks the scene, its collections and their objects to the copies **without copying the
//!   collections**: a collection shared with another scene then holds the copies in both
//!   scenes, and the originals are left with no user (the other scene's active object is
//!   cleared, as Blender's base is freed);
//! - `obdata` gives each object whose data has more than one user (orphan objects count) a
//!   copy of it, in object order, so the last user keeps the original; `material` does the
//!   same per material slot (the copy goes where the slot links: object or data); `animation`
//!   and `obdata_animation` copy shared actions;
//! - poll `ED_operator_objectmode`; always `FINISHED`.

use std::collections::HashMap;

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CollectionId, IdAny, IdCode, IdRef, ObjectId, SceneId};
use frap_scene::id::{Collection, layers, slots};
use frap_scene::scenes::{duplicate_id, relink_ids};
use frap_scene::{ChangeLog, Main};
use frap_spec_ops_object::object::{self, make_single_user::Type};

/// The collections of a scene in `scene_collections_array` order: `None` (the scene
/// collection), then every child collection depth first (each once).
fn scene_collections(main: &Main, scene: IdRef<SceneId>) -> Vec<Option<IdRef<CollectionId>>> {
    fn walk(
        main: &Main,
        c: &Collection,
        out: &mut Vec<Option<IdRef<CollectionId>>>,
    ) {
        for &child in &c.children {
            if out.contains(&Some(child)) {
                continue;
            }
            out.push(Some(child));
            if let Some(cc) = main.collections.get(child) {
                walk(main, cc, out);
            }
        }
    }
    let mut out = vec![None];
    if let Some(s) = main.scenes.get(scene) {
        walk(main, &s.collection, &mut out);
    }
    out
}

fn collection(
    main: &Main,
    scene: IdRef<SceneId>,
    c: Option<IdRef<CollectionId>>,
) -> Option<&Collection> {
    match c {
        None => main.scenes.get(scene).map(|s| &s.collection),
        Some(c) => main.collections.get(c),
    }
}

/// `FOREACH_SCENE_OBJECT`: the objects of a scene, each once, in collection order.
fn scene_objects(main: &Main, scene: IdRef<SceneId>) -> Vec<IdRef<ObjectId>> {
    let mut out = Vec::new();
    for c in scene_collections(main, scene) {
        if let Some(c) = collection(main, scene, c) {
            for &ob in &c.objects {
                if !out.contains(&ob) {
                    out.push(ob);
                }
            }
        }
    }
    out
}

/// `BKE_object_scenes_users_get`: the number of scenes whose collections hold `ob`.
fn scenes_users(main: &Main, ob: IdRef<ObjectId>) -> usize {
    main.scenes
        .iter()
        .filter(|(s, _)| scene_objects(main, *s).contains(&ob))
        .count()
}

/// `single_data_needs_duplication`: more than one user, or linked.
fn needs_duplication(main: &Main, a: IdAny) -> bool {
    main.users(a) > 1 || main.header(a).is_some_and(|h| h.library.is_some())
}

fn local(main: &Main, a: IdAny) -> bool {
    main.header(a).is_some_and(|h| h.library.is_none())
}

/// `single_object_users` (without collection copies).
fn single_object_users(
    main: &mut Main,
    scene: IdRef<SceneId>,
    tagged: Option<&[IdRef<ObjectId>]>,
    log: &mut ChangeLog,
) {
    let mut map: HashMap<IdAny, IdAny> = HashMap::new();
    for ob in scene_objects(main, scene) {
        if map.contains_key(&ob.any()) || tagged.is_some_and(|t| !t.contains(&ob)) {
            continue;
        }
        if local(main, ob.any())
            && scenes_users(main, ob) > 1
            && let Some(new) = duplicate_id(main, ob.any(), log)
        {
            map.insert(ob.any(), new);
        }
    }
    if map.is_empty() {
        return;
    }
    // Other scenes keep their bases' flags under the copies (Blender rebuilds them from the
    // objects' base flags) but lose an active object that was copied (its base is freed).
    let others: Vec<IdRef<SceneId>> = main
        .scenes
        .iter()
        .map(|(r, _)| r)
        .filter(|&r| r != scene)
        .collect();
    // `BKE_libblock_relink_to_newid` on the scene (its master collection and view layers),
    // then `libblock_relink_collection`: every child collection and every object in them.
    relink_ids(main, &[scene.any()], &map, log);
    let mut ids: Vec<IdAny> = Vec::new();
    for c in scene_collections(main, scene).into_iter().flatten() {
        relink_ids(main, &[c.any()], &map, log);
        ids.push(c.any());
    }
    let mut objects: Vec<IdAny> = Vec::new();
    for c in scene_collections(main, scene) {
        if let Some(c) = collection(main, scene, c) {
            for &ob in &c.objects {
                if !objects.contains(&ob.any()) {
                    objects.push(ob.any());
                }
            }
        }
    }
    relink_ids(main, &objects, &map, log);
    for s in others {
        let still: Vec<IdRef<ObjectId>> = scene_objects(main, s);
        let Some(sc) = main.scenes.get(s) else {
            continue;
        };
        let touched = sc.view_layers.iter().any(|vl| {
            vl.bases.iter().any(|b| {
                map.contains_key(&b.object.any()) && !still.contains(&b.object)
            })
        });
        if !touched {
            continue;
        }
        if let Some(sc) = main.scenes.get_mut(s, log) {
            for vl in &mut sc.view_layers {
                if vl
                    .active
                    .is_some_and(|a| map.contains_key(&a.any()) && !still.contains(&a))
                {
                    vl.active = None;
                }
                for i in 0..vl.bases.len() {
                    let old = vl.bases[i].object;
                    let Some(new) = map.get(&old.any()).and_then(|n| n.typed::<ObjectId>())
                    else {
                        continue;
                    };
                    if !still.contains(&old)
                        && still.contains(&new)
                        && !vl.bases.iter().any(|b| b.object == new)
                    {
                        vl.bases[i].object = new;
                    }
                }
            }
        }
    }
    layers::sync_all(main, log);
}

/// `single_obdata_users`
fn single_obdata_users(main: &mut Main, obs: &[IdRef<ObjectId>], log: &mut ChangeLog) {
    for &ob in obs {
        let Some(data) = main.objects.get(ob).and_then(|o| o.data) else {
            continue;
        };
        if !local(main, ob.any()) || !needs_duplication(main, data) {
            continue;
        }
        if let Some(new) = duplicate_id(main, data, log)
            && let Some(o) = main.objects.get_mut(ob, log)
        {
            o.data = Some(new);
        }
    }
}

/// `single_mat_users`: a copy per shared slot material, assigned where the slot links
/// (`BKE_MAT_ASSIGN_EXISTING`).
fn single_mat_users(main: &mut Main, obs: &[IdRef<ObjectId>], log: &mut ChangeLog) {
    for &ob in obs {
        if !local(main, ob.any()) {
            continue;
        }
        let n = main.objects.get(ob).map_or(0, |o| slots::slot_count(main, o));
        for i in 0..n {
            let Some(ma) = main.objects.get(ob).and_then(|o| slots::slot(main, o, i).1) else {
                continue;
            };
            if !needs_duplication(main, ma.any()) {
                continue;
            }
            if let Some(new) = duplicate_id(main, ma.any(), log).and_then(|n| n.typed()) {
                slots::set_slot_material(main, ob, i, Some(new), log);
            }
        }
    }
}

/// The action of an ID's animation data (objects and the data types that have one).
fn action_of(main: &Main, a: IdAny) -> Option<IdAny> {
    let ad = match a.code {
        IdCode::OB => main.objects.get(a.typed()?)?.animation_data.as_ref(),
        IdCode::CA => main.cameras.get(a.typed()?)?.animation_data.as_ref(),
        IdCode::LA => main.lights.get(a.typed()?)?.animation_data.as_ref(),
        _ => None,
    };
    ad.and_then(|ad| ad.action).map(|r| r.any())
}

/// `BKE_animdata_duplicate_id_action` when the action is shared
/// (`single_object_action_users`, `single_objectdata_action_users`).
fn single_action_user(main: &mut Main, a: IdAny, log: &mut ChangeLog) {
    let Some(act) = action_of(main, a) else {
        return;
    };
    if !needs_duplication(main, act) {
        return;
    }
    let Some(new) = duplicate_id(main, act, log) else {
        return;
    };
    let map: HashMap<IdAny, IdAny> = [(act, new)].into_iter().collect();
    relink_ids(main, &[a], &map, log);
}

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::MakeSingleUser) -> OpResult {
    let selected = p.r#type.value == Type::SelectedObjects;
    let scene = cx.ctx().win.scene;
    // `BKE_view_layer_selected_objects_tag`: the selected bases, visible or not.
    let objects_now = |cx: &OpCtx| -> Vec<IdRef<ObjectId>> {
        let c = cx.ctx();
        if selected {
            c.selected_objects()
        } else {
            scene_objects(c.main, scene)
        }
    };
    if p.object.value {
        let tagged: Option<Vec<IdRef<ObjectId>>> = selected.then(|| {
            cx.ctx()
                .view_layer()
                .bases
                .iter()
                .filter(|b| b.selected())
                .map(|b| b.object)
                .collect()
        });
        let (main, log) = cx.main_mut();
        single_object_users(main, scene, tagged.as_deref(), log);
        log.context_changed = true;
    }
    if p.obdata.value {
        let obs = objects_now(cx);
        let (main, log) = cx.main_mut();
        single_obdata_users(main, &obs, log);
    }
    if p.material.value {
        let obs = objects_now(cx);
        let (main, log) = cx.main_mut();
        single_mat_users(main, &obs, log);
    }
    if p.animation.value {
        let obs = objects_now(cx);
        let (main, log) = cx.main_mut();
        for ob in obs {
            if local(main, ob.any()) {
                single_action_user(main, ob.any(), log);
            }
        }
    }
    if p.obdata_animation.value {
        let obs = objects_now(cx);
        let (main, log) = cx.main_mut();
        for ob in obs {
            if local(main, ob.any())
                && let Some(d) = main.objects.get(ob).and_then(|o| o.data)
            {
                single_action_user(main, d, log);
            }
        }
    }
    Ok(OpStatus::Finished)
}
