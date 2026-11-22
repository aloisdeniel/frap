// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.collection.objects_remove(collection=…)`, tier T0
//! (`tests/golden/cases/object/data_m8/collections.py`). Port of
//! `collection_objects_remove_exec` (`editors/object/object_collection.cc`, Blender 5.2.4,
//! GPL-2.0-or-later); the `collection` items are `objects_add_active`'s.
//!
//! Verified on 5.2.4:
//! - unset: the first collection holding the active object (`context.object`);
//!   `All Collections`: every one, in `FOREACH_COLLECTION` order (master collection first);
//! - every selected editable object leaves each such collection. The selection is read again
//!   for each collection (`CTX_DATA_BEGIN` inside the loop): once the selected objects lost
//!   their last collection of the scene they have no base, so the later collections (e.g. ones
//!   outside the scene) keep them;
//! - the objects stay in `bpy.data` (orphans when no collection holds them); an active object
//!   that loses its base leaves `context.object` `None`;
//! - no active object, or no collection holding it: `CANCELLED`; poll
//!   `ED_operator_objectmode`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::Context;
use frap_scene::id::layers;
use frap_spec_ops_object::collection;

use super::collection__objects_add_active::common;

/// The body of `objects_remove` and `objects_remove_active`: removes the selected editable
/// objects from the collections `choice` picks among those holding `ob`. Returns whether any
/// collection had a selected object to process (`changed_multi`).
pub(crate) fn remove_selected(
    cx: &mut OpCtx,
    ob: frap_rna::IdRef<frap_rna::ObjectId>,
    choice: common::CollectionChoice,
) -> bool {
    let scene = cx.ctx().win.scene;
    let mut changed_multi = false;
    let collections = common::foreach_collection(cx.main());
    for col in collections {
        if !common::wanted(choice, col) || !common::has_object(cx.main(), scene, col, ob) {
            continue;
        }
        // `CTX_DATA_BEGIN (C, Base *, base, selected_editable_bases)`, read now.
        let selected = cx.ctx().selected_editable_objects();
        if selected.is_empty() {
            continue;
        }
        let (main, log) = cx.main_mut();
        let mut touched_scene = false;
        for other in selected {
            if common::unlink_object(main, log, scene, col, other) {
                touched_scene |= common::in_scene(main, scene, col);
            }
        }
        // `BKE_collection_object_remove` resyncs when the collection is in a scene.
        if touched_scene {
            layers::sync_all(main, log);
        }
        changed_multi = true;
    }
    changed_multi
}

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    dyn_enums = [("collection", common::items)],
    notes = ["collection items: the collections holding the active object (Scene Collection first), after 'All Collections' when there are two or more; unset is the first of them"]
)]
fn exec(cx: &mut OpCtx, p: collection::ObjectsRemove) -> OpResult {
    let (ob, choice) = {
        let c: Context = cx.ctx();
        let ob = c.object();
        (ob, common::choice(&c, &p.collection, ob)?)
    };
    let Some(ob) = ob else {
        cx.report(
            ReportType::Warning,
            "frap: nothing removed: objects_remove needs an active object",
        );
        return Ok(OpStatus::Cancelled);
    };
    if !remove_selected(cx, ob, choice) {
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
