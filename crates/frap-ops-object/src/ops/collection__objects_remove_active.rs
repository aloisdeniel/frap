// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.collection.objects_remove_active(collection=…)`, tier T0
//! (`tests/golden/cases/object/data_m8/collections.py`). Port of
//! `objects_remove_active_exec` (`editors/object/object_collection.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). The same removal as `collection.objects_remove` (see there), with two
//! differences verified on 5.2.4:
//! - the active object is the view layer's (`BKE_view_layer_active_object_get`): one without
//!   a base (removed from every collection of the scene) is `CANCELLED`;
//! - when no collection had a selected object to remove, the ERROR `Active object contains no
//!   collections` is reported and the operator still finishes, which `bpy.ops` raises as
//!   `RuntimeError: Error: Active object contains no collections` (frap raises the same; the
//!   statement changed nothing).

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::Context;
use frap_spec_ops_object::collection;

use super::collection__objects_add_active::common;
use super::collection__objects_remove::remove_selected;

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    dyn_enums = [("collection", common::items)],
    notes = ["collection items: the collections holding the active object (Scene Collection first), after 'All Collections' when there are two or more; unset is the first of them"]
)]
fn exec(cx: &mut OpCtx, p: collection::ObjectsRemoveActive) -> OpResult {
    let (ob, choice) = {
        let c: Context = cx.ctx();
        // `BKE_view_layer_active_object_get`: the active base's object.
        let ob = c
            .active_object()
            .filter(|&o| c.view_layer().base(o).is_some());
        (ob, common::choice(&c, &p.collection, ob)?)
    };
    let Some(ob) = ob else {
        cx.report(
            ReportType::Warning,
            "frap: nothing removed: objects_remove_active needs an active object in the view layer",
        );
        return Ok(OpStatus::Cancelled);
    };
    if !remove_selected(cx, ob, choice) {
        return Err(OpError::Runtime(
            "Active object contains no collections".into(),
        ));
    }
    Ok(OpStatus::Finished)
}
