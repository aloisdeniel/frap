// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.link_to_collection(collection_uid=-1, is_new=False,
//! new_collection_name='')`, tier T0 (`tests/golden/cases/object/collection/*.py`): links
//! `context.selected_objects` to the collection with that `session_uid` (or to a new child of
//! it) without unlinking them elsewhere. Same exec as `object.move_to_collection`
//! (`move_to_collection_exec`, `editors/object/object_edit.cc`, Blender 5.2.4,
//! GPL-2.0-or-later); see `move_to_collection.rs` for the verified behaviour.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use super::move_to_collection::common::{CollectionOp, run_collection_op};

#[operator(
    poll = poll::OBJECTMODE,
    tier = T0,
    notes = [
        "collection_uid is a session_uid: pass bpy.data.collections['Name'].session_uid or bpy.context.scene.collection.session_uid",
    ]
)]
fn exec(cx: &mut OpCtx, p: object::LinkToCollection) -> OpResult {
    run_collection_op(
        cx,
        CollectionOp::Link,
        &p.collection_uid,
        p.is_new.value,
        &p.new_collection_name.value,
    )
}
