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
//!   collections). Existing bases keep their flags; a new base is selected when the object's
//!   stored base flag is (`object_base_new` reads `ob->base_flag & BASE_SELECTED`): an object
//!   selected in one scene and linked into another is selected there too (verified on 5.2.4).
//!   Blender syncs `ob->base_flag` from a base in `ED_object_base_select` (`select_set`) and
//!   from the window's view layer at each depsgraph evaluation; frap keeps it in
//!   `Main::base_selected`, written by [`select_set`] and, before each sync, from the bases of
//!   every object whose bases all agree (so a conflict keeps the last `select_set`).
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

/// `Main::base_selected`: the `BASE_SELECTED` bit of each object's `ob->base_flag`. Runtime
/// state: `Debug` leaves the entries out, so a saved and reopened state prints the same.
#[derive(Clone, Default)]
pub struct BaseSelectedMemory(pub BTreeMap<IdRef<ObjectId>, bool>);

impl std::fmt::Debug for BaseSelectedMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BaseSelectedMemory(runtime)")
    }
}

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
    /// Enabled in the viewport (`BASE_ENABLED_AND_VISIBLE_IN_DEFAULT_VIEWPORT`): neither the
    /// object's nor every reaching collection's `hide_viewport` is set. A layer collection's
    /// `hide_viewport` and `hide_set` do not clear it. The viewport depsgraph evaluates exactly
    /// these bases (`need_pull_base_into_graph`; verified on 5.2.4: `frame_set` animates a
    /// `hide_set` object but not a `hide_viewport` one).
    pub enabled_viewport: bool,
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

/// Runtime: per (scene, view layer name), the layer-collection tree Blender's lazy resync
/// (`VIEW_LAYER_OUT_OF_SYNC`, `BKE_view_layer_synced_ensure`) still starts from. Blender only
/// tags a view layer when collections are linked or unlinked; the tree is rebuilt once, at the
/// next read that ensures the sync, from the tree of the previous sync. frap syncs eagerly, so
/// it keeps that previous tree here from the first change on and rebuilds every eager sync
/// from it: a collection unlinked and linked again (or moved: linked under another parent,
/// then unlinked) keeps its layer collection's flags, as in Blender (verified on 5.2.4).
/// [`end_lazy_resync`] drops the trees where Blender would have resynced: operators and
/// evaluation boundaries (`view_layer.update()`, `frame_set`, depsgraph reads) and writes of
/// layer-collection flags. `Debug` leaves the entries out.
#[derive(Clone, Default)]
pub struct LazyResync(pub BTreeMap<(IdRef<SceneId>, smol_str::SmolStr), LayerCollection>);

impl std::fmt::Debug for LazyResync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LazyResync(runtime)")
    }
}

/// Blender resynced every view layer (see [`LazyResync`]): later syncs start from the current
/// trees.
pub fn end_lazy_resync(main: &mut Main) {
    main.lazy_resync.0.clear();
}

/// One layer collection of the previous tree, or one created by the sync
/// (`LayerCollectionResync`).
struct ResyncNode {
    collection: Option<IdRef<CollectionId>>,
    /// The flags (the layer collection without its children).
    flags: LayerCollection,
    parent: Option<usize>,
    children: Vec<usize>,
    is_used: bool,
    is_valid_as_child: bool,
}

/// `layer_collection_resync_create_recurse`.
fn resync_create(
    nodes: &mut Vec<ResyncNode>,
    parent: Option<usize>,
    lc: &LayerCollection,
    scene: &Scene,
    collections: &IdMap<Collection>,
    depth: usize,
) -> usize {
    let usable = lc.collection.is_none_or(|c| collections.contains(c));
    let is_valid_as_child = usable
        && parent.is_none_or(|p| {
            let pc = nodes[p].collection;
            let parent_usable = pc.is_none_or(|c| collections.contains(c));
            parent_usable
                && lc.collection.is_some_and(|c| {
                    coll(scene, collections, pc).is_some_and(|col| col.children.contains(&c))
                })
        });
    let is_used = is_valid_as_child && parent.is_none_or(|p| nodes[p].is_used);
    let i = nodes.len();
    nodes.push(ResyncNode {
        collection: lc.collection,
        flags: LayerCollection {
            children: Vec::new(),
            ..lc.clone()
        },
        parent,
        children: Vec::new(),
        is_used,
        is_valid_as_child,
    });
    if let Some(p) = parent {
        nodes[p].children.push(i);
    }
    if depth < MAX_DEPTH {
        for child in &lc.children {
            resync_create(nodes, Some(i), child, scene, collections, depth + 1);
        }
    }
    i
}

/// `layer_collection_resync_find`: the best unused layer collection of the previous tree for
/// `child` under the node `start`: a direct child of `start`, else (breadth first through
/// `start`'s descendants, then its siblings' subtrees, going up) one whose collection moved
/// away from its parent and that is not used yet.
fn resync_find(nodes: &[ResyncNode], start: usize, child: IdRef<CollectionId>) -> Option<usize> {
    let mut queue = std::collections::VecDeque::from([start]);
    let mut root = start;
    while let Some(cur) = queue.pop_front() {
        let n = &nodes[cur];
        if n.collection == Some(child)
            && (n.parent == Some(start) || (!n.is_used && !n.is_valid_as_child))
        {
            return Some(cur);
        }
        queue.extend(n.children.iter().copied());
        if queue.is_empty()
            && let Some(p) = nodes[root].parent
        {
            queue.extend(nodes[p].children.iter().copied().filter(|&x| x != root));
            root = p;
        }
    }
    None
}

/// `layer_collection_sync` (the layer-collection half): the children of the collection of
/// node `at` reuse the layer collections [`resync_find`] finds, or get new ones with the
/// parent's flags (`child_layer->flag = parent_layer_flag`: a collection linked under an
/// excluded one is excluded, verified on 5.2.4).
fn resync_lc(
    nodes: &mut Vec<ResyncNode>,
    known: &std::collections::HashSet<IdRef<CollectionId>>,
    at: usize,
    lc: &mut LayerCollection,
    scene: &Scene,
    collections: &IdMap<Collection>,
    depth: usize,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    let Some(col) = coll(scene, collections, nodes[at].collection) else {
        return;
    };
    for child in &col.children {
        if !collections.contains(*child) {
            continue;
        }
        let (node, mut child_lc) = match known
            .contains(child)
            .then(|| resync_find(nodes, at, *child))
            .flatten()
        {
            Some(found) => {
                nodes[found].is_used = true;
                let mut l = nodes[found].flags.clone();
                l.collection = Some(*child);
                (found, l)
            }
            None => {
                let l = LayerCollection {
                    collection: Some(*child),
                    children: Vec::new(),
                    ..lc.clone()
                };
                let i = nodes.len();
                nodes.push(ResyncNode {
                    collection: Some(*child),
                    flags: l.clone(),
                    parent: Some(at),
                    children: Vec::new(),
                    is_used: true,
                    is_valid_as_child: true,
                });
                nodes[at].children.push(i);
                (i, l)
            }
        };
        resync_lc(
            nodes,
            known,
            node,
            &mut child_lc,
            scene,
            collections,
            depth + 1,
        );
        lc.children.push(child_lc);
    }
}

/// `BKE_layer_collection_sync` of the layer-collection tree: the tree for the scene's current
/// collection hierarchy, rebuilt from `old` (the tree of the previous sync).
fn sync_layer_tree(
    old: &LayerCollection,
    scene: &Scene,
    collections: &IdMap<Collection>,
) -> LayerCollection {
    let mut nodes = Vec::new();
    let root = resync_create(&mut nodes, None, old, scene, collections, 0);
    // Collections without a previous layer collection skip the search (a new one only matches
    // the node being created).
    let known = nodes.iter().filter_map(|n| n.collection).collect();
    let mut lc = LayerCollection {
        children: Vec::new(),
        ..old.clone()
    };
    lc.collection = None;
    resync_lc(&mut nodes, &known, root, &mut lc, scene, collections, 0);
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
            e.enabled_viewport |= !r.coll_hide_viewport;
        },
    );
    // The first base of each object (a map, not a search per object: every response's context
    // block reads this, so it must stay O(#bases log #bases); M8 WP 8.8).
    let mut base_hidden: BTreeMap<IdRef<ObjectId>, bool> = BTreeMap::new();
    for b in &vl.bases {
        base_hidden
            .entry(b.object)
            .or_insert_with(|| b.flag.contains(BaseFlag::HIDDEN));
    }
    for (ob, v) in out.iter_mut() {
        let Some(o) = objects.get(*ob) else {
            continue;
        };
        let hidden = base_hidden.get(ob).copied().unwrap_or(false);
        v.visible &= !o.hide_viewport && !hidden;
        v.selectable &= !o.hide_viewport && !o.hide_select && !hidden;
        v.renderable &= !o.hide_render;
        v.enabled_viewport &= !o.hide_viewport;
    }
    out
}

/// The view layers of `scene` after a sync against the current collections and objects; new
/// bases are selected as `base_selected` says (`Main::base_selected`).
pub fn synced_view_layers(
    scene: &Scene,
    collections: &IdMap<Collection>,
    objects: &IdMap<Object>,
    base_selected: &BaseSelectedMemory,
) -> Vec<ViewLayer> {
    scene
        .view_layers
        .iter()
        .map(|vl| sync_view_layer(scene, vl, collections, objects, base_selected, None))
        .collect()
}

fn sync_view_layer(
    scene: &Scene,
    vl: &ViewLayer,
    collections: &IdMap<Collection>,
    objects: &IdMap<Object>,
    base_selected: &BaseSelectedMemory,
    lazy_base: Option<&LayerCollection>,
) -> ViewLayer {
    let active_lc = vl
        .layer_collection
        .at(&vl.active_layer_collection)
        .map(|lc| lc.collection);
    let layer_collection = sync_layer_tree(
        lazy_base.unwrap_or(&vl.layer_collection),
        scene,
        collections,
    );
    // Bases in sync order. The previous flags and the objects already placed are looked up
    // in maps (a search per object made every sync O(#objects²): M8 WP 8.8); only an object
    // met a second time (it is in several collections) searches the list, to move it.
    let mut old_flags: BTreeMap<IdRef<ObjectId>, BaseFlag> = BTreeMap::new();
    for b in &vl.bases {
        old_flags.entry(b.object).or_insert(b.flag);
    }
    let mut placed: std::collections::BTreeSet<IdRef<ObjectId>> = Default::default();
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
            if placed.contains(&ob) {
                if let Some(i) = bases.iter().position(|b| b.object == ob)
                    && i != 0
                    && i + 1 != bases.len()
                {
                    let b = bases.remove(i);
                    bases.push(b);
                }
                return;
            }
            placed.insert(ob);
            let flag = old_flags.get(&ob).copied().unwrap_or_else(|| {
                if base_selected.0.get(&ob).copied().unwrap_or(false) {
                    BaseFlag::SELECTED
                } else {
                    BaseFlag::empty()
                }
            });
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
    remember_base_selection(main);
    sync_scene_only(main, scene, log);
}

/// Stores the selection of every object whose bases (in every view layer of every scene) all
/// agree into `Main::base_selected` (the depsgraph's `BKE_object_sync_to_original`, which
/// copies the window view layer's base flags; frap does not know the window here, so an
/// object selected in one scene and not in another keeps the value it had).
fn remember_base_selection(main: &mut Main) {
    let mut agree: BTreeMap<IdRef<ObjectId>, Option<bool>> = BTreeMap::new();
    for (_, s) in main.scenes.iter() {
        for vl in &s.view_layers {
            for b in &vl.bases {
                let sel = b.selected();
                agree
                    .entry(b.object)
                    .and_modify(|e| {
                        if *e != Some(sel) {
                            *e = None;
                        }
                    })
                    .or_insert(Some(sel));
            }
        }
    }
    for (ob, sel) in agree {
        if let Some(sel) = sel {
            main.base_selected.0.insert(ob, sel);
        }
    }
}

fn sync_scene_only(main: &mut Main, scene: IdRef<SceneId>, log: &mut ChangeLog) {
    let Some(s) = main.scenes.get(scene) else {
        return;
    };
    let lazy = &main.lazy_resync.0;
    let new: Vec<ViewLayer> = s
        .view_layers
        .iter()
        .map(|vl| {
            let base = lazy.get(&(scene, vl.name.clone()));
            sync_view_layer(
                s,
                vl,
                &main.collections,
                &main.objects,
                &main.base_selected,
                base,
            )
        })
        .collect();
    if new != s.view_layers {
        // The layer trees Blender's next (lazy) sync starts from: the ones before the first
        // change since it last resynced.
        for vl in &s.view_layers {
            let changed = new
                .iter()
                .find(|n| n.name == vl.name)
                .is_none_or(|n| n.layer_collection != vl.layer_collection);
            if changed {
                main.lazy_resync
                    .0
                    .entry((scene, vl.name.clone()))
                    .or_insert_with(|| vl.layer_collection.clone());
            }
        }
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.view_layers = new;
            log.context_changed = true;
        }
    }
}

/// Resyncs every scene's view layers (after collection membership, collection or object
/// visibility flags, or `exclude` changed).
pub fn sync_all(main: &mut Main, log: &mut ChangeLog) {
    remember_base_selection(main);
    let scenes: Vec<IdRef<SceneId>> = main.scenes.iter().map(|(r, _)| r).collect();
    for s in scenes {
        sync_scene_only(main, s, log);
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
    // `BKE_scene_object_base_flag_sync_from_base`: the object remembers the resulting state.
    main.base_selected
        .0
        .insert(ob, if can { state } else { base.selected() });
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
    // Reaching a layer collection (`LayerCollection.children`) resynced the view layer.
    end_lazy_resync(main);
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

// Ported from blender/source/blender/makesrna/intern/rna_layer.cc
// (`rna_LayerCollection_exclude_update`, `rna_LayerCollection_exclude_update_recursive`;
// Blender 5.2.4, GPL-2.0-or-later).
/// `LayerCollection.exclude = v`: sets the flag, clears the layer collection's own
/// `LAYER_COLLECTION_PREVIOUSLY_EXCLUDED`, then, on every write (also one that changes
/// nothing), sets the whole subtree to the same value: excluding remembers in each descendant
/// whether it was excluded already, and including re-includes only the descendants that were
/// not. Verified on 5.2.4: after `Collection.exclude = True`, `children['Sub'].exclude` reads
/// True and `exclude = False` reads False again; a Sub excluded before stays excluded; a
/// second `exclude = True` remembers the first one's descendants as excluded; `exclude =
/// False` on an included collection re-includes its excluded children; a grandchild included
/// under an excluded child is re-included.
pub(crate) fn set_layer_collection_exclude(
    h: &mut dyn crate::Host,
    this: &frap_rna::RnaPtr,
    v: &frap_rna::Value,
) -> Result<(), frap_rna::RnaError> {
    fn recurse(children: &mut [LayerCollection], exclude: bool) {
        for lc in children {
            if exclude {
                lc.previously_excluded = lc.exclude;
                lc.exclude = true;
            } else if !lc.previously_excluded {
                lc.exclude = false;
            }
            recurse(&mut lc.children, exclude);
        }
    }
    let (scene, i, path) = crate::id::paths::layer_collection_of(h.main(), this)?;
    let Some(b) = v.as_bool() else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    // Reaching a layer collection (`LayerCollection.children`) resynced the view layer.
    end_lazy_resync(main);
    let Some(old) = main
        .scenes
        .get(scene)
        .and_then(|s| s.view_layers[i].layer_collection.at(&path))
    else {
        return Ok(());
    };
    let mut new = old.clone();
    new.exclude = b;
    new.previously_excluded = false;
    recurse(&mut new.children, b);
    if new == *old {
        return Ok(());
    }
    if let Some(s) = main.scenes.get_mut(scene, log)
        && let Some(lc) = s.view_layers[i].layer_collection.at_mut(&path)
    {
        *lc = new;
    }
    sync_scene(main, scene, log);
    Ok(())
}

// Ported from blender/source/blender/makesrna/intern/rna_layer.cc
// (`rna_LayerCollection_exclude_update`; Blender 5.2.4, GPL-2.0-or-later).
/// The objects a write of `LayerCollection.exclude` at `this` tags for an animation update
/// (`ID_RECALC_ANIMATION`): when the layer collection is not excluded after the write, every
/// object with a base in its view layer, so the next evaluation re-applies their animation at
/// the current frame (verified on 5.2.4: re-including any collection, even an empty unrelated
/// one, or writing `exclude = False` on an included one, overwrites a manual `location` of an
/// animated object at `view_layer.update()`; a parent with no base, and shape keys, are not
/// re-applied). Empty after `exclude = True`.
pub fn exclude_update_objects(main: &Main, this: &frap_rna::RnaPtr) -> Vec<IdRef<ObjectId>> {
    let Ok((scene, i, path)) = crate::id::paths::layer_collection_of(main, this) else {
        return Vec::new();
    };
    let Some(vl) = main.scenes.get(scene).and_then(|s| s.view_layers.get(i)) else {
        return Vec::new();
    };
    if vl.layer_collection.at(&path).is_none_or(|lc| lc.exclude) {
        return Vec::new();
    }
    vl.bases.iter().map(|b| b.object).collect()
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
        sync_view_layer(
            scene,
            &vl,
            collections,
            objects,
            &BaseSelectedMemory::default(),
            None,
        )
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
