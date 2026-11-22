// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.collection.objects_add_active(collection=…)`, tier T0
//! (`tests/golden/cases/object/data_m8/collections.py`). Port of `objects_add_active_exec`,
//! `collection_object_active_itemf` and `collection_object_active_find_index`
//! (`editors/object/object_collection.cc`), `BKE_collection_object_find`,
//! `BKE_collection_object_cyclic_check` and `FOREACH_COLLECTION_BEGIN`
//! (`blenkernel/intern/collection.cc`, `BKE_collection.hh`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4:
//! - `collection` items: the collections holding the active object (`context.object`), the
//!   scene's master collection first (`Scene Collection`) then `bpy.data.collections` in order
//!   (also collections outside the scene), preceded by `All Collections` when there are two or
//!   more; an unknown name is Blender's `enum "X" not found in (…)`;
//! - unset, the enum's value is 0: the **first** collection holding the active object (not
//!   `All Collections`); `All Collections` (value `INT_MAX`) is every collection holding it;
//! - every selected editable object is linked to those collections (`collection.objects`
//!   order: appended); an object whose instanced collection would make a cycle is skipped with
//!   the WARNING `Skipped some collections because of cycle detected`;
//! - nothing linked (everything already there) or no active object: `CANCELLED`;
//! - poll `ED_operator_objectmode`.
//!
//! This file also holds the membership helpers of the `collection.objects_*` operators
//! ([`common`]).

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::Context;
use frap_spec_ops_object::collection;

use self::common::{CollectionChoice, choice, find_order, has_object};
use super::move_to_collection::common::{get, link_object};

/// Helpers shared by `collection.objects_add_active`, `objects_remove`,
/// `objects_remove_active` and `objects_remove_all`.
pub(crate) mod common {
    use frap_op::EnumItemOwned;
    use frap_op::prelude::*;
    use frap_rna::{CollectionId, IdRef, ObjectId, PyExc, RnaError, RnaErrorKind, SceneId};
    use frap_scene::{Context, Main};
    use smol_str::SmolStr;

    pub(crate) use crate::ops::move_to_collection::common::SceneColl;

    /// `FOREACH_COLLECTION_BEGIN (bmain, scene, …)`: the scene's master collection, then every
    /// collection of `bpy.data.collections` (in or out of the scene), in list order.
    pub(crate) fn foreach_collection(main: &Main) -> Vec<SceneColl> {
        std::iter::once(None)
            .chain(main.collections.iter().map(|(r, _)| Some(r)))
            .collect()
    }

    /// `BKE_collection_has_object` (direct membership).
    pub(crate) fn has_object(
        main: &Main,
        scene: IdRef<SceneId>,
        c: SceneColl,
        ob: IdRef<ObjectId>,
    ) -> bool {
        crate::ops::move_to_collection::common::has_object(main, scene, c, ob)
    }

    /// The `BKE_collection_object_find` walk: every collection directly holding `ob`, in
    /// [`foreach_collection`] order.
    pub(crate) fn find_order(
        main: &Main,
        scene: IdRef<SceneId>,
        ob: IdRef<ObjectId>,
    ) -> Vec<SceneColl> {
        foreach_collection(main)
            .into_iter()
            .filter(|&c| has_object(main, scene, c, ob))
            .collect()
    }

    /// `collection->id.name + 2`: the master collection is `Scene Collection`.
    pub(crate) fn name(main: &Main, scene: IdRef<SceneId>, c: SceneColl) -> SmolStr {
        crate::ops::move_to_collection::common::get(main, scene, c)
            .map(|col| col.header.name.clone())
            .unwrap_or_default()
    }

    /// `collection_object_active_itemf`: `All Collections` (when the active object is in two
    /// or more collections), then each collection holding it.
    pub(crate) fn items(c: &Context) -> Vec<EnumItemOwned> {
        let Some(ob) = c.object() else {
            return Vec::new();
        };
        let scene = c.win.scene;
        let found = find_order(c.main, scene, ob);
        let mut out = Vec::with_capacity(found.len() + 1);
        if found.len() >= 2 {
            out.push(EnumItemOwned {
                identifier: ALL.into(),
                name: ALL.into(),
                description: Default::default(),
            });
        }
        out.extend(found.into_iter().map(|col| {
            let n = name(c.main, scene, col);
            EnumItemOwned {
                identifier: n.clone(),
                name: n,
                description: Default::default(),
            }
        }));
        out
    }

    /// The `All Collections` item (`INT_MAX`: no single collection).
    pub(crate) const ALL: &str = "All Collections";

    /// What the `collection` enum selects.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub(crate) enum CollectionChoice {
        /// Every collection holding the active object (`single_collection == nullptr`).
        All,
        /// One collection.
        One(SceneColl),
    }

    /// `RNA_enum_get(op->ptr, "collection")` + `collection_object_active_find_index`: the
    /// item's value is its position among the collections holding `ob` (unset: 0, the first
    /// one), `All Collections` none; an index without a collection is every collection, as in
    /// Blender. A name that is not an item is Blender's conversion `TypeError`.
    pub(crate) fn choice(
        c: &Context,
        prop: &frap_rna::Prop<SmolStr>,
        ob: Option<IdRef<ObjectId>>,
    ) -> Result<CollectionChoice, OpError> {
        let Some(ob) = ob else {
            return Ok(CollectionChoice::All);
        };
        let found = find_order(c.main, c.win.scene, ob);
        let index = if prop.is_set {
            let all = items(c);
            if !all.iter().any(|i| i.identifier == prop.value) {
                return Err(not_found(&prop.value, &all));
            }
            if prop.value == ALL {
                return Ok(CollectionChoice::All);
            }
            found
                .iter()
                .position(|&col| name(c.main, c.win.scene, col) == prop.value)
                .unwrap_or(usize::MAX)
        } else {
            0
        };
        Ok(found
            .get(index)
            .map_or(CollectionChoice::All, |&col| CollectionChoice::One(col)))
    }

    /// Blender's `enum "X" not found in (…)` for a name that is not an item.
    fn not_found(wanted: &str, items: &[EnumItemOwned]) -> OpError {
        let list = items
            .iter()
            .map(|i| format!("'{}'", i.identifier))
            .collect::<Vec<_>>()
            .join(", ");
        let mut e = RnaError::new(
            RnaErrorKind::Enum,
            PyExc::TypeError,
            format!(
                "Converting py args to operator properties: enum \"{wanted}\" not found in ({list})"
            ),
        );
        e.detail.insert(
            "valid".into(),
            items
                .iter()
                .map(|i| i.identifier.as_str())
                .collect::<Vec<_>>()
                .into(),
        );
        OpError::Rna(e)
    }

    /// Whether `choice` lets collection `col` through.
    pub(crate) fn wanted(choice: CollectionChoice, col: SceneColl) -> bool {
        match choice {
            CollectionChoice::All => true,
            CollectionChoice::One(one) => one == col,
        }
    }

    /// `BKE_collection_object_cyclic_check(bmain, object, collection)`: whether linking
    /// `object` to `collection` would make an instancing cycle. A faithful port, including
    /// Blender's recursion, which checks the nested objects against the instanced collection
    /// (not against `collection`), and the `ID_TAG_DOIT` bookkeeping that reports a cycle that
    /// already exists.
    pub(crate) fn cyclic_check(
        main: &Main,
        object: IdRef<ObjectId>,
        collection: SceneColl,
    ) -> bool {
        // `BKE_main_id_tag_listbase(&bmain->collections, ID_TAG_DOIT, true)`.
        let mut doit: std::collections::HashSet<IdRef<CollectionId>> =
            main.collections.iter().map(|(r, _)| r).collect();
        cyclic_internal(main, object, collection, &mut doit)
    }

    fn cyclic_internal(
        main: &Main,
        object: IdRef<ObjectId>,
        collection: SceneColl,
        doit: &mut std::collections::HashSet<IdRef<CollectionId>>,
    ) -> bool {
        let Some(dup) = main.objects.get(object).and_then(|o| o.instance_collection) else {
            return false;
        };
        if !doit.contains(&dup) {
            // "Cycle already exists in collections, let's prevent further creepiness."
            return true;
        }
        doit.remove(&dup);
        if collection == Some(dup) {
            return true;
        }
        for ob in recursive_objects(main, dup) {
            if cyclic_internal(main, ob, Some(dup), doit) {
                return true;
            }
        }
        doit.insert(dup);
        false
    }

    /// `FOREACH_COLLECTION_OBJECT_RECURSIVE_BEGIN` (`BKE_collection_object_cache_get`): the
    /// objects of `c` and of its children, depth first, each once.
    fn recursive_objects(main: &Main, c: IdRef<CollectionId>) -> Vec<IdRef<ObjectId>> {
        let mut out: Vec<IdRef<ObjectId>> = Vec::new();
        let mut seen: Vec<IdRef<CollectionId>> = Vec::new();
        fn walk(
            main: &Main,
            c: IdRef<CollectionId>,
            out: &mut Vec<IdRef<ObjectId>>,
            seen: &mut Vec<IdRef<CollectionId>>,
        ) {
            if seen.contains(&c) {
                return;
            }
            seen.push(c);
            let Some(col) = main.collections.get(c) else {
                return;
            };
            for &o in &col.objects {
                if !out.contains(&o) {
                    out.push(o);
                }
            }
            for &child in &col.children {
                walk(main, child, out, seen);
            }
        }
        walk(main, c, &mut out, &mut seen);
        out
    }

    /// `BKE_collection_object_remove(bmain, collection, ob, false)` on the collection itself
    /// (the caller resyncs the view layers). Returns whether `ob` was there.
    pub(crate) fn unlink_object(
        main: &mut Main,
        log: &mut frap_scene::ChangeLog,
        scene: IdRef<SceneId>,
        c: SceneColl,
        ob: IdRef<ObjectId>,
    ) -> bool {
        if !has_object(main, scene, c, ob) {
            return false;
        }
        match crate::ops::move_to_collection::common::get_mut(main, log, scene, c) {
            Some(col) => {
                col.objects.retain(|o| *o != ob);
                true
            }
            None => false,
        }
    }

    /// Whether `c` is the master collection of `scene` or below it
    /// (`BKE_collection_is_in_scene`): removing from it changes the scene's bases.
    pub(crate) fn in_scene(main: &Main, scene: IdRef<SceneId>, c: SceneColl) -> bool {
        match c {
            None => true,
            Some(r) => {
                crate::ops::move_to_collection::common::scene_collections(main, scene).contains(&r)
            }
        }
    }
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    dyn_enums = [("collection", common::items)],
    notes = ["collection items: the collections holding the active object (Scene Collection first), after 'All Collections' when there are two or more; unset is the first of them"]
)]
fn exec(cx: &mut OpCtx, p: collection::ObjectsAddActive) -> OpResult {
    let (ob, single, scene, selected) = {
        let c: Context = cx.ctx();
        let ob = c.object();
        (
            ob,
            choice(&c, &p.collection, ob)?,
            c.win.scene,
            c.selected_editable_objects(),
        )
    };
    let Some(ob) = ob else {
        cx.report(
            ReportType::Warning,
            "frap: nothing linked: objects_add_active needs an active object",
        );
        return Ok(OpStatus::Cancelled);
    };
    let mut is_cycle = false;
    let mut changed_multi = false;
    {
        let (main, log) = cx.main_mut();
        for col in common::foreach_collection(main) {
            if let CollectionChoice::One(one) = single
                && one != col
            {
                continue;
            }
            if !has_object(main, scene, col, ob) {
                continue;
            }
            // `BKE_collection_object_add` refuses a collection linked from a library.
            if get(main, scene, col).is_none_or(|c| c.header.library.is_some()) {
                continue;
            }
            for &other in &selected {
                if has_object(main, scene, col, other) {
                    continue;
                }
                if common::cyclic_check(main, other, col) {
                    is_cycle = true;
                } else if link_object(main, log, scene, col, other) {
                    changed_multi = true;
                }
            }
        }
        if changed_multi {
            frap_scene::id::layers::sync_all(main, log);
        }
    }
    if is_cycle {
        cx.report(
            ReportType::Warning,
            "Skipped some collections because of cycle detected",
        );
    }
    if !changed_multi {
        let held = find_order(cx.main(), scene, ob).len();
        if held == 0 {
            cx.report(
                ReportType::Warning,
                "frap: nothing linked: the active object is in no collection",
            );
        }
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
