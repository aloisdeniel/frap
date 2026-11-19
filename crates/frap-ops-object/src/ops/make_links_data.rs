// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.make_links_data(type='OBDATA')`, tier T0
//! (`tests/golden/cases/scenes/links_data.py`; M3 WP 3.10, M34W0-4). Port of
//! `make_links_data_exec` and `allow_make_links_data` (`editors/object/object_relations.cc`),
//! `BKE_object_material_assign` (`blenkernel/intern/material.cc`), `BKE_animdata_copy_id`,
//! `BKE_object_link_modifiers`, `BKE_object_groups(_clear)` (`blenkernel/intern/object.cc`),
//! Blender 5.2.4, GPL-2.0-or-later. From the active object to every other selected editable
//! object (verified on 5.2.4):
//! - `OBDATA` (same type, not an empty): shares the data; the slot overrides follow the new
//!   data's material count (`BKE_object_materials_sync_length`);
//! - `MATERIAL` (both types take materials): assigns each source slot's material to the same
//!   slot of the target (`BKE_MAT_ASSIGN_USERPREF`: where the target's active slot links, the
//!   data by default), growing the target's data material list when needed;
//! - `ANIMATION`: shares the object's action (and the data's, for data of the same type);
//! - `GROUPS`: the target leaves every collection and joins the source's collections;
//! - `MODIFIERS` (neither is an empty): replaces the target's stack with copies of the
//!   source's modifiers the target type supports (no Hook or Collision);
//! - `DUPLICOLLECTION`, `CONSTRAINTS`, `FONTS`, `EFFECTS`, `LIGHT_LINKING`, `SHADOW_LINKING`
//!   copy data frap objects never have (instance collections, constraints, text, Grease
//!   Pencil, light linking), so they change nothing, as in Blender for such objects;
//! - always `FINISHED`; poll `ED_operator_object_active_objectmode` (an active, not hidden
//!   object in Object Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdAny, IdCode, IdRef, ObjectId, SceneId};
use frap_scene::id::{Collection, SlotLink, layers, slots};
use frap_scene::{ChangeLog, Context, Main, ObjectMode};
use frap_spec_ops_object::object::{self, make_links_data::Type};

/// `ED_operator_object_active_objectmode`
fn active_objectmode(c: &Context) -> Result<(), PollFail> {
    poll::active_object(c)?;
    match poll::active(c) {
        Some((_, ob)) if ob.mode != ObjectMode::Object => Err(poll::fail(
            PollReq::ModeObject,
            "bpy.ops.object.mode_set(mode='OBJECT')",
        )),
        _ => Ok(()),
    }
}

/// `ED_operator_object_active_objectmode`
const ACTIVE_OBJECTMODE: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ModeObject],
    check: active_objectmode,
};

/// `OB_TYPE_SUPPORT_MATERIAL` among frap's object types.
fn supports_material(ty: &str) -> bool {
    matches!(ty, "MESH" | "POINTCLOUD")
}

/// `allow_make_links_data`
fn allowed(ty: Type, src: &str, dst: &str) -> bool {
    match ty {
        Type::Obdata => src == dst && src != "EMPTY",
        Type::Material => supports_material(src) && supports_material(dst),
        Type::Duplicollection => dst == "EMPTY",
        Type::Animation | Type::Groups | Type::Constraints => true,
        Type::Modifiers => src != "EMPTY" && dst != "EMPTY",
        // Text and Grease Pencil objects do not exist in frap.
        Type::Fonts | Type::Effects => false,
        // `has_geometry_visibility`
        Type::LightLinking | Type::ShadowLinking => {
            let geo = |t: &str| matches!(t, "MESH" | "LIGHT" | "POINTCLOUD");
            geo(src) && geo(dst)
        }
    }
}

/// `BKE_object_materials_sync_length` after `dst.data` changed from a data with `old_len`
/// slots to one with `new_len` (frap keeps hidden overrides like `ob->mat`, §5.6).
fn sync_slots(ob: &mut frap_scene::id::Object, old_len: usize, new_len: usize) {
    if new_len == 0 {
        ob.material_slot_overrides.clear();
    } else if new_len > old_len {
        ob.material_slot_overrides.truncate(old_len);
    }
    let max_index = new_len.saturating_sub(1) as i32;
    if ob.active_material_index > max_index {
        ob.active_material_index = max_index;
    }
}

/// `BKE_object_material_assign(…, BKE_MAT_ASSIGN_USERPREF)` of `ma` to slot `i` of `ob`.
fn assign_material(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    i: usize,
    ma: Option<IdRef<frap_rna::MaterialId>>,
    log: &mut ChangeLog,
) {
    let Some(o) = main.objects.get(ob) else {
        return;
    };
    let Some(me) = o.data.and_then(|d| d.typed::<frap_rna::MeshId>()) else {
        return;
    };
    let old_len = slots::slot_count(main, o);
    // The link of the active slot (`ob->actcol`), else the preference (data).
    let link = if old_len > 0 {
        let act = o.active_material_index.clamp(0, old_len as i32 - 1) as usize;
        slots::slot(main, o, act).0
    } else {
        SlotLink::Data
    };
    if i >= old_len {
        // The data's array grows, and so does the object's (`MEM_recallocN`).
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.materials.resize(i + 1, None);
        }
        let users: Vec<IdRef<ObjectId>> = main
            .objects
            .iter()
            .filter(|(_, x)| x.data == Some(me.any()))
            .map(|(r, _)| r)
            .collect();
        for u in users {
            if let Some(x) = main.objects.get_mut(u, log) {
                x.material_slot_overrides.truncate(old_len);
            }
        }
    }
    if let Some(o) = main.objects.get_mut(ob, log) {
        slots::override_mut(o, i).link = link;
    }
    slots::set_slot_material(main, ob, i, ma, log);
}

/// `BKE_object_groups`: the collections holding `ob` (the scene collection first, then
/// `bpy.data.collections`), in Blender's prepended (reversed) order.
fn groups(
    main: &Main,
    scene: IdRef<SceneId>,
    ob: IdRef<ObjectId>,
) -> Vec<Option<IdRef<frap_rna::CollectionId>>> {
    let mut out = Vec::new();
    if main
        .scenes
        .get(scene)
        .is_some_and(|s| s.collection.objects.contains(&ob))
    {
        out.push(None);
    }
    for (r, c) in main.collections.iter() {
        if c.objects.contains(&ob) {
            out.push(Some(r));
        }
    }
    out.reverse();
    out
}

fn coll_mut<'m>(
    main: &'m mut Main,
    scene: IdRef<SceneId>,
    c: Option<IdRef<frap_rna::CollectionId>>,
    log: &mut ChangeLog,
) -> Option<&'m mut Collection> {
    match c {
        None => main.scenes.get_mut(scene, log).map(|s| &mut s.collection),
        Some(c) => main.collections.get_mut(c, log),
    }
}

/// `BKE_object_link_modifiers`: copies of the source modifiers the target supports
/// (`BKE_object_support_modifier_type_check`, no Hook or Collision).
fn link_modifiers(
    main: &mut Main,
    src: IdRef<ObjectId>,
    dst: IdRef<ObjectId>,
    log: &mut ChangeLog,
) {
    use frap_modifiers::{ModTypeFlags, ModifierType};
    let Some(s) = main.objects.get(src) else {
        return;
    };
    let dst_ty = main.objects.get(dst).map_or("EMPTY", |o| o.object_type());
    let mods: Vec<_> = s
        .modifiers
        .iter()
        .filter(|m| {
            let ty = ModifierType::from_identifier(m.settings.type_identifier());
            let id = m.settings.type_identifier();
            if matches!(id, "HOOK" | "COLLISION") {
                return false;
            }
            match dst_ty {
                "EMPTY" | "POINTCLOUD" => matches!(id, "NODES" | "MESH_SEQUENCE_CACHE"),
                "MESH" => ty.and_then(frap_modifiers::registry::find).is_none_or(|i| {
                    i.flags
                        .intersects(ModTypeFlags::ACCEPTS_MESH | ModTypeFlags::ACCEPTS_CVS)
                }),
                _ => false,
            }
        })
        .cloned()
        .collect();
    if let Some(o) = main.objects.get_mut(dst, log) {
        o.modifiers = mods;
    }
}

/// `BKE_animdata_copy_id(dst_data, src_data)` for the data types frap animates (camera,
/// light): the target data's animation data becomes a copy of the source's.
fn copy_data_anim(main: &mut Main, dst: IdAny, src: IdAny, log: &mut ChangeLog) {
    match (dst.code, src.code) {
        (IdCode::CA, IdCode::CA) => {
            let ad = src
                .typed()
                .and_then(|r| main.cameras.get(r))
                .map(|c| c.animation_data.clone());
            if let Some(ad) = ad
                && let Some(c) = dst.typed().and_then(|r| main.cameras.get_mut(r, log))
            {
                c.animation_data = ad;
            }
        }
        (IdCode::LA, IdCode::LA) => {
            let ad = src
                .typed()
                .and_then(|r| main.lights.get(r))
                .map(|l| l.animation_data.clone());
            if let Some(ad) = ad
                && let Some(l) = dst.typed().and_then(|r| main.lights.get_mut(r, log))
            {
                l.animation_data = ad;
            }
        }
        _ => {}
    }
}

#[operator(poll = ACTIVE_OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::MakeLinksData) -> OpResult {
    let ty = p.r#type.value;
    let (scene, src, dsts) = {
        let c = cx.ctx();
        let src = c
            .active_object()
            .ok_or_else(|| OpError::Runtime("no active object".into()))?;
        (c.win.scene, src, c.selected_editable_objects())
    };
    let src_groups = if ty == Type::Groups {
        groups(cx.main(), scene, src)
    } else {
        Vec::new()
    };
    let (main, log) = cx.main_mut();
    let Some(src_ob) = main.objects.get(src).cloned() else {
        return Ok(OpStatus::Finished);
    };
    let src_ty = src_ob.object_type();
    for dst in dsts {
        if dst == src {
            continue;
        }
        let Some(dst_ob) = main.objects.get(dst) else {
            continue;
        };
        if !allowed(ty, src_ty, dst_ob.object_type()) {
            continue;
        }
        match ty {
            Type::Obdata => {
                let old_len = slots::slot_count(main, dst_ob);
                let new_len = slots::slot_count(main, &src_ob);
                if let Some(o) = main.objects.get_mut(dst, log) {
                    o.data = src_ob.data;
                    sync_slots(o, old_len, new_len);
                }
            }
            Type::Material => {
                for i in 0..slots::slot_count(main, &src_ob) {
                    let ma = slots::slot(main, &src_ob, i).1;
                    assign_material(main, dst, i, ma, log);
                }
            }
            Type::Animation => {
                let dst_data = dst_ob.data;
                if let Some(o) = main.objects.get_mut(dst, log) {
                    o.animation_data = src_ob.animation_data.clone();
                }
                if let (Some(dd), Some(sd)) = (dst_data, src_ob.data)
                    && dd != sd
                {
                    copy_data_anim(main, dd, sd, log);
                }
            }
            Type::Groups => {
                for c in groups(main, scene, dst) {
                    if let Some(c) = coll_mut(main, scene, c, log) {
                        c.objects.retain(|&o| o != dst);
                    }
                }
                for &c in &src_groups {
                    if let Some(c) = coll_mut(main, scene, c, log)
                        && !c.objects.contains(&dst)
                    {
                        c.objects.push(dst);
                    }
                }
                layers::sync_all(main, log);
            }
            Type::Modifiers => link_modifiers(main, src, dst, log),
            // Nothing frap objects can hold (see the module notes).
            Type::Duplicollection
            | Type::Constraints
            | Type::Fonts
            | Type::Effects
            | Type::LightLinking
            | Type::ShadowLinking => {}
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use frap_scene::generated::modifier_settings::{ModifierSettings, ModifierType};
    use frap_scene::id::Modifier;

    use super::*;

    fn modifier(name: &str, ty: ModifierType) -> Modifier {
        Modifier {
            name: name.into(),
            show_viewport: true,
            show_render: true,
            show_in_editmode: false,
            show_on_cage: false,
            show_expanded: true,
            is_active: false,
            use_pin_to_last: false,
            persistent_uid: 7,
            settings: ModifierSettings::new(ty),
        }
    }

    #[test]
    fn allowed_follows_blender() {
        assert!(allowed(Type::Obdata, "MESH", "MESH"));
        assert!(!allowed(Type::Obdata, "MESH", "CAMERA"));
        assert!(!allowed(Type::Obdata, "EMPTY", "EMPTY"));
        assert!(allowed(Type::Material, "MESH", "MESH"));
        assert!(!allowed(Type::Material, "MESH", "CAMERA"));
        assert!(!allowed(Type::Modifiers, "MESH", "EMPTY"));
        assert!(allowed(Type::Duplicollection, "MESH", "EMPTY"));
        assert!(!allowed(Type::Fonts, "MESH", "MESH"));
        assert!(allowed(Type::LightLinking, "MESH", "LIGHT"));
        assert!(!allowed(Type::LightLinking, "MESH", "CAMERA"));
    }

    #[test]
    fn modifiers_are_replaced_by_copies_the_target_supports() {
        let (mut main, _) = frap_scene::startup::factory();
        let mut log = ChangeLog::default();
        let cube = main.objects.find("Cube").unwrap();
        let light = main.objects.find("Light").unwrap();
        let mut sphere = main.objects.get(cube).unwrap().clone();
        sphere.modifiers = vec![
            modifier("Sub", ModifierType::Subsurf),
            modifier("Hook", ModifierType::Hook),
            modifier("GN", ModifierType::Nodes),
        ];
        sphere.header = frap_rna::IdHeader::new("Src");
        let src = main.objects.insert("Src", sphere, &mut log);
        if let Some(o) = main.objects.get_mut(cube, &mut log) {
            o.modifiers = vec![modifier("Old", ModifierType::Array)];
        }
        link_modifiers(&mut main, src, cube, &mut log);
        let names: Vec<&str> = main
            .objects
            .get(cube)
            .unwrap()
            .modifiers
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        assert_eq!(names, ["Sub", "GN"]);
        assert_eq!(
            main.objects.get(cube).unwrap().modifiers[0].persistent_uid,
            7
        );
        // A light takes no modifier.
        link_modifiers(&mut main, src, light, &mut log);
        assert!(main.objects.get(light).unwrap().modifiers.is_empty());
    }

    #[test]
    fn obdata_links_keep_the_slot_rules() {
        let mut ob = frap_scene::id::Object::new("O", None);
        ob.material_slot_overrides = vec![Default::default(); 3];
        ob.active_material_index = 2;
        sync_slots(&mut ob, 3, 1);
        assert_eq!(ob.material_slot_overrides.len(), 3);
        assert_eq!(ob.active_material_index, 0);
        sync_slots(&mut ob, 1, 4);
        assert_eq!(ob.material_slot_overrides.len(), 1);
        sync_slots(&mut ob, 4, 0);
        assert!(ob.material_slot_overrides.is_empty());
    }
}
