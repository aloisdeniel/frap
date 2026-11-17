// SPDX-License-Identifier: GPL-3.0-or-later
//! View-layer synchronisation and base visibility (ARCHITECTURE §8): a port of the parts of
//! Blender's `BKE_layer_collection_sync` / `BKE_base_eval_flags`
//! (`blenkernel/intern/layer.cc`, GPL-2.0-or-later) that frap needs.
//!
//! - The layer-collection tree mirrors the scene's collection hierarchy; a layer collection
//!   keeps its flags (`exclude`, `hide_viewport`, `holdout`, `indirect_only`) as long as its
//!   collection stays at that place (matched by collection, as Blender does).
//! - Bases are rebuilt in sync order: depth first, **child layer collections before the
//!   collection's own objects**, excluded subtrees skipped. An object already placed is moved
//!   to the tail unless it is the first or the last base of the new list (Blender's
//!   `ELEM(base, first, last)` test; verified on 5.2.4 with an object linked into two
//!   collections). Existing bases keep their flags; new bases start unselected.
//! - Unselectable bases are deselected (`BKE_base_eval_flags`): an object is selectable when it
//!   is reached through a collection path without `hide_viewport`/`hide_select`, and it has
//!   neither `hide_select` nor `hide_viewport` itself (verified: `hide_viewport`,
//!   `hide_select` and a collection's `hide_viewport`/`hide_select` deselect; a layer
//!   collection's `hide_viewport` does not). A base hidden with `hide_set` is not selectable
//!   either.
//! - The active object is dropped when it has no base any more (verified with `exclude`).
//!
//! Blender syncs lazily (`view_layer.objects` is stale until the next depsgraph update after a
//! link); frap syncs eagerly whenever membership changes, which only differs in that window.

use std::collections::BTreeMap;

use frap_rna::{CollectionId, IdRef, ObjectId, SceneId};

use crate::context::{Base, BaseFlag, LayerCollection, LayerCollectionPath, ViewLayer};
use crate::data::{ChangeLog, IdMap, Main};
use crate::id::{Collection, Object, Scene};

/// Deepest collection nesting followed (guards against a cyclic `children` graph).
const MAX_DEPTH: usize = 256;

/// Visibility facts of one base, from every collection path that reaches its object.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BaseVisibility {
    /// Visible in the (virtual) viewport: `visible_get()`.
    pub visible: bool,
    /// Selectable (`BASE_SELECTABLE`): `select_set(True)` has an effect. A layer collection's
    /// `hide_viewport` does not clear it (verified on 5.2.4); `hide_set(True)` does.
    pub selectable: bool,
    /// Rendered (`hide_render` of the object and its collections).
    pub renderable: bool,
}

/// The collection a layer collection shows (`None` = the scene's master collection).
fn coll<'a>(
    scene: &'a Scene,
    collections: &'a IdMap<Collection>,
    c: Option<IdRef<CollectionId>>,
) -> Option<&'a Collection> {
    match c {
        None => Some(&scene.collection),
        Some(r) => collections.get(r),
    }
}

/// Rebuilds a layer-collection subtree for collection `c`, keeping the flags of `old` and of
/// its children that still show the same collections.
fn sync_lc(
    old: Option<&LayerCollection>,
    c: Option<IdRef<CollectionId>>,
    scene: &Scene,
    collections: &IdMap<Collection>,
    depth: usize,
) -> LayerCollection {
    let mut lc = match old {
        Some(o) => LayerCollection {
            children: Vec::new(),
            ..o.clone()
        },
        None => LayerCollection::new(c),
    };
    lc.collection = c;
    if depth >= MAX_DEPTH {
        return lc;
    }
    let Some(col) = coll(scene, collections, c) else {
        return lc;
    };
    let mut used = vec![false; old.map_or(0, |o| o.children.len())];
    for child in &col.children {
        if !collections.contains(*child) {
            continue;
        }
        let prev = old.and_then(|o| {
            o.children
                .iter()
                .enumerate()
                .find(|(i, x)| !used[*i] && x.collection == Some(*child))
                .map(|(i, x)| {
                    used[i] = true;
                    x
                })
        });
        lc.children
            .push(sync_lc(prev, Some(*child), scene, collections, depth + 1));
    }
    lc
}

#[derive(Clone, Copy, Default)]
struct Restrict {
    coll_hide_viewport: bool,
    coll_hide_select: bool,
    coll_hide_render: bool,
    layer_hide: bool,
}

/// Walks the layer tree in sync order, calling `f(object, restrict)` for every object
/// occurrence of a non-excluded layer collection (children first, then own objects).
fn walk(
    lc: &LayerCollection,
    scene: &Scene,
    collections: &IdMap<Collection>,
    parent: Restrict,
    depth: usize,
    f: &mut dyn FnMut(IdRef<ObjectId>, Restrict),
) {
    if lc.exclude || depth >= MAX_DEPTH {
        return;
    }
    let Some(col) = coll(scene, collections, lc.collection) else {
        return;
    };
    let r = Restrict {
        coll_hide_viewport: parent.coll_hide_viewport || col.hide_viewport,
        coll_hide_select: parent.coll_hide_select || col.hide_select,
        coll_hide_render: parent.coll_hide_render || col.hide_render,
        layer_hide: parent.layer_hide || lc.hide_viewport,
    };
    for child in &lc.children {
        walk(child, scene, collections, r, depth + 1, f);
    }
    for ob in &col.objects {
        f(*ob, r);
    }
}

/// Per-object visibility in one view layer (objects without a base are absent).
pub fn visibility(
    scene: &Scene,
    vl: &ViewLayer,
    collections: &IdMap<Collection>,
    objects: &IdMap<Object>,
) -> BTreeMap<IdRef<ObjectId>, BaseVisibility> {
    let mut out: BTreeMap<IdRef<ObjectId>, BaseVisibility> = BTreeMap::new();
    walk(
        &vl.layer_collection,
        scene,
        collections,
        Restrict::default(),
        0,
        &mut |ob, r| {
            let e = out.entry(ob).or_default();
            e.visible |= !r.coll_hide_viewport && !r.layer_hide;
            e.selectable |= !r.coll_hide_viewport && !r.coll_hide_select;
            e.renderable |= !r.coll_hide_render;
        },
    );
    for (ob, v) in out.iter_mut() {
        let Some(o) = objects.get(*ob) else {
            continue;
        };
        let hidden = vl
            .bases
            .iter()
            .find(|b| b.object == *ob)
            .is_some_and(|b| b.flag.contains(BaseFlag::HIDDEN));
        v.visible &= !o.hide_viewport && !hidden;
        v.selectable &= !o.hide_viewport && !o.hide_select && !hidden;
        v.renderable &= !o.hide_render;
    }
    out
}

/// The view layers of `scene` after a sync against the current collections and objects.
pub fn synced_view_layers(
    scene: &Scene,
    collections: &IdMap<Collection>,
    objects: &IdMap<Object>,
) -> Vec<ViewLayer> {
    scene
        .view_layers
        .iter()
        .map(|vl| sync_view_layer(scene, vl, collections, objects))
        .collect()
}

fn sync_view_layer(
    scene: &Scene,
    vl: &ViewLayer,
    collections: &IdMap<Collection>,
    objects: &IdMap<Object>,
) -> ViewLayer {
    let active_lc = vl
        .layer_collection
        .at(&vl.active_layer_collection)
        .map(|lc| lc.collection);
    let layer_collection = sync_lc(Some(&vl.layer_collection), None, scene, collections, 0);
    // Bases in sync order.
    let mut bases: Vec<Base> = Vec::new();
    walk(
        &layer_collection,
        scene,
        collections,
        Restrict::default(),
        0,
        &mut |ob, _| {
            if !objects.contains(ob) {
                return;
            }
            if let Some(i) = bases.iter().position(|b| b.object == ob) {
                if i != 0 && i + 1 != bases.len() {
                    let b = bases.remove(i);
                    bases.push(b);
                }
                return;
            }
            let flag = vl
                .bases
                .iter()
                .find(|b| b.object == ob)
                .map_or(BaseFlag::empty(), |b| b.flag);
            bases.push(Base { object: ob, flag });
        },
    );
    let mut out = ViewLayer {
        name: vl.name.clone(),
        bases,
        active: vl.active,
        layer_collection,
        active_layer_collection: LayerCollectionPath::default(),
    };
    // Deselect what cannot be selected.
    let vis = visibility(scene, &out, collections, objects);
    for b in &mut out.bases {
        if !vis.get(&b.object).is_some_and(|v| v.selectable) {
            b.flag.remove(BaseFlag::SELECTED);
        }
    }
    if let Some(a) = out.active
        && !out.bases.iter().any(|b| b.object == a)
    {
        out.active = None;
    }
    out.active_layer_collection = active_lc
        .and_then(|c| out.layer_collection.path_of(c))
        .unwrap_or_default();
    out
}

/// Resyncs the view layers of one scene; the scene is written (bumped, logged) only when
/// something changed.
pub fn sync_scene(main: &mut Main, scene: IdRef<SceneId>, log: &mut ChangeLog) {
    let Some(s) = main.scenes.get(scene) else {
        return;
    };
    let new = synced_view_layers(s, &main.collections, &main.objects);
    if new != s.view_layers
        && let Some(s) = main.scenes.get_mut(scene, log)
    {
        s.view_layers = new;
        log.context_changed = true;
    }
}

/// Resyncs every scene's view layers (after collection membership, collection or object
/// visibility flags, or `exclude` changed).
pub fn sync_all(main: &mut Main, log: &mut ChangeLog) {
    let scenes: Vec<IdRef<SceneId>> = main.scenes.iter().map(|(r, _)| r).collect();
    for s in scenes {
        sync_scene(main, s, log);
    }
}

fn not_in_layer(
    main: &Main,
    ob: IdRef<ObjectId>,
    vl: &ViewLayer,
    what: &str,
) -> frap_rna::RnaError {
    frap_rna::RnaError::new(
        frap_rna::RnaErrorKind::Other,
        frap_rna::PyExc::RuntimeError,
        format!(
            "Error: Object '{}' cannot be {what} because it is not in View Layer '{}'!\n",
            main.objects.name_of(ob).unwrap_or_default(),
            vl.name
        ),
    )
}

/// `Object.select_set(state, view_layer)` (`rna_Object_select_set` → `ED_object_base_select`,
/// verified on 5.2.4): selecting needs a selectable base (`BASE_SELECTABLE`: an object hidden
/// with `hide_set` or unselectable stays unselected, silently; one under a layer collection
/// with `hide_viewport` is selected); deselecting always works; selecting an object without a
/// base raises `RuntimeError: Error: Object 'X' cannot be selected because it is not in View
/// Layer 'ViewLayer'!`, deselecting it is a no-op.
pub fn select_set(
    main: &mut Main,
    log: &mut ChangeLog,
    scene: IdRef<SceneId>,
    vl: usize,
    ob: IdRef<ObjectId>,
    state: bool,
) -> Result<(), frap_rna::RnaError> {
    let Some(s) = main.scenes.get(scene) else {
        return Ok(());
    };
    let layer = &s.view_layers[vl];
    let Some(base) = layer.base(ob) else {
        return if state {
            Err(not_in_layer(main, ob, layer, "selected"))
        } else {
            Ok(())
        };
    };
    let can = !state
        || visibility(s, layer, &main.collections, &main.objects)
            .get(&ob)
            .is_some_and(|v| v.selectable);
    if !can || base.selected() == state {
        return Ok(());
    }
    if let Some(s) = main.scenes.get_mut(scene, log)
        && let Some(b) = s.view_layers[vl].base_mut(ob)
    {
        b.flag.set(BaseFlag::SELECTED, state);
    }
    log.context_changed = true;
    Ok(())
}

/// `Object.hide_set(state, view_layer)` (verified on 5.2.4): hiding sets the base's hidden
/// flag and deselects it; unhiding clears the flag (the selection is not restored); hiding an
/// object without a base raises `RuntimeError: Error: Object 'X' cannot be hidden because it
/// is not in View Layer 'ViewLayer'!`, unhiding it is a no-op.
pub fn hide_set(
    main: &mut Main,
    log: &mut ChangeLog,
    scene: IdRef<SceneId>,
    vl: usize,
    ob: IdRef<ObjectId>,
    state: bool,
) -> Result<(), frap_rna::RnaError> {
    let Some(s) = main.scenes.get(scene) else {
        return Ok(());
    };
    let layer = &s.view_layers[vl];
    let Some(base) = layer.base(ob) else {
        return if state {
            Err(not_in_layer(main, ob, layer, "hidden"))
        } else {
            Ok(())
        };
    };
    let mut flag = base.flag;
    flag.set(BaseFlag::HIDDEN, state);
    if state {
        flag.remove(BaseFlag::SELECTED);
    }
    if flag == base.flag {
        return Ok(());
    }
    if let Some(s) = main.scenes.get_mut(scene, log)
        && let Some(b) = s.view_layers[vl].base_mut(ob)
    {
        b.flag = flag;
    }
    log.context_changed = true;
    Ok(())
}

/// Writes one flag of the layer collection a pointer addresses (`exclude` also resyncs the
/// view layer: excluded collections have no bases).
pub(crate) fn set_layer_collection_flag(
    h: &mut dyn crate::Host,
    this: &frap_rna::RnaPtr,
    v: &frap_rna::Value,
    flag: fn(&mut LayerCollection) -> &mut bool,
) -> Result<(), frap_rna::RnaError> {
    let (scene, i, path) = crate::id::paths::layer_collection_of(h.main(), this)?;
    let Some(b) = v.as_bool() else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    let changed = main
        .scenes
        .get(scene)
        .and_then(|s| s.view_layers[i].layer_collection.at(&path))
        .is_some_and(|lc| *flag(&mut lc.clone()) != b);
    if !changed {
        return Ok(());
    }
    if let Some(s) = main.scenes.get_mut(scene, log)
        && let Some(lc) = s.view_layers[i].layer_collection.at_mut(&path)
    {
        *flag(lc) = b;
    }
    sync_scene(main, scene, log);
    Ok(())
}

/// `LayerCollection.is_visible`: not excluded and not hidden along its layer-collection and
/// collection chain (`LAYER_COLLECTION_VISIBLE_VIEW_LAYER` after a sync; Blender reports the
/// value of the last depsgraph sync, frap the current one, D-04).
pub fn layer_collection_visible(
    scene: &Scene,
    vl: &ViewLayer,
    collections: &IdMap<Collection>,
    path: &LayerCollectionPath,
) -> bool {
    let mut lc = &vl.layer_collection;
    let mut chain = vec![lc];
    for &i in &path.0 {
        match lc.children.get(i as usize) {
            Some(c) => {
                lc = c;
                chain.push(c);
            }
            None => return false,
        }
    }
    chain.iter().all(|lc| {
        !lc.exclude
            && !lc.hide_viewport
            && coll(scene, collections, lc.collection).is_none_or(|c| !c.hide_viewport)
    })
}

impl LayerCollection {
    /// Path of the first layer collection (depth first) showing collection `c`
    /// (`None` = the master collection, at the empty path).
    pub fn path_of(&self, c: Option<IdRef<CollectionId>>) -> Option<LayerCollectionPath> {
        fn go(lc: &LayerCollection, c: Option<IdRef<CollectionId>>, path: &mut Vec<u16>) -> bool {
            if lc.collection == c {
                return true;
            }
            for (i, ch) in lc.children.iter().enumerate() {
                path.push(i as u16);
                if go(ch, c, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        go(self, c, &mut path).then(|| LayerCollectionPath(path.into_iter().collect()))
    }
    /// Mutable access along a path of child indices.
    pub fn at_mut(&mut self, path: &LayerCollectionPath) -> Option<&mut LayerCollection> {
        let mut lc = self;
        for &i in &path.0 {
            lc = lc.children.get_mut(i as usize)?;
        }
        Some(lc)
    }
}

impl ViewLayer {
    /// A view layer of `scene` with a layer-collection tree and bases synced to its
    /// collections (all flags off, nothing selected, no active object).
    pub fn new_synced(
        name: &str,
        scene: &Scene,
        collections: &IdMap<Collection>,
        objects: &IdMap<Object>,
    ) -> ViewLayer {
        let vl = ViewLayer {
            name: name.into(),
            bases: Vec::new(),
            active: None,
            layer_collection: LayerCollection::new(None),
            active_layer_collection: LayerCollectionPath::default(),
        };
        sync_view_layer(scene, &vl, collections, objects)
    }
    /// The base of `ob` in this view layer.
    pub fn base(&self, ob: IdRef<ObjectId>) -> Option<&Base> {
        self.bases.iter().find(|b| b.object == ob)
    }
    /// Mutable base of `ob`.
    pub fn base_mut(&mut self, ob: IdRef<ObjectId>) -> Option<&mut Base> {
        self.bases.iter_mut().find(|b| b.object == ob)
    }
}
