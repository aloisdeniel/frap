// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_remove(all=False, all_unlocked=False)`, tier T0. Port of
//! `vertex_group_remove_exec` (`vgroup_delete_active` / `BKE_object_defgroup_remove_all_ex`,
//! `editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later): removes the active
//! group (or every group; frap has no group locks, so `all_unlocked` removes every group too).
//! The weights go with the group (from the edit BMesh in Edit Mode), higher groups shift down
//! and the active index follows. Poll `vertex_group_poll` (`Object has no vertex groups`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn remove_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_remove")
}

/// `vertex_group_poll`.
const REMOVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: remove_poll,
};

#[operator(poll = REMOVE_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::VertexGroupRemove) -> OpResult {
    let Some((ob, me, act)) = common::active_group(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    if p.all.value || p.all_unlocked.value {
        while cx
            .main()
            .meshes
            .get(me)
            .is_some_and(|m| !m.vertex_group_names.is_empty())
        {
            common::remove_group(cx, ob, me, 0)?;
        }
    } else if let Some(g) = act {
        common::remove_group(cx, ob, me, g)?;
    }
    Ok(OpStatus::Finished)
}
