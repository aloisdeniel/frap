// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.scene.delete()`, tier T0 (`tests/golden/cases/scenes/*.py`; M3 WP 3.10). Port of
//! `scene_delete_poll`, `scene_delete_exec` and `ED_scene_delete`
//! (`editors/scene/scene_edit.cc`, Blender 5.2.4, GPL-2.0-or-later): deletes the context
//! scene after switching the window to its neighbour (`BKE_scene_find_replacement`: the
//! previous scene in `bpy.data.scenes`, else the next); its objects and collections stay in
//! `bpy.data`. The poll (`BKE_scene_can_be_removed`) fails while the context scene is the only
//! local scene (verified on 5.2.4: `Operator bpy.ops.scene.delete.poll() failed, context is
//! incorrect`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_scene::scenes::{can_be_removed, remove_scene};
use frap_spec_ops_object::scene;

/// `scene_delete_poll`: another local scene exists.
fn delete_poll(c: &Context) -> Result<(), PollFail> {
    if can_be_removed(c.main, c.win.scene) {
        return Ok(());
    }
    // No `PollReq` names "another scene exists": the failure lists no unmet requirement and
    // the hint says what to add.
    let mut f = poll::fail(
        PollReq::SceneEditable,
        "the last local scene cannot be deleted: add one first, e.g. \
         bpy.ops.scene.new(type='NEW')",
    );
    f.unmet.clear();
    Err(f)
}

/// `scene_delete_poll`
const DELETE_POLL: Poll = Poll {
    requires: &[],
    check: delete_poll,
};

#[operator(
    poll = DELETE_POLL,
    spec = scene::OPSPEC_SCENE_OT_DELETE,
    tier = T0,
    notes = ["deletes bpy.context.scene; the window shows the previous scene of bpy.data.scenes (else the next)"]
)]
fn exec(cx: &mut OpCtx, _p: scene::SceneDelete) -> OpResult {
    let scene = cx.ctx().win.scene;
    let mut win = cx.ctx().win.clone();
    let removed = {
        let (main, log) = cx.main_mut();
        remove_scene(main, &mut win, scene, log).is_ok()
    };
    if !removed {
        cx.report(
            ReportType::Warning,
            "frap: nothing deleted: the scene is the last local one",
        );
        return Ok(OpStatus::Cancelled);
    }
    *cx.win_mut() = win;
    // The replacement scene's depsgraph is evaluated at its frame by the closing boundary
    // (`ED_scene_change_update`; verified on 5.2.4).
    cx.mark_time_dirty();
    Ok(OpStatus::Finished)
}
