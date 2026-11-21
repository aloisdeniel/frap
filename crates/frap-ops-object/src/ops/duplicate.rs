// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.duplicate(linked=False, mode='TRANSLATION')`, tier T0
//! (`tests/golden/cases/object/duplicate/*.py`). Port of `duplicate_exec`,
//! `object_add_duplicate_internal`, `BKE_object_duplicate` and
//! `BKE_collection_object_add_from` (`editors/object/object_add.cc`,
//! `blenkernel/intern/object.cc`, `blenkernel/intern/collection.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4:
//! - duplicates `context.selected_objects` in base order; nothing selected → `CANCELLED`;
//! - each copy keeps the name (uniquified: `Cube.001`), transform, parent, modifiers, slot
//!   overrides and display settings, and is linked to every collection of the scene that holds
//!   the original (master included);
//! - the factory "Duplicate Data" preferences decide what is copied: mesh, camera, light,
//!   armature, point cloud data are copied (each datablock once per call, so copies of two
//!   objects sharing a mesh share the new mesh) with their shape keys, materials are
//!   **shared**; `linked=True` shares the object data too (and copies nothing else);
//! - `USER_DUP_ACT`: the active action of each copied object, object data and shape key is
//!   replaced by its own copy (two IDs sharing an action get two copies); NLA strip actions are
//!   not copied but remapped (next point), so a strip shares the copy of an action that was
//!   also active, and keeps the original otherwise;
//! - every reference of a copy to a datablock copied in the same call points to the copy
//!   (`BKE_libblock_relink_to_newid`: parents, modifier operands such as an Armature
//!   modifier's object, NLA strip actions; an action copied twice maps to its last copy),
//!   otherwise it keeps the original;
//! - in the context view layer the originals are deselected and the copies selected; the
//!   copy of the active object becomes active (an unselected or no active object stays);
//! - `mode` is only read by the interactive transform that follows (`duplicate_move`).
//!
//! Objects in Pose Mode are skipped as in Blender (Pose Mode arrives in M6).

use std::collections::HashMap;

use frap_anim::AnimData;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdAny, IdCode, IdRef, IdType, MeshId, ObjectId};
use frap_scene::id::layers;
use frap_scene::{BaseFlag, ChangeLog, IdMap, Main, ObjectMode};
use frap_spec_ops_object::object;

use super::move_to_collection::common::{all_scene_collections, has_object, link_object};

/// A local copy of an ID in its map (`BKE_id_copy_for_duplicate`): same requested name
/// (uniquified), fake user, library and packed flags cleared.
fn copy_in<T: IdType + Clone>(m: &mut IdMap<T>, a: IdAny, log: &mut ChangeLog) -> Option<IdAny> {
    let r = a.typed::<T::Kind>()?;
    let mut v = m.get(r)?.clone();
    let name = v.header().name.clone();
    let h = v.header_mut();
    h.use_fake_user = false;
    h.library = None;
    h.is_linked_packed = false;
    Some(m.insert(&name, v, log).any())
}

/// The `AnimData` of a datablock duplicate copies (`BKE_animdata_from_id`), bumped.
fn adt_mut<'m>(main: &'m mut Main, log: &mut ChangeLog, a: IdAny) -> Option<&'m mut AnimData> {
    let adt = match a.code {
        IdCode::OB => &mut main.objects.get_mut(a.typed()?, log)?.animation_data,
        IdCode::CA => &mut main.cameras.get_mut(a.typed()?, log)?.animation_data,
        IdCode::LA => &mut main.lights.get_mut(a.typed()?, log)?.animation_data,
        IdCode::AR => &mut main.armatures.get_mut(a.typed()?, log)?.animation_data,
        IdCode::PT => &mut main.pointclouds.get_mut(a.typed()?, log)?.animation_data,
        IdCode::KE => &mut main.shape_keys.get_mut(a.typed()?, log)?.animation_data,
        _ => return None,
    };
    adt.as_mut()
}

/// The datablocks copied in this call and their copies (`ID.newid`, `ID_TAG_NEW`).
#[derive(Default)]
struct NewIds {
    /// Original -> copy. An action copied more than once maps to its last copy, as
    /// `ID_NEW_SET` overwrites `newid`.
    map: HashMap<IdAny, IdAny>,
    /// Every copy, in creation order (`ID_TAG_NEW`).
    created: Vec<IdAny>,
}

impl NewIds {
    /// `BKE_id_copy_for_duplicate`: the copy of `a` (made once per call: shared object data
    /// is copied once), with its shape keys (`BKE_key_from_id`), and, when `dup_act`
    /// (`USER_DUP_ACT`), a copy of the active action of the copy and of its shape keys
    /// (`BKE_animdata_duplicate_id_action`).
    fn duplicate(
        &mut self,
        main: &mut Main,
        log: &mut ChangeLog,
        a: IdAny,
        dup_act: bool,
    ) -> IdAny {
        if let Some(n) = self.map.get(&a) {
            return *n;
        }
        let new = match a.code {
            IdCode::OB => copy_in(&mut main.objects, a, log),
            IdCode::ME => copy_in(&mut main.meshes, a, log),
            IdCode::CA => copy_in(&mut main.cameras, a, log),
            IdCode::LA => copy_in(&mut main.lights, a, log),
            IdCode::AR => copy_in(&mut main.armatures, a, log),
            IdCode::PT => copy_in(&mut main.pointclouds, a, log),
            _ => None,
        };
        let Some(new) = new else {
            return a;
        };
        self.map.insert(a, new);
        self.created.push(new);
        // A mesh copy owns a copy of its shape keys.
        let mut key_new = None;
        if let Some(n) = new.typed::<MeshId>()
            && let Some(k) = main.meshes.get(n).and_then(|m| m.shape_keys)
            && let Some(k2) = copy_in(&mut main.shape_keys, k.any(), log)
        {
            if let Some(m) = main.meshes.get_mut(n, log) {
                m.shape_keys = k2.typed();
            }
            if let Some(key) = k2.typed().and_then(|k| main.shape_keys.get_mut(k, log)) {
                key.user = Some(n);
            }
            self.map.insert(k.any(), k2);
            self.created.push(k2);
            key_new = Some(k2);
        }
        if dup_act {
            self.duplicate_action(main, log, new);
            if let Some(k2) = key_new {
                self.duplicate_action(main, log, k2);
            }
        }
        new
    }

    /// `animdata_copy_id_action`: the active action of `id` is replaced by a fresh copy (one
    /// per ID, even when IDs share the action: verified on 5.2.4), recorded as the action's
    /// `newid`. NLA strip actions are not copied here; [`NewIds::relink`] remaps them.
    fn duplicate_action(&mut self, main: &mut Main, log: &mut ChangeLog, id: IdAny) {
        let Some(act) = adt_mut(main, log, id).and_then(|ad| ad.action) else {
            return;
        };
        let Some(copied) = copy_in(&mut main.actions, act.any(), log) else {
            return;
        };
        self.map.insert(act.any(), copied);
        self.created.push(copied);
        if let Some(ad) = adt_mut(main, log, id) {
            ad.action = copied.typed();
        }
    }

    /// `BKE_libblock_relink_to_newid` on every copy (`copy_object_set_idnew`): each reference
    /// of a copy to a datablock copied in this call points to its (last) copy: parents,
    /// modifier operands, object data, active and NLA strip actions (verified on 5.2.4: a
    /// strip of a duplicated object uses the copy of its action when the object's or its
    /// data's active action was copied, the original otherwise).
    fn relink(&self, main: &mut Main, log: &mut ChangeLog) {
        fn go<T: IdType>(
            m: &mut IdMap<T>,
            a: IdAny,
            map: &HashMap<IdAny, IdAny>,
            log: &mut ChangeLog,
        ) {
            let Some(r) = a.typed::<T::Kind>() else {
                return;
            };
            let mut hits: Vec<(IdAny, IdAny)> = Vec::new();
            if let Some(t) = m.get(r) {
                t.for_each_ref(&mut |x| {
                    if let Some(&n) = map.get(&x)
                        && !hits.contains(&(x, n))
                    {
                        hits.push((x, n));
                    }
                });
            }
            if hits.is_empty() {
                return;
            }
            if let Some(t) = m.get_mut(r, log) {
                for (from, to) in hits {
                    t.remap_ref(from, Some(to));
                }
            }
        }
        for &id in &self.created {
            match id.code {
                IdCode::OB => go(&mut main.objects, id, &self.map, log),
                IdCode::CA => go(&mut main.cameras, id, &self.map, log),
                IdCode::LA => go(&mut main.lights, id, &self.map, log),
                IdCode::AR => go(&mut main.armatures, id, &self.map, log),
                IdCode::PT => go(&mut main.pointclouds, id, &self.map, log),
                IdCode::KE => go(&mut main.shape_keys, id, &self.map, log),
                _ => {}
            }
            let strips = |main: &Main| {
                let mut any = false;
                let read = match id.code {
                    IdCode::OB => id
                        .typed()
                        .and_then(|r| main.objects.get(r))
                        .and_then(|o| o.animation_data.as_ref()),
                    IdCode::CA => id
                        .typed()
                        .and_then(|r| main.cameras.get(r))
                        .and_then(|o| o.animation_data.as_ref()),
                    IdCode::LA => id
                        .typed()
                        .and_then(|r| main.lights.get(r))
                        .and_then(|o| o.animation_data.as_ref()),
                    IdCode::AR => id
                        .typed()
                        .and_then(|r| main.armatures.get(r))
                        .and_then(|o| o.animation_data.as_ref()),
                    IdCode::PT => id
                        .typed()
                        .and_then(|r| main.pointclouds.get(r))
                        .and_then(|o| o.animation_data.as_ref()),
                    IdCode::KE => id
                        .typed()
                        .and_then(|r| main.shape_keys.get(r))
                        .and_then(|o| o.animation_data.as_ref()),
                    _ => None,
                };
                for s in read
                    .iter()
                    .flat_map(|a| &a.nla_tracks)
                    .flat_map(|t| &t.strips)
                {
                    any |= s.action.is_some_and(|a| self.map.contains_key(&a.any()));
                }
                any
            };
            if strips(main)
                && let Some(ad) = adt_mut(main, log, id)
            {
                for s in ad.nla_tracks.iter_mut().flat_map(|t| &mut t.strips) {
                    if let Some(n) = s.action.and_then(|a| self.map.get(&a.any())) {
                        s.action = n.typed();
                    }
                }
            }
        }
    }
}

/// `duplicate_exec` (also the first step of the `duplicate_move*` macros).
fn duplicate(cx: &mut OpCtx, linked: bool) -> OpResult {
    let (selected, active, scene, layer) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        (c.selected_objects(), c.active_object(), c.win.scene, layer)
    };
    let (main, log) = cx.main_mut();
    let mut ids = NewIds::default();
    let mut pairs: Vec<(IdRef<ObjectId>, IdRef<ObjectId>)> = Vec::new();
    for ob in selected {
        let Some(src) = main.objects.get(ob) else {
            continue;
        };
        if src.mode == ObjectMode::Pose {
            continue;
        }
        let data = src.data;
        // `BKE_object_duplicate`: `linked=True` duplicates with no flags (nothing but the
        // object is copied, no action either).
        let Some(new) = ids
            .duplicate(main, log, ob.any(), !linked)
            .typed::<ObjectId>()
        else {
            continue;
        };
        if new == ob {
            continue;
        }
        if !linked && let Some(d) = data {
            let new_data = ids.duplicate(main, log, d, true);
            if let Some(o) = main.objects.get_mut(new, log) {
                o.data = Some(new_data);
            }
        }
        // `BKE_collection_object_add_from`: every collection of the scene holding the
        // original, or the master collection when none (linked collections only).
        let mut linked_somewhere = false;
        for c in all_scene_collections(main, scene) {
            if has_object(main, scene, c, ob) && link_object(main, log, scene, c, new) {
                linked_somewhere = true;
            }
        }
        if !linked_somewhere {
            link_object(main, log, scene, None, new);
        }
        pairs.push((ob, new));
    }
    if pairs.is_empty() {
        cx.report(
            ReportType::Warning,
            "frap: nothing duplicated: no visible object is selected",
        );
        return Ok(OpStatus::Cancelled);
    }
    // `copy_object_set_idnew`: references between the copies point to the copies.
    ids.relink(main, log);
    layers::sync_all(main, log);
    // Selection in the context view layer.
    let vis = main
        .scenes
        .get(scene)
        .map(|s| layers::visibility(s, &s.view_layers[layer], &main.collections, &main.objects))
        .unwrap_or_default();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        let vl = &mut s.view_layers[layer];
        for &(old, new) in &pairs {
            if let Some(b) = vl.base_mut(old) {
                b.flag.remove(BaseFlag::SELECTED);
            }
            if vis.get(&new).is_some_and(|v| v.selectable)
                && let Some(b) = vl.base_mut(new)
            {
                b.flag.insert(BaseFlag::SELECTED);
            }
        }
        if let Some(a) = active
            && let Some(&(_, na)) = pairs.iter().find(|(old, _)| *old == a)
            && vl.base(na).is_some()
        {
            vl.active = Some(na);
        }
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    notes = ["materials are shared, mesh/camera/light/armature data, shape keys and active actions copied (factory Duplicate Data preferences); references between copies (parents, modifier operands, NLA strip actions) remapped"]
)]
fn exec(cx: &mut OpCtx, p: object::Duplicate) -> OpResult {
    duplicate(cx, p.linked.value)
}
