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
//!   armature, point cloud data and actions are copied (each datablock once per call, so
//!   copies of two objects sharing a mesh share the new mesh), materials are **shared**;
//!   `linked=True` shares the object data too (and copies nothing else);
//! - a copy whose parent was duplicated in the same call gets the parent's copy
//!   (`BKE_libblock_relink_to_newid`), otherwise it keeps the original parent;
//! - in the context view layer the originals are deselected and the copies selected; the
//!   copy of the active object becomes active (an unselected or no active object stays);
//! - `mode` is only read by the interactive transform that follows (`duplicate_move`).
//!
//! Objects in Pose Mode are skipped as in Blender (Pose Mode arrives in M6).

use std::collections::HashMap;

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{ActionId, IdAny, IdCode, IdRef, IdType, ObjectId};
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

/// IDs already copied in this call (`ID.newid`), so shared data is copied once.
#[derive(Default)]
struct NewIds(HashMap<IdAny, IdAny>);

impl NewIds {
    /// The copy of the object data `a` (`USER_DUP_MESH | CAMERA | LAMP | ARM | POINTCLOUD`).
    fn data(&mut self, main: &mut Main, log: &mut ChangeLog, a: IdAny) -> IdAny {
        if let Some(n) = self.0.get(&a) {
            return *n;
        }
        let new = match a.code {
            IdCode::ME => {
                let new = copy_in(&mut main.meshes, a, log);
                // A mesh copy owns a copy of its shape keys.
                if let Some(n) = new.and_then(|n| n.typed::<frap_rna::MeshId>())
                    && let Some(k) = main.meshes.get(n).and_then(|m| m.shape_keys)
                    && let Some(k2) = copy_in(&mut main.shape_keys, k.any(), log)
                    && let Some(m) = main.meshes.get_mut(n, log)
                {
                    m.shape_keys = k2.typed();
                }
                new
            }
            IdCode::CA => {
                let new = copy_in(&mut main.cameras, a, log);
                self.data_action(main, log, new)
            }
            IdCode::LA => {
                let new = copy_in(&mut main.lights, a, log);
                self.data_action(main, log, new)
            }
            IdCode::AR => copy_in(&mut main.armatures, a, log),
            IdCode::PT => copy_in(&mut main.pointclouds, a, log),
            _ => None,
        };
        let new = new.unwrap_or(a);
        self.0.insert(a, new);
        new
    }

    /// `USER_DUP_ACT` on copied camera and light data: their action is copied too.
    fn data_action(
        &mut self,
        main: &mut Main,
        log: &mut ChangeLog,
        new: Option<IdAny>,
    ) -> Option<IdAny> {
        let n = new?;
        let action = match n.code {
            IdCode::CA => main
                .cameras
                .get(n.typed()?)
                .and_then(|c| c.animation_data.as_ref()?.action),
            IdCode::LA => main
                .lights
                .get(n.typed()?)
                .and_then(|l| l.animation_data.as_ref()?.action),
            _ => None,
        };
        if let Some(act) = action {
            let copied = self.action(main, log, act);
            match n.code {
                IdCode::CA => {
                    if let Some(ad) = main
                        .cameras
                        .get_mut(n.typed()?, log)
                        .and_then(|c| c.animation_data.as_mut())
                    {
                        ad.action = Some(copied);
                    }
                }
                IdCode::LA => {
                    if let Some(ad) = main
                        .lights
                        .get_mut(n.typed()?, log)
                        .and_then(|l| l.animation_data.as_mut())
                    {
                        ad.action = Some(copied);
                    }
                }
                _ => {}
            }
        }
        Some(n)
    }

    /// The copy of an action (`USER_DUP_ACT`).
    fn action(
        &mut self,
        main: &mut Main,
        log: &mut ChangeLog,
        a: IdRef<ActionId>,
    ) -> IdRef<ActionId> {
        if let Some(n) = self.0.get(&a.any()).and_then(|n| n.typed()) {
            return n;
        }
        let new = copy_in(&mut main.actions, a.any(), log)
            .and_then(|n| n.typed())
            .unwrap_or(a);
        self.0.insert(a.any(), new.any());
        new
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
        let (data, action) = (src.data, src.animation_data.as_ref().and_then(|a| a.action));
        let Some(new) = copy_in(&mut main.objects, ob.any(), log).and_then(|n| n.typed()) else {
            continue;
        };
        if !linked {
            let new_data = data.map(|d| ids.data(main, log, d));
            let new_action = action.map(|a| ids.action(main, log, a));
            if let Some(o) = main.objects.get_mut(new, log) {
                o.data = new_data;
                if let (Some(ad), Some(a)) = (o.animation_data.as_mut(), new_action) {
                    ad.action = Some(a);
                }
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
    for &(_, new) in &pairs {
        let parent = main.objects.get(new).and_then(|o| o.parent);
        if let Some(p) = parent
            && let Some(&(_, np)) = pairs.iter().find(|(old, _)| *old == p)
            && let Some(o) = main.objects.get_mut(new, log)
        {
            o.parent = Some(np);
        }
    }
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
    notes = ["materials are shared, mesh/camera/light data and actions copied (factory Duplicate Data preferences)"]
)]
fn exec(cx: &mut OpCtx, p: object::Duplicate) -> OpResult {
    duplicate(cx, p.linked.value)
}
