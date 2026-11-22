// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.collection.objects_remove_all()`, tier T0
//! (`tests/golden/cases/object/data_m8/collections.py`). Port of
//! `collection_objects_remove_all_exec` (`editors/object/object_collection.cc`) and
//! `BKE_object_groups_clear` (`blenkernel/intern/object.cc`), Blender 5.2.4,
//! GPL-2.0-or-later. Verified on 5.2.4: every selected editable object (read once, before
//! anything changes) leaves every collection holding it: the scene's master collection and
//! every collection of `bpy.data.collections`, in the scene or not. The objects stay in
//! `bpy.data` without a base (the active object then reads `None`). Always `FINISHED`; poll
//! `ED_operator_objectmode`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::layers;
use frap_spec_ops_object::collection;

use super::collection__objects_add_active::common;

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: collection::ObjectsRemoveAll) -> OpResult {
    let (scene, selected) = {
        let c = cx.ctx();
        (c.win.scene, c.selected_editable_objects())
    };
    let (main, log) = cx.main_mut();
    let mut changed = false;
    for ob in selected {
        for col in common::find_order(main, scene, ob) {
            changed |= common::unlink_object(main, log, scene, col, ob);
        }
    }
    if changed {
        layers::sync_all(main, log);
    }
    Ok(OpStatus::Finished)
}
