// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.collection_unlink()`, tier T0 (`tests/golden/cases/object/collection/*.py`).
//! Port of `collection_unlink_exec` (`editors/object/object_collection.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Verified on 5.2.4 in a VIEW_3D context: `context.collection` (the
//! active layer collection's collection) is **deleted** (`BKE_id_delete`): it leaves
//! `bpy.data.collections`, its objects stay in `bpy.data` (without a base when no other
//! collection of the scene holds them; the active object then becomes `None`), its child
//! collections are unlinked (orphans), and the active layer collection falls back to the
//! scene collection. The poll fails when `context.collection` is the scene's master collection
//! (and outside Object Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

/// `ED_operator_objectmode` and a `context.collection` that is not the master collection.
fn unlink_poll(c: &Context) -> Result<(), PollFail> {
    poll::objectmode(c)?;
    if c.collection().is_some() {
        return Ok(());
    }
    let hint = match c.main.collections.iter().next() {
        Some((_, col)) => format!(
            "bpy.context.view_layer.active_layer_collection = \
             bpy.context.view_layer.layer_collection.children[{}]",
            poll::py_name(&col.header.name)
        ),
        None => {
            "bpy.context.view_layer.active_layer_collection must not be the scene collection".into()
        }
    };
    // No `PollReq` names "a collection other than the scene collection is active": the
    // failure lists no unmet requirement and the hint says what to change.
    let mut f = poll::fail(PollReq::SceneEditable, hint);
    f.unmet.clear();
    Err(f)
}

/// `collection_unlink` poll.
const UNLINK_POLL: Poll = Poll {
    requires: &[PollReq::SceneEditable, PollReq::ModeObject],
    check: unlink_poll,
};

#[operator(
    poll = UNLINK_POLL,
    tier = T0,
    notes = ["deletes bpy.context.collection (the active layer collection's collection), as Blender"]
)]
fn exec(cx: &mut OpCtx, _p: object::CollectionUnlink) -> OpResult {
    let Some(col) = cx.ctx().collection() else {
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    main.remove(col.any(), log);
    log.context_changed = true;
    Ok(OpStatus::Finished)
}
