// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.collection_add()`, tier T0 (`tests/golden/cases/object/collection/*.py`).
//! Port of `collection_add_exec` (`editors/object/object_collection.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4: creates `Collection` (uniquified) with a fake user,
//! linked to no scene, and links the active object (`context.object`) to it; without an
//! active object it is `CANCELLED`. Poll `ED_operator_objectmode`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::Collection;
use frap_spec_ops_object::object;

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::CollectionAdd) -> OpResult {
    let Some(ob) = cx.ctx().object() else {
        cx.report(
            ReportType::Warning,
            "frap: nothing added: collection_add links the active object, and there is none",
        );
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    let mut col = Collection::new("Collection");
    col.header.use_fake_user = true;
    col.objects.push(ob);
    main.collections.insert("Collection", col, log);
    Ok(OpStatus::Finished)
}
