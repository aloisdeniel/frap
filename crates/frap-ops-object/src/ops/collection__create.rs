// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.collection.create(name='')`, tier T0
//! (`tests/golden/cases/object/data_m8/collections.py`). Port of `collection_create_exec`
//! (`editors/object/object_collection.cc`) over `BKE_collection_add(bmain, nullptr, name)`
//! (`blenkernel/intern/collection.cc`), Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4:
//! - a new collection named `name` (empty: `Collection`, uniquified: `Collection.001`) with a
//!   fake user, linked to no scene (`users == 1`);
//! - every selected object (`selected_bases`: editable or not) is linked to it;
//! - always `FINISHED` (also with nothing selected); poll `ED_operator_objectmode`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::Collection;
use frap_spec_ops_object::collection;

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: collection::Create) -> OpResult {
    let selected = cx.ctx().selected_objects();
    let (main, log) = cx.main_mut();
    // `collection_add`: the requested name, `Collection` when empty (`BKE_id_new`).
    let mut col = Collection::new(p.name.value.as_str());
    col.header.use_fake_user = true;
    for ob in selected {
        if !col.objects.contains(&ob) {
            col.objects.push(ob);
        }
    }
    main.collections.insert(p.name.value.as_str(), col, log);
    Ok(OpStatus::Finished)
}
