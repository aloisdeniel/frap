// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.duplicates_make_real(use_base_parent=False, use_hierarchy=False)`, tier T0
//! (`tests/golden/cases/object/add_m8/make_real*.py`). Ported from
//! blender/source/blender/editors/object/object_add.cc (`object_duplicates_make_real_exec`,
//! `make_object_duplilist_real`, `DupliObjectHash` / `DupliObjectInstancerHash`) and
//! blenkernel/intern/object_dupli.cc (`object_duplilist`, `make_duplis_collection`,
//! `make_recursive_duplis`, `copy_dupli_context`, `make_dupli`) with
//! `BKE_collection_object_cache_get` (`blenkernel/intern/collection.cc`), Blender 5.2.4,
//! GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `ED_operator_objectmode`; each selected editable object (base order) that instances
//!   a collection (`instance_type` `COLLECTION` with an `instance_collection`, not
//!   `hide_viewport`) is made real: every object of the collection and of its child
//!   collections (link order, each once; skipped when hidden in the viewport, by itself or by
//!   every collection reaching it; the instancer itself skipped), recursively through the
//!   collections those objects instance, becomes a copy (`BKE_id_copy`: the data is shared,
//!   the name uniquified) at `instancer.matrix_world · translation(-instance_offset) ·
//!   object.matrix_world`, without parent, animation or instancing, linked to the collections
//!   of the scene holding the instancer, and selected; references between the copies (e.g. a
//!   modifier's object) point to the copies;
//! - `use_hierarchy`: a copy whose original's parent was copied from the same instance gets
//!   that copy as parent (parent type, bone, vertices and `matrix_parent_inverse` of the
//!   original); `use_base_parent`: a copy still without parent gets the instancer (or, with
//!   `use_hierarchy`, the copy of a nested instancer) as parent, type `OBJECT`, its
//!   `matrix_parent_inverse` kept; a parented copy keeps its world matrix;
//! - the instancer loses `instance_collection` and its instancing (`instance_type` `NONE`)
//!   and is deselected; the active object does not change; always `FINISHED`;
//! - Blender does not decrement the collection's user count for the instancer (D-152), frap
//!   counts users from the references; instancing on vertices or faces of a mesh with
//!   children (`instance_type` `VERTS` / `FACES`) answers `NotImplementedError`.

use std::collections::HashMap;

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CollectionId, IdRef, IdType, ObjectId};
use frap_scene::id::layers;
use frap_scene::id::transform::parent_matrix_in;
use frap_scene::{BaseFlag, Main};
use frap_spec_ops_object::object;
use glam::{Mat4, Vec3};

use super::move_to_collection::common::{all_scene_collections, has_object, link_object};
use crate::support::xform::{Worlds, apply_mat4};

/// `MAX_DUPLI_RECUR`
const MAX_DUPLI_RECUR: usize = 8;

/// A `DupliObjectHash` key: the instanced object and the persistent ids of the outer levels.
type ParentKey = (IdRef<ObjectId>, Vec<i32>);

/// One `DupliObject` of a collection instance.
#[derive(Clone, Debug, PartialEq)]
struct Dupli {
    /// The instanced object (`dob->ob`).
    ob: IdRef<ObjectId>,
    /// `dob->mat`.
    mat: Mat4,
    /// `dob->persistent_id`: the index at this level first, then the outer levels
    /// (`INT_MAX` padding left out).
    pid: Vec<i32>,
}

/// `BKE_collection_object_cache_get`: the objects of `c` and of its child collections, each
/// once in first-reached order, with `BASE_ENABLED_VIEWPORT` (reached through at least one
/// chain of collections without `hide_viewport`).
fn collection_objects(main: &Main, c: IdRef<CollectionId>) -> Vec<(IdRef<ObjectId>, bool)> {
    fn fill(
        main: &Main,
        c: IdRef<CollectionId>,
        restrict: bool,
        out: &mut Vec<(IdRef<ObjectId>, bool)>,
        depth: usize,
    ) {
        let Some(col) = main.collections.get(c) else {
            return;
        };
        if depth > 256 {
            return;
        }
        let restrict = restrict || col.hide_viewport;
        for &ob in &col.objects {
            match out.iter_mut().find(|(o, _)| *o == ob) {
                Some(e) => e.1 |= !restrict,
                None => out.push((ob, !restrict)),
            }
        }
        for &ch in &col.children {
            fill(main, ch, restrict, out, depth + 1);
        }
    }
    let mut out = Vec::new();
    fill(main, c, false, &mut out, 0);
    out
}

/// Whether `ob` generates collection duplis (`get_dupli_generator` → `gen_dupli_collection`).
/// `Err` for vertex / face instancing that would generate duplis.
fn collection_generator(main: &Main, ob: IdRef<ObjectId>) -> Result<bool, OpError> {
    let Some(o) = main.objects.get(ob) else {
        return Ok(false);
    };
    if o.instance_type == "NONE" || o.hide_viewport {
        return Ok(false);
    }
    if matches!(o.instance_type.as_str(), "VERTS" | "FACES") {
        let has_children = main.objects.iter().any(|(_, c)| c.parent == Some(ob));
        if o.object_type() == "MESH" && has_children {
            return Err(OpError::NotImplemented(format!(
                "frap: making '{}' instances real is not implemented ('{}': instancing on {} of a mesh); only collection instances are",
                o.instance_type,
                o.header.name,
                if o.instance_type == "VERTS" {
                    "vertices"
                } else {
                    "faces"
                }
            )));
        }
        return Ok(false);
    }
    Ok(o.instance_type == "COLLECTION")
}

/// `make_duplis_collection` for `inst` in the dupli context (`space`, the persistent ids of
/// the outer levels, innermost last), recursing through `make_recursive_duplis`.
fn make_duplis_collection(
    main: &Main,
    worlds: &Worlds,
    inst: IdRef<ObjectId>,
    space: Mat4,
    ctx_pid: &mut Vec<i32>,
    stack: &mut Vec<IdRef<ObjectId>>,
    out: &mut Vec<Dupli>,
) -> Result<(), OpError> {
    let Some(collection) = main.objects.get(inst).and_then(|o| o.instance_collection) else {
        return Ok(());
    };
    let Some(col) = main.collections.get(collection) else {
        return Ok(());
    };
    // Combine the collection offset and the instancer's matrix.
    let collection_mat =
        worlds.get(inst) * Mat4::from_translation(-Vec3::from(col.instance_offset));
    for (index, (cob, enabled)) in collection_objects(main, collection).into_iter().enumerate() {
        let index = index as i32;
        let hidden = main.objects.get(cob).is_none_or(|o| o.hide_viewport);
        if !enabled || hidden || cob == inst {
            continue;
        }
        let mat = space * (collection_mat * worlds.get(cob));
        let mut pid = vec![index];
        pid.extend(ctx_pid.iter().rev());
        out.push(Dupli { ob: cob, mat, pid });
        // `make_recursive_duplis(ctx, cob, collection_mat, index)`.
        if stack.contains(&cob) || ctx_pid.len() >= MAX_DUPLI_RECUR {
            continue;
        }
        if ctx_pid.len() + 1 == MAX_DUPLI_RECUR - 1 {
            // "Maximum instance recursion level reached".
            continue;
        }
        if !collection_generator(main, cob)? {
            continue;
        }
        ctx_pid.push(index);
        stack.push(cob);
        let r = make_duplis_collection(
            main,
            worlds,
            cob,
            space * collection_mat,
            ctx_pid,
            stack,
            out,
        );
        stack.pop();
        ctx_pid.pop();
        r?;
    }
    Ok(())
}

/// `object_duplilist(depsgraph, ob)` for a collection instancer.
fn duplilist(main: &Main, worlds: &Worlds, ob: IdRef<ObjectId>) -> Result<Vec<Dupli>, OpError> {
    let mut out = Vec::new();
    if !collection_generator(main, ob)? {
        return Ok(out);
    }
    make_duplis_collection(
        main,
        worlds,
        ob,
        Mat4::IDENTITY,
        &mut Vec::new(),
        &mut vec![ob],
        &mut out,
    )?;
    Ok(out)
}

/// `make_object_duplilist_real` for one instancer; returns the selection changes (copies
/// selected, the instancer deselected).
fn make_real(
    cx: &mut OpCtx,
    worlds: &Worlds,
    base: IdRef<ObjectId>,
    list: &[Dupli],
    use_base_parent: bool,
    use_hierarchy: bool,
) -> Vec<(IdRef<ObjectId>, bool)> {
    let scene = cx.ctx().win.scene;
    if list.is_empty() {
        return Vec::new();
    }
    let (main, log) = cx.main_mut();
    let mut changes = Vec::new();
    // First pass: the copies.
    let mut dst: Vec<IdRef<ObjectId>> = Vec::with_capacity(list.len());
    let mut new_world: HashMap<IdRef<ObjectId>, Mat4> = HashMap::new();
    let mut newid: HashMap<IdRef<ObjectId>, IdRef<ObjectId>> = HashMap::new();
    // `parent_gh` (first insertion wins): (object, persistent ids of the outer levels).
    let mut parent_gh: Vec<(ParentKey, IdRef<ObjectId>)> = Vec::new();
    // `instancer_gh`: full persistent ids of the duplis that instance themselves.
    let mut instancer_gh: Vec<(Vec<i32>, IdRef<ObjectId>)> = Vec::new();
    for dob in list {
        let Some(src) = main.objects.get(dob.ob) else {
            continue;
        };
        let mut o = src.clone();
        let name = o.header.name.clone();
        o.header.use_fake_user = false;
        o.header.library = None;
        o.header.is_linked_packed = false;
        o.animation_data = None;
        o.parent = None;
        let is_dupli_instancer = o.instance_type != "NONE";
        o.instance_type = "NONE".into();
        o.instance_collection = None;
        apply_mat4(&mut o, &dob.mat, false, None);
        let ob_dst = main.objects.insert(&name, o, log);
        // `BKE_collection_object_add_from(bmain, scene, base->object, ob_dst)`.
        let mut linked = false;
        for c in all_scene_collections(main, scene) {
            if has_object(main, scene, c, base) && link_object(main, log, scene, c, ob_dst) {
                linked = true;
            }
        }
        if !linked {
            link_object(main, log, scene, None, ob_dst);
        }
        changes.push((ob_dst, true));
        new_world.insert(ob_dst, dob.mat);
        newid.insert(dob.ob, ob_dst);
        dst.push(ob_dst);
        if use_hierarchy {
            let key = (dob.ob, dob.pid[1..].to_vec());
            if !parent_gh.iter().any(|(k, _)| *k == key) {
                parent_gh.push((key, ob_dst));
            }
            if is_dupli_instancer
                && use_base_parent
                && !instancer_gh.iter().any(|(k, _)| *k == dob.pid)
            {
                instancer_gh.push((dob.pid.clone(), ob_dst));
            }
        }
    }
    // Second pass: references between the copies, then the parents.
    for (dob, &ob_dst) in list.iter().zip(&dst) {
        // `BKE_libblock_relink_to_newid`.
        let hits: Vec<(IdRef<ObjectId>, IdRef<ObjectId>)> = {
            let mut v = Vec::new();
            if let Some(o) = main.objects.get(ob_dst) {
                o.for_each_ref(&mut |a| {
                    if let Some(r) = a.typed::<ObjectId>()
                        && let Some(&n) = newid.get(&r)
                        && !v.contains(&(r, n))
                    {
                        v.push((r, n));
                    }
                });
            }
            v
        };
        if !hits.is_empty()
            && let Some(o) = main.objects.get_mut(ob_dst, log)
        {
            for (from, to) in hits {
                o.remap_ref(from.any(), Some(to.any()));
            }
        }
        let src = main.objects.get(dob.ob).cloned();
        let mut parent: Option<IdRef<ObjectId>> = None;
        if use_hierarchy
            && let Some(src) = &src
            && let Some(src_par) = src.parent
        {
            let key = (src_par, dob.pid[1..].to_vec());
            if let Some((_, par)) = parent_gh.iter().find(|(k, _)| *k == key) {
                parent = Some(*par);
                if let Some(o) = main.objects.get_mut(ob_dst, log) {
                    o.parent_type = src.parent_type.clone();
                    o.parent_bone = src.parent_bone.clone();
                    o.parent_vertices = src.parent_vertices;
                    o.matrix_parent_inverse = src.matrix_parent_inverse;
                    o.parent = Some(*par);
                }
            }
        }
        if use_base_parent && parent.is_none() {
            let par = instancer_gh
                .iter()
                .find(|(k, _)| k[..] == dob.pid[1..])
                .map(|(_, o)| *o)
                .unwrap_or(base);
            parent = Some(par);
            if let Some(o) = main.objects.get_mut(ob_dst, log) {
                o.parent = Some(par);
                o.parent_type = "OBJECT".into();
            }
        }
        if let Some(par) = parent {
            let par_world = new_world
                .get(&par)
                .copied()
                .unwrap_or_else(|| worlds.get(par));
            let pm = match (main.objects.get(ob_dst), main.objects.get(par)) {
                (Some(o), Some(po)) => parent_matrix_in(main, o, po, par_world),
                _ => par_world,
            };
            if let Some(o) = main.objects.get_mut(ob_dst, log) {
                apply_mat4(o, &dob.mat, false, Some(&pm));
            }
        }
    }
    if let Some(o) = main.objects.get_mut(base, log) {
        if o.instance_type == "COLLECTION" {
            o.instance_collection = None;
        }
        o.instance_type = "NONE".into();
    }
    changes.push((base, false));
    changes
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    support = Partial("collection instances only: vertex and face instancing answer OP_NOT_IMPLEMENTED"),
    notes = ["copies share their data with the instanced objects (BKE_id_copy), as in Blender"]
)]
fn exec(cx: &mut OpCtx, p: object::DuplicatesMakeReal) -> OpResult {
    let (bases, worlds) = {
        let c = cx.ctx();
        (c.selected_editable_objects(), Worlds::of(c.main))
    };
    // Every duplilist from the state before the operator (the evaluated depsgraph).
    let lists = bases
        .iter()
        .map(|&b| duplilist(cx.main(), &worlds, b))
        .collect::<Result<Vec<_>, _>>()?;
    let mut changes: Vec<(IdRef<ObjectId>, bool)> = Vec::new();
    for (base, list) in bases.into_iter().zip(&lists) {
        changes.extend(make_real(
            cx,
            &worlds,
            base,
            list,
            p.use_base_parent.value,
            p.use_hierarchy.value,
        ));
    }
    if changes.is_empty() {
        return Ok(OpStatus::Finished);
    }
    let (scene, view_layer) = {
        let c = cx.ctx();
        (c.win.scene, c.view_layer().name.clone())
    };
    let (main, log) = cx.main_mut();
    layers::sync_all(main, log);
    // `base_select`: selecting needs `BASE_SELECTABLE`.
    let vis = main
        .scenes
        .get(scene)
        .and_then(|s| {
            s.view_layers
                .iter()
                .find(|v| v.name == view_layer)
                .map(|vl| layers::visibility(s, vl, &main.collections, &main.objects))
        })
        .unwrap_or_default();
    if let Some(s) = main.scenes.get_mut(scene, log)
        && let Some(vl) = s.view_layers.iter_mut().find(|v| v.name == view_layer)
    {
        for (ob, state) in changes {
            let selectable = vis.get(&ob).is_some_and(|v| v.selectable);
            if let Some(b) = vl.base_mut(ob)
                && (!state || selectable)
            {
                b.flag.set(BaseFlag::SELECTED, state);
            }
        }
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::id::{Collection, Object};
    use frap_scene::ChangeLog;

    #[test]
    fn cache_lists_each_object_once_and_ors_the_restriction() {
        let mut main = Main::default();
        let mut log = ChangeLog::default();
        let a = main.objects.insert("A", Object::new("A", None), &mut log);
        let b = main.objects.insert("B", Object::new("B", None), &mut log);
        let mut sub = Collection::new("Sub");
        sub.objects = vec![b, a];
        sub.hide_viewport = true;
        let sub = main.collections.insert("Sub", sub, &mut log);
        let mut top = Collection::new("Top");
        top.objects = vec![a];
        top.children = vec![sub];
        let top = main.collections.insert("Top", top, &mut log);
        assert_eq!(collection_objects(&main, top), vec![(a, true), (b, false)]);
    }
}
