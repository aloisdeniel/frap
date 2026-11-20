// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_remove(all=False, all_unlocked=False)`, tier T0. Port of
//! `vertex_group_remove_exec` (`vgroup_delete_active` / `BKE_object_defgroup_remove_all_ex`,
//! `editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later): removes the active
//! group, every group (`all`) or every unlocked group (`all_unlocked`, which wins over `all`).
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
        // `BKE_object_defgroup_remove_all_ex(ob, only_unlocked)`: the groups in order.
        let only_unlocked = p.all_unlocked.value;
        let mut g = 0;
        while let Some(locked) = cx
            .main()
            .meshes
            .get(me)
            .filter(|m| g < m.vertex_group_names.len())
            .map(|m| m.vertex_group_locked(g))
        {
            if only_unlocked && locked {
                g += 1;
            } else {
                common::remove_group(cx, ob, me, g)?;
            }
        }
    } else if let Some(g) = act {
        common::remove_group(cx, ob, me, g)?;
    }
    Ok(OpStatus::Finished)
}
