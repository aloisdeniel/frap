// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.collection_instance_add(name='Collection', collection='', align='WORLD',
//! location=…, rotation=…, scale=…, session_uid=0, drop_x=0, drop_y=0)`, tier T0
//! (`tests/golden/cases/object/add_m8/collection_instance*.py`). Ported from
//! blender/source/blender/editors/object/object_add.cc (`collection_add_info_get_from_op`,
//! `collection_instance_add_exec`) and blenkernel/intern/collection.cc
//! (`BKE_collection_cycle_find`, `collection_instance_find_recursive`), Blender 5.2.4,
//! GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `ED_operator_objectmode`; the collection is looked up by `session_uid` when set,
//!   else by `name` when set (the default `'Collection'` counts only when passed); otherwise,
//!   or when that lookup fails, the `collection` enum (the names of `bpy.data.collections`,
//!   the first one when unset) picks it; no collection at all → `CANCELLED`;
//! - when the active layer collection would contain its own instance (it is the instanced
//!   collection, below it, or holds an object instancing it), the active layer collection moves
//!   to its closest visible ancestor (this sticks after the operator), so the instance goes
//!   there;
//! - the new empty is named after the collection, placed like the other add operators with
//!   its `scale` (unset: 1), `instance_type` `COLLECTION`, `instance_collection` the
//!   collection, `empty_display_size` 1 (the preference `collection_instance_empty_size`); it
//!   is selected and active, the others deselected;
//! - `drop_x` / `drop_y` (placement under the mouse, used only for a collection found by name
//!   or `session_uid` with `location` unset) answer `NotImplementedError` (deviations §2).

use frap_op::EnumItemOwned;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CollectionId, IdRef, SceneId};
use frap_scene::{Context, Main};
use frap_spec_ops_object::object;

use super::add_named::lookup;
use super::hide_collection::layer::activate_parent;
use super::move_to_collection::common::SceneColl;

/// `RNA_collection_itemf`: every collection of `bpy.data.collections`, by name.
fn collection_items(c: &Context) -> Vec<EnumItemOwned> {
    c.main
        .collections
        .iter()
        .map(|(_, col)| EnumItemOwned {
            identifier: col.header.name.clone(),
            name: col.header.name.clone(),
            description: Default::default(),
        })
        .collect()
}

/// The parents of a collection (`collection->runtime->parents`): the collections and the
/// scenes' master collections listing it as a child.
fn parents(main: &Main, c: SceneColl) -> Vec<(Option<IdRef<SceneId>>, SceneColl)> {
    let Some(c) = c else {
        return Vec::new();
    };
    let mut out: Vec<(Option<IdRef<SceneId>>, SceneColl)> = main
        .collections
        .iter()
        .filter(|(_, p)| p.children.contains(&c))
        .map(|(r, _)| (None, Some(r)))
        .collect();
    out.extend(
        main.scenes
            .iter()
            .filter(|(_, s)| s.collection.children.contains(&c))
            .map(|(r, _)| (Some(r), None)),
    );
    out
}

/// `collection_instance_find_recursive(collection, instance_collection)`: an object of
/// `collection` or below instances `instance_collection` (never the master) or the
/// collection holding it.
fn instance_find_recursive(
    main: &Main,
    collection: IdRef<CollectionId>,
    instance: SceneColl,
    depth: usize,
) -> bool {
    let Some(col) = main.collections.get(collection) else {
        return false;
    };
    if depth > 256 {
        return false;
    }
    for &ob in &col.objects {
        let ic = main.objects.get(ob).and_then(|o| o.instance_collection);
        if ic.is_some() && (ic == instance || ic == Some(collection)) {
            return true;
        }
    }
    col.children
        .iter()
        .any(|&ch| instance_find_recursive(main, ch, instance, depth + 1))
}

/// `BKE_collection_cycle_find(new_ancestor, collection)`.
fn cycle_find(main: &Main, new_ancestor: SceneColl, collection: IdRef<CollectionId>) -> bool {
    fn go(main: &Main, anc: SceneColl, c: IdRef<CollectionId>, depth: usize) -> bool {
        if anc == Some(c) {
            return true;
        }
        if depth > 256 {
            return false;
        }
        if parents(main, anc)
            .into_iter()
            .any(|(_, p)| go(main, p, c, depth + 1))
        {
            return true;
        }
        instance_find_recursive(main, c, anc, 0)
    }
    go(main, new_ancestor, collection, 0)
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    support = Partial("drop_x/drop_y (placement under the mouse) answer OP_NOT_IMPLEMENTED: the virtual 3D View has no region"),
    notes = [
        "the collection comes from session_uid, else name (when passed), else the collection enum",
        "the instance goes to the closest visible ancestor of the active collection when that one is part of the instanced collection (the active collection moves there)",
    ],
    dyn_enums = [("collection", collection_items)]
)]
fn exec(cx: &mut OpCtx, p: object::CollectionInstanceAdd) -> OpResult {
    let found = lookup::id(&cx.main().collections, &p.name, &p.session_uid);
    if found.is_some() && !p.location.is_set && lookup::drop_is_set(&p.drop_x, &p.drop_y) {
        return Err(OpError::NotImplemented(
            "frap: collection_instance_add(drop_x=…, drop_y=…) places the instance under the mouse in a 3D View region, which frap's virtual 3D View does not have; pass location=… instead".into(),
        ));
    }
    let target = found.or_else(|| {
        let main = cx.main();
        let index = if p.collection.is_set {
            main.collections
                .iter()
                .position(|(_, c)| c.header.name == p.collection.value)?
        } else {
            0
        };
        main.collections.iter().nth(index).map(|(r, _)| r)
    });
    let Some(target) = target else {
        cx.report(
            ReportType::Warning,
            "frap: nothing to instance: bpy.data.collections is empty",
        );
        return Ok(OpStatus::Cancelled);
    };
    let align = helpers::Align::from_identifier(p.align.value.identifier())
        .unwrap_or(helpers::Align::World);
    let at = super::camera_add::common::placement(cx, &p.location, &p.rotation, align, &p.scale)?;
    // "Avoid dependency cycles": the active layer collection climbs to a visible ancestor.
    let (scene, layer, new_active) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        let mut path = vl.active_layer_collection.clone();
        let mut steps = 0;
        while steps < 256
            && cycle_find(
                c.main,
                vl.layer_collection.at(&path).and_then(|lc| lc.collection),
                target,
            )
        {
            if path.0.is_empty() {
                break;
            }
            path = activate_parent(vl, &c.main.collections, &path);
            steps += 1;
        }
        let changed = path != vl.active_layer_collection;
        (c.win.scene, layer, changed.then_some(path))
    };
    if let Some(path) = new_active {
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.view_layers[layer].active_layer_collection = path;
        }
        log.context_changed = true;
    }
    let name = cx
        .main()
        .collections
        .get(target)
        .map(|c| c.header.name.clone())
        .unwrap_or_default();
    let ob = helpers::add_object_with_data(cx, &name, None, at, false)?;
    let o = cx.object_mut(ob)?;
    // `add_type()` does not have scale argument so copy that value separately.
    o.scale = at.scale;
    o.instance_collection = Some(target);
    o.empty_display_size = 1.0;
    o.instance_type = "COLLECTION".into();
    cx.result("object", ob.any());
    Ok(OpStatus::Finished)
}
