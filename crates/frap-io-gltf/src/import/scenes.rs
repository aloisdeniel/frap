// SPDX-License-Identifier: GPL-3.0-or-later
//! Scene-level import: the Blender scenes and collections the glTF scenes map to, linking
//! objects, view-layer bases, selection, the active object and the exclusion of non-default
//! scenes.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/scene.py` (`BlenderScene.create`,
//! `select_imported_objects`, `set_active_object`) and the scene/collection part of
//! `blender/imp/vnode.py` (`init_vnodes`), Apache-2.0, The glTF-Blender-IO authors. The base
//! order follows `BKE_layer_collection_sync` (blender/source/blender/blenkernel/intern/layer.cc,
//! GPL-2.0-or-later): child collections first (depth first, in `children` order), then the
//! collection's own objects; excluded collections contribute no bases (verified on 5.2.4).

use frap_rna::{CollectionId, IdRef, ObjectId, SceneId};
use frap_scene::id::{Collection, Scene};
use frap_scene::{
    Base, BaseFlag, ChangeLog, LayerCollection, LayerCollectionPath, Main, ViewLayer,
};
use smol_str::SmolStr;

use super::vnode::VKind;
use super::{Gltf, ImportError, extras, nodes};

/// A collection objects can be linked into: a scene's embedded master collection or a
/// `bpy.data.collections` ID.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CollTarget {
    /// `scene.collection`
    Master(IdRef<SceneId>),
    /// `bpy.data.collections[…]`
    Coll(IdRef<CollectionId>),
}

fn collection_mut<'m>(
    main: &'m mut Main,
    t: CollTarget,
    log: &mut ChangeLog,
) -> Option<&'m mut Collection> {
    match t {
        CollTarget::Master(s) => main.scenes.get_mut(s, log).map(|s| &mut s.collection),
        CollTarget::Coll(c) => main.collections.get_mut(c, log),
    }
}

/// `collection.objects.link(ob)` (no-op when already linked).
pub(crate) fn link_object(
    main: &mut Main,
    t: CollTarget,
    ob: IdRef<ObjectId>,
    log: &mut ChangeLog,
) {
    if let Some(c) = collection_mut(main, t, log)
        && !c.objects.contains(&ob)
    {
        c.objects.push(ob);
    }
}

/// `collection.children.link(child)`.
fn link_child(main: &mut Main, t: CollTarget, child: IdRef<CollectionId>, log: &mut ChangeLog) {
    if let Some(c) = collection_mut(main, t, log)
        && !c.children.contains(&child)
    {
        c.children.push(child);
    }
}

/// `bpy.data.collections.new(name)` linked under `parent`.
fn new_collection(
    main: &mut Main,
    name: &str,
    parent: CollTarget,
    log: &mut ChangeLog,
) -> IdRef<CollectionId> {
    let c = main.collections.insert(name, Collection::new(""), log);
    link_child(main, parent, c, log);
    c
}

/// `bpy.data.scenes.new(name)`: an empty scene with one "ViewLayer".
fn new_scene(main: &mut Main, name: &str, log: &mut ChangeLog) -> IdRef<SceneId> {
    let vl = ViewLayer {
        name: "ViewLayer".into(),
        bases: Vec::new(),
        active: None,
        layer_collection: LayerCollection::new(None),
        active_layer_collection: LayerCollectionPath::default(),
    };
    main.scenes.insert(name, Scene::new("", vec![vl]), log)
}

/// The view layer named `name` (the first one when missing, as `Context::view_layer` does).
fn view_layer<'s>(scene: &'s Scene, name: &str) -> Option<&'s ViewLayer> {
    scene
        .view_layers
        .iter()
        .find(|v| v.name == name)
        .or_else(|| scene.view_layers.first())
}

fn view_layer_mut<'s>(scene: &'s mut Scene, name: &str) -> Option<&'s mut ViewLayer> {
    let i = scene
        .view_layers
        .iter()
        .position(|v| v.name == name)
        .unwrap_or(0);
    scene.view_layers.get_mut(i)
}

/// `bpy.context.collection`: the active layer collection's collection.
fn active_collection(main: &Main, scene: IdRef<SceneId>, vl: &str) -> CollTarget {
    let Some(s) = main.scenes.get(scene) else {
        return CollTarget::Master(scene);
    };
    view_layer(s, vl)
        .and_then(|v| v.layer_collection.at(&v.active_layer_collection))
        .and_then(|lc| lc.collection)
        .map(CollTarget::Coll)
        .unwrap_or(CollTarget::Master(scene))
}

/// Rebuilds the layer-collection tree of `old` for `collection`'s children, keeping the flags
/// of layer collections that still exist.
fn sync_layer_collection(main: &Main, coll: &Collection, old: &LayerCollection) -> LayerCollection {
    let mut lc = LayerCollection {
        children: Vec::new(),
        ..old.clone()
    };
    for &child in &coll.children {
        let Some(cc) = main.collections.get(child) else {
            continue;
        };
        let prev = old
            .children
            .iter()
            .find(|l| l.collection == Some(child))
            .cloned()
            .unwrap_or_else(|| LayerCollection::new(Some(child)));
        lc.children.push(sync_layer_collection(main, cc, &prev));
    }
    lc
}

/// Bases in `BKE_layer_collection_sync` order.
fn collect_bases(
    main: &Main,
    coll: &Collection,
    lc: &LayerCollection,
    out: &mut Vec<IdRef<ObjectId>>,
) {
    for (child, clc) in coll.children.iter().zip(&lc.children) {
        if clc.exclude {
            continue;
        }
        if let Some(cc) = main.collections.get(*child) {
            collect_bases(main, cc, clc, out);
        }
    }
    for &ob in &coll.objects {
        if !out.contains(&ob) {
            out.push(ob);
        }
    }
}

/// `BKE_layer_collection_sync` for every view layer of `scene`: layer collections follow the
/// collection tree, bases follow the sync order; existing bases keep their flags, new bases
/// are unselected, and the active object is cleared when its base is gone.
pub(crate) fn sync_view_layers(main: &mut Main, scene: IdRef<SceneId>, log: &mut ChangeLog) {
    let Some(s) = main.scenes.get(scene) else {
        return;
    };
    let mut layers = Vec::with_capacity(s.view_layers.len());
    for vl in &s.view_layers {
        let lc = sync_layer_collection(main, &s.collection, &vl.layer_collection);
        let mut order = Vec::new();
        collect_bases(main, &s.collection, &lc, &mut order);
        let bases: Vec<Base> = order
            .into_iter()
            .map(|ob| {
                vl.bases
                    .iter()
                    .find(|b| b.object == ob)
                    .cloned()
                    .unwrap_or(Base {
                        object: ob,
                        flag: BaseFlag::empty(),
                    })
            })
            .collect();
        let active = vl.active.filter(|a| bases.iter().any(|b| b.object == *a));
        layers.push((lc, bases, active));
    }
    if let Some(s) = main.scenes.get_mut(scene, log) {
        for (vl, (lc, bases, active)) in s.view_layers.iter_mut().zip(layers) {
            vl.layer_collection = lc;
            vl.bases = bases;
            vl.active = active;
        }
    }
    log.context_changed = true;
}

/// `BlenderScene.create` (with the Blender part of `init_vnodes`).
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
) -> Result<(), ImportError> {
    let scene = cx.scene;
    cx.active_collection = active_collection(main, scene, &cx.view_layer);
    let n_scenes = cx.doc.scenes.len();
    let default_scene = cx.doc.scene.map(|s| s as usize);
    if cx.settings.import_scene_extras
        && let Some(d) = default_scene
        && let Some(pyscene) = cx.doc.scenes.get(d)
        && let Some(s) = main.scenes.get_mut(scene, log)
    {
        extras::set_extras(&mut s.header, pyscene.extras.as_ref(), &[]);
    }

    // Scenes: the default glTF scene is the context scene, the others are new scenes.
    for (idx, sc) in cx.doc.scenes.iter().enumerate() {
        if idx != default_scene.unwrap_or(0) {
            let name = sc.name.clone().unwrap_or_else(|| format!("Scene {idx}"));
            cx.blender_scenes.push(new_scene(main, &name, log));
        } else {
            cx.blender_scenes.push(scene);
        }
    }
    // Collections: one scene → the active collection; several → one collection per glTF
    // scene under the active collection (with import_scene_as_collection).
    if n_scenes == 1 {
        cx.collections
            .insert(Some(default_scene.unwrap_or(0)), cx.active_collection);
    } else if n_scenes > 1 {
        for idx in 0..n_scenes {
            if cx.settings.import_scene_as_collection {
                let name = cx.doc.scenes[idx]
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Scene {idx}"));
                let c = new_collection(main, &name, cx.active_collection, log);
                cx.collections.insert(Some(idx), CollTarget::Coll(c));
            } else if Some(idx) == default_scene {
                cx.collections.insert(Some(idx), cx.active_collection);
            }
        }
    }
    let has_orphans = cx.vtree.nodes[..cx.doc.nodes.len()]
        .iter()
        .any(|v| v.scenes.is_empty());
    if has_orphans {
        let c = new_collection(main, "Orphan Nodes", cx.active_collection, log);
        cx.collections.insert(None, CollTarget::Coll(c));
    }

    nodes::create_vnode(cx, main, log, cx.vtree.root)?;

    for s in touched_scenes(cx) {
        sync_view_layers(main, s, log);
    }

    if cx.settings.import_select_created_objects && cx.settings.import_scene_as_collection {
        select_imported_objects(cx, main, log);
        set_active_object(cx, main, log);
    }
    if cx.settings.import_scene_as_collection
        && let Some(default) = default_scene
    {
        exclude_other_scenes(cx, main, log, default);
    }
    Ok(())
}

/// Scenes whose collections the import changed (the context scene first).
fn touched_scenes(cx: &Gltf) -> Vec<IdRef<SceneId>> {
    let mut out = vec![cx.scene];
    for &s in &cx.blender_scenes {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// `select_imported_objects`: deselect everything, then select every imported object.
fn select_imported_objects(cx: &Gltf, main: &mut Main, log: &mut ChangeLog) {
    let objects: Vec<IdRef<ObjectId>> = cx
        .vtree
        .nodes
        .iter()
        .filter(|v| v.kind == VKind::Object)
        .filter_map(|v| v.blender_object)
        .collect();
    let Some(s) = main.scenes.get_mut(cx.scene, log) else {
        return;
    };
    let Some(vl) = view_layer_mut(s, &cx.view_layer) else {
        return;
    };
    for b in vl.bases.iter_mut() {
        b.flag.remove(BaseFlag::SELECTED);
    }
    for b in vl.bases.iter_mut() {
        if objects.contains(&b.object) {
            b.flag.insert(BaseFlag::SELECTED);
        }
    }
    log.context_changed = true;
}

/// `set_active_object`: the first root of the default scene, else of the first scene with
/// nodes, else the first child of the dummy root.
fn set_active_object(cx: &Gltf, main: &mut Main, log: &mut ChangeLog) {
    let doc = &cx.doc;
    let mut vnode: Option<usize> = None;
    if let Some(d) = doc.scene
        && let Some(first) = doc.scenes.get(d as usize).and_then(|s| s.nodes.first())
    {
        vnode = Some(*first as usize);
    }
    if vnode.is_none() {
        vnode = doc
            .scenes
            .iter()
            .find_map(|s| s.nodes.first())
            .map(|&n| n as usize);
    }
    if vnode.is_none() {
        match cx.vtree.nodes[cx.vtree.root].children.first() {
            Some(&c) => vnode = Some(c),
            None => return,
        }
    }
    let Some(ob) = vnode
        .and_then(|v| cx.vtree.nodes.get(v))
        .and_then(|v| v.blender_object)
    else {
        return;
    };
    if let Some(s) = main.scenes.get_mut(cx.scene, log)
        && let Some(vl) = view_layer_mut(s, &cx.view_layer)
    {
        vl.active = Some(ob);
    }
    log.context_changed = true;
}

/// Excludes, in the active layer collection, the collections of the non-default scenes.
fn exclude_other_scenes(cx: &Gltf, main: &mut Main, log: &mut ChangeLog, default: usize) {
    let names: Vec<SmolStr> = cx
        .collections
        .iter()
        .filter(|(k, _)| **k != Some(default) && k.is_some())
        .filter_map(|(_, t)| match t {
            CollTarget::Coll(c) => main.collections.name_of(*c).map(SmolStr::new),
            CollTarget::Master(_) => None,
        })
        .collect();
    if names.is_empty() {
        return;
    }
    let Some(s) = main.scenes.get(cx.scene) else {
        return;
    };
    let Some(vl_idx) = s
        .view_layers
        .iter()
        .position(|v| v.name == cx.view_layer)
        .or(if s.view_layers.is_empty() {
            None
        } else {
            Some(0)
        })
    else {
        return;
    };
    let path = s.view_layers[vl_idx].active_layer_collection.clone();
    let child_colls: Vec<Option<IdRef<CollectionId>>> = s.view_layers[vl_idx]
        .layer_collection
        .at(&path)
        .map(|lc| lc.children.iter().map(|c| c.collection).collect())
        .unwrap_or_default();
    let targets: Vec<usize> = child_colls
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.and_then(|c| main.collections.name_of(c))
                .is_some_and(|n| names.iter().any(|x| x == n))
        })
        .map(|(i, _)| i)
        .collect();
    if let Some(s) = main.scenes.get_mut(cx.scene, log) {
        let vl = &mut s.view_layers[vl_idx];
        let mut lc = &mut vl.layer_collection;
        for &i in &path.0 {
            lc = &mut lc.children[i as usize];
        }
        for i in targets {
            lc.children[i].exclude = true;
        }
    }
    sync_view_layers(main, cx.scene, log);
}
