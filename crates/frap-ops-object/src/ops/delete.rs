// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.delete(use_global=False, confirm=True)`, tier T0
//! (`tests/golden/cases/object/delete/*.py`). Port of `object_delete_exec`
//! (`editors/object/object_add.cc`, Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4:
//! - deletes `context.selected_objects` (selected **and visible**; a selected object under a
//!   hidden layer collection is kept); nothing selected → `CANCELLED`;
//! - the object datablock is removed from `bpy.data.objects` when `use_global` is set or it is
//!   used by at most one collection; an object linked into collections of other scenes is only
//!   unlinked from the context scene's collections (and removed once nothing links it);
//! - its data is **not** freed: the mesh stays in `bpy.data.meshes` with `users == 0` (an
//!   orphan, dropped on save), so the next `primitive_cube_add` names its object `Cube` and its
//!   mesh `Cube.001`;
//! - references to it are cleared: children lose their parent (they keep their local
//!   transform and `matrix_parent_inverse`, so they move in world space), `scene.camera` and
//!   the active object become `None`;
//! - an `INFO` report `Deleted N object(s)`.
//!
//! `confirm` only matters for the interactive invoke. Libraries in frap have no indirectly
//! linked objects, so Blender's two refusals for those (`Cannot delete indirectly linked
//! object '…'`) cannot occur.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CollectionId, IdRef, ObjectId, SceneId};
use frap_scene::id::layers;
use frap_scene::{ChangeLog, Main};
use frap_spec_ops_object::object;

/// Every collection of `scene` below its master collection (depth first, each once).
fn scene_collections(main: &Main, scene: IdRef<SceneId>) -> Vec<IdRef<CollectionId>> {
    let Some(s) = main.scenes.get(scene) else {
        return Vec::new();
    };
    let mut out: Vec<IdRef<CollectionId>> = Vec::new();
    let mut stack: Vec<IdRef<CollectionId>> = s.collection.children.iter().rev().copied().collect();
    while let Some(c) = stack.pop() {
        if out.contains(&c) {
            continue;
        }
        out.push(c);
        if let Some(col) = main.collections.get(c) {
            stack.extend(col.children.iter().rev().copied());
        }
    }
    out
}

/// `ED_object_base_free_and_unlink_no_indirect_check`: unlinks `ob` from every collection of
/// `scene` (master included), touching only the collections that hold it, then resyncs the
/// view layers.
fn unlink_from_scene(main: &mut Main, log: &mut ChangeLog, scene: IdRef<SceneId>, ob: IdRef<ObjectId>) {
    for c in scene_collections(main, scene) {
        if main.collections.get(c).is_some_and(|col| col.objects.contains(&ob))
            && let Some(col) = main.collections.get_mut(c, log)
        {
            col.objects.retain(|o| *o != ob);
        }
    }
    if main
        .scenes
        .get(scene)
        .is_some_and(|s| s.collection.objects.contains(&ob))
        && let Some(s) = main.scenes.get_mut(scene, log)
    {
        s.collection.objects.retain(|o| *o != ob);
    }
    layers::sync_all(main, log);
}

#[operator(poll = poll::OBJECTMODE, tier = T0, spec = object::OPSPEC_OBJECT_OT_DELETE)]
fn exec(cx: &mut OpCtx, p: object::ObjectDelete) -> OpResult {
    let (selected, scene) = {
        let c = cx.ctx();
        (c.selected_objects(), c.win.scene)
    };
    if selected.is_empty() {
        cx.report(
            ReportType::Warning,
            "frap: nothing deleted: no visible object is selected",
        );
        return Ok(OpStatus::Cancelled);
    }
    let use_global = p.use_global.value;
    let (main, log) = cx.main_mut();
    let mut count = 0usize;
    for ob in selected {
        if !main.objects.contains(ob) {
            continue;
        }
        let a = ob.any();
        // `use_global || ID_REAL_USERS(ob) <= 1`: tagged for a full delete.
        if use_global || main.users(a) <= 1 {
            main.remove(a, log);
        } else {
            unlink_from_scene(main, log, scene, ob);
            if main.users(a) == 0 {
                main.remove(a, log);
            }
        }
        count += 1;
    }
    log.context_changed = true;
    cx.report(ReportType::Info, format!("Deleted {count} object(s)"));
    Ok(OpStatus::Finished)
}
