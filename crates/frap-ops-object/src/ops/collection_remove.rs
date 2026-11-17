// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.collection_remove()`, tier T0 (`tests/golden/cases/object/collection/*.py`).
//! Port of `collection_remove_exec` (`editors/object/object_collection.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): it removes the active object from the collection of the Properties
//! editor's context pointer (`CTX_data_pointer_get_type(C, "collection", …)`, the collection
//! panel of the Object properties). Scripts and the 3D View have no such pointer, so the
//! operator is always `CANCELLED` there (verified on 5.2.4 in a VIEW_3D context, with the
//! object in the active collection). frap has no Properties editor: same result, with a
//! WARNING naming the data-API call that does it. Poll `ED_operator_objectmode`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    notes = ["always CANCELLED outside the Properties editor (as Blender); use bpy.data.collections['C'].objects.unlink(obj)"]
)]
fn exec(cx: &mut OpCtx, _p: object::CollectionRemove) -> OpResult {
    cx.report(
        ReportType::Warning,
        "frap: nothing removed: collection_remove needs the Properties editor's collection \
         (Blender cancels it in scripts too); use \
         bpy.data.collections['Name'].objects.unlink(bpy.context.object)",
    );
    Ok(OpStatus::Cancelled)
}
