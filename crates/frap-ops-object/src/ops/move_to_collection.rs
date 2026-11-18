// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.move_to_collection(collection_uid=-1, is_new=False,
//! new_collection_name='')`, tier T0 (`tests/golden/cases/object/collection/*.py`). Port of
//! `move_to_collection_exec` (`editors/object/object_edit.cc`, Blender 5.2.4,
//! GPL-2.0-or-later), shared with `object.link_to_collection`. Verified on 5.2.4:
//! - poll `ED_operator_objectmode`;
//! - `collection_uid` must be **set** (`Error: No collection selected`) and name the
//!   `session_uid` of the scene's master collection or of a collection below it (`Error:
//!   Unexpected error, collection not found`, also for `-1`, `0` and collections outside the
//!   scene);
//! - `is_new` creates a child collection of that one named `new_collection_name` (empty →
//!   `Collection`, uniquified) and targets it;
//! - every object of `context.selected_objects` is moved: linked to the target (once) and
//!   unlinked from every other collection of the scene (master included); objects keep their
//!   selection and the active object stays;
//! - INFO report `<Object> moved to <Collection>` for one object, `Objects moved to
//!   <Collection>` otherwise (also with nothing selected, which still finishes); the master
//!   collection is called `Scene Collection`;
//! - `link_to_collection` links instead (`<Object> linked to <Collection>`, `Objects linked to
//!   …`), and with exactly one selected object already in the target fails with `Error: <Object>
//!   already in <Collection>`.
//!
//! frap's master collection has `session_uid` 0 (Blender gives it a real one), so
//! `collection_uid=0` targets the master collection in frap; read the uid from
//! `bpy.context.scene.collection.session_uid` to stay portable.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use self::common::{CollectionOp, run_collection_op};

/// Collection helpers shared by the WP 1.20 operators (`move_to_collection`,
/// `link_to_collection`, `collection_*`, `duplicate`, `join`).
pub(crate) mod common {
    use frap_op::prelude::*;
    use frap_rna::{CollectionId, IdRef, ObjectId, SceneId};
    use frap_scene::id::{Collection, layers};
    use frap_scene::{ChangeLog, Main};
    use smol_str::SmolStr;

    /// A collection of a scene: `None` is the scene's (embedded) master collection.
    pub(crate) type SceneColl = Option<IdRef<CollectionId>>;

    /// Every collection below the master collection of `scene`, depth first in `children`
    /// order, each once (`FOREACH_SCENE_COLLECTION` without the master).
    pub(crate) fn scene_collections(
        main: &Main,
        scene: IdRef<SceneId>,
    ) -> Vec<IdRef<CollectionId>> {
        let Some(s) = main.scenes.get(scene) else {
            return Vec::new();
        };
        let mut out: Vec<IdRef<CollectionId>> = Vec::new();
        let mut stack: Vec<IdRef<CollectionId>> =
            s.collection.children.iter().rev().copied().collect();
        while let Some(c) = stack.pop() {
            if out.contains(&c) {
                continue;
            }
            let Some(col) = main.collections.get(c) else {
                continue;
            };
            out.push(c);
            stack.extend(col.children.iter().rev().copied());
        }
        out
    }

    /// The master collection then every collection of the scene (`FOREACH_SCENE_COLLECTION`).
    pub(crate) fn all_scene_collections(main: &Main, scene: IdRef<SceneId>) -> Vec<SceneColl> {
        std::iter::once(None)
            .chain(scene_collections(main, scene).into_iter().map(Some))
            .collect()
    }

    /// A collection of the scene (read).
    pub(crate) fn get(main: &Main, scene: IdRef<SceneId>, c: SceneColl) -> Option<&Collection> {
        match c {
            None => main.scenes.get(scene).map(|s| &s.collection),
            Some(r) => main.collections.get(r),
        }
    }

    /// A collection of the scene (write: COW, bumped and logged).
    pub(crate) fn get_mut<'m>(
        main: &'m mut Main,
        log: &mut ChangeLog,
        scene: IdRef<SceneId>,
        c: SceneColl,
    ) -> Option<&'m mut Collection> {
        match c {
            None => main.scenes.get_mut(scene, log).map(|s| &mut s.collection),
            Some(r) => main.collections.get_mut(r, log),
        }
    }

    /// `BKE_collection_ui_name_get`: the master collection is `Scene Collection`.
    pub(crate) fn ui_name(main: &Main, scene: IdRef<SceneId>, c: SceneColl) -> SmolStr {
        match c {
            None => "Scene Collection".into(),
            Some(_) => get(main, scene, c)
                .map(|col| col.header.name.clone())
                .unwrap_or_default(),
        }
    }

    /// True when the collection holds `ob` directly.
    pub(crate) fn has_object(
        main: &Main,
        scene: IdRef<SceneId>,
        c: SceneColl,
        ob: IdRef<ObjectId>,
    ) -> bool {
        get(main, scene, c).is_some_and(|col| col.objects.contains(&ob))
    }

    /// `collection_object_add`: appends `ob` unless the collection already holds it or is
    /// linked from a library. Returns whether it was added. The caller resyncs the layers.
    pub(crate) fn link_object(
        main: &mut Main,
        log: &mut ChangeLog,
        scene: IdRef<SceneId>,
        c: SceneColl,
        ob: IdRef<ObjectId>,
    ) -> bool {
        let editable = get(main, scene, c).is_some_and(|col| col.header.library.is_none());
        if !editable || has_object(main, scene, c, ob) {
            return false;
        }
        if let Some(col) = get_mut(main, log, scene, c) {
            col.objects.push(ob);
            return true;
        }
        false
    }

    /// `scene_collections_object_remove`: unlinks `ob` from every collection of the scene
    /// (master included) except `keep`, touching only the collections that hold it, then
    /// resyncs the view layers.
    pub(crate) fn unlink_from_scene(
        main: &mut Main,
        log: &mut ChangeLog,
        scene: IdRef<SceneId>,
        ob: IdRef<ObjectId>,
        keep: Option<SceneColl>,
    ) {
        for c in all_scene_collections(main, scene) {
            if Some(c) == keep || !has_object(main, scene, c, ob) {
                continue;
            }
            if let Some(col) = get_mut(main, log, scene, c) {
                col.objects.retain(|o| *o != ob);
            }
        }
        layers::sync_all(main, log);
    }

    /// `ED_object_base_free_and_unlink`: unlinks `ob` from the scene's collections and frees it
    /// once nothing uses it (children lose their parent, `scene.camera` and the active object
    /// are cleared by `Main::remove`). Its data is kept (an orphan when unused).
    pub(crate) fn free_and_unlink(
        main: &mut Main,
        log: &mut ChangeLog,
        scene: IdRef<SceneId>,
        ob: IdRef<ObjectId>,
    ) {
        if !main.objects.contains(ob) {
            return;
        }
        unlink_from_scene(main, log, scene, ob, None);
        if main.users(ob.any()) == 0 {
            main.remove(ob.any(), log);
        }
    }

    /// `BKE_collection_add(bmain, parent, name)`: a new collection (empty name →
    /// `Collection`, uniquified) appended to `parent`'s children, layers resynced.
    pub(crate) fn add_child(
        main: &mut Main,
        log: &mut ChangeLog,
        scene: IdRef<SceneId>,
        parent: SceneColl,
        name: &str,
    ) -> IdRef<CollectionId> {
        let c = main.collections.insert(name, Collection::new(name), log);
        if let Some(p) = get_mut(main, log, scene, parent) {
            p.children.push(c);
        }
        layers::sync_all(main, log);
        c
    }

    /// `BKE_collection_from_session_uid`: the master collection or a collection below it
    /// whose `session_uid` is `uid`.
    ///
    /// Candidate deviation (for docs/deviations.md): frap's master collection has no ID of its
    /// own, so `scene.collection.session_uid` reads 0 (35 in Blender's factory scene) and
    /// `collection_uid=0` targets it, where Blender raises `Unexpected error, collection not
    /// found` for a literal 0. Matching 0 keeps `collection_uid=scene.collection.session_uid`
    /// working in both.
    pub(crate) fn from_session_uid(
        main: &Main,
        scene: IdRef<SceneId>,
        uid: i64,
    ) -> Option<SceneColl> {
        all_scene_collections(main, scene).into_iter().find(|c| {
            get(main, scene, *c).is_some_and(|col| i64::from(col.header.session_uid as i32) == uid)
        })
    }

    /// Which of the two operators runs.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub(crate) enum CollectionOp {
        /// `object.move_to_collection`
        Move,
        /// `object.link_to_collection`
        Link,
    }

    /// `move_to_collection_exec` for both operators.
    pub(crate) fn run_collection_op(
        cx: &mut OpCtx,
        what: CollectionOp,
        uid: &frap_rna::Prop<i32>,
        is_new: bool,
        new_name: &str,
    ) -> OpResult {
        if !uid.is_set {
            return Err(OpError::Runtime("No collection selected".into()));
        }
        let (scene, objects) = {
            let c = cx.ctx();
            (c.win.scene, c.selected_objects())
        };
        let Some(mut target) = from_session_uid(cx.main(), scene, i64::from(uid.value)) else {
            return Err(OpError::Runtime(
                "Unexpected error, collection not found".into(),
            ));
        };
        if get(cx.main(), scene, target).is_some_and(|c| c.header.library.is_some()) {
            return Err(OpError::Runtime(
                "Cannot add objects to a library override or linked collection".into(),
            ));
        }
        let (main, log) = cx.main_mut();
        if is_new {
            target = Some(add_child(main, log, scene, target, new_name));
        }
        let name = ui_name(main, scene, target);
        if let [single] = objects.as_slice()
            && what == CollectionOp::Link
            && has_object(main, scene, target, *single)
        {
            let ob = main.objects.name_of(*single).unwrap_or_default().to_owned();
            return Err(OpError::Runtime(format!("{ob} already in {name}")));
        }
        for ob in &objects {
            link_object(main, log, scene, target, *ob);
            if what == CollectionOp::Move {
                unlink_from_scene(main, log, scene, *ob, Some(target));
            }
        }
        layers::sync_all(main, log);
        log.context_changed = true;
        let verb = match what {
            CollectionOp::Move => "moved",
            CollectionOp::Link => "linked",
        };
        let who = match objects.as_slice() {
            [single] => main.objects.name_of(*single).unwrap_or_default().to_owned(),
            _ => "Objects".to_owned(),
        };
        cx.report(ReportType::Info, format!("{who} {verb} to {name}"));
        Ok(OpStatus::Finished)
    }
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    notes = [
        "collection_uid is a session_uid: pass bpy.data.collections['Name'].session_uid or bpy.context.scene.collection.session_uid",
    ]
)]
fn exec(cx: &mut OpCtx, p: object::MoveToCollection) -> OpResult {
    run_collection_op(
        cx,
        CollectionOp::Move,
        &p.collection_uid,
        p.is_new.value,
        &p.new_collection_name.value,
    )
}
