// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_weight_set_active(weight_group=-1)`, tier T1 (`tests/golden/cases/
//! object/vgroups_m8/weights_active_vertex.py`). Port of `vertex_weight_set_active_exec`
//! (`editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later): group `weight_group`
//! becomes the active vertex group (`-1`, the default, changes nothing). As in Blender the
//! index is not checked against the number of groups.
//!
//! Poll `vertex_group_vert_select_mesh_poll`: a mesh in Edit Mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn set_active_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_weight_set_active")
}

/// `vertex_group_vert_select_mesh_poll`.
const SET_ACTIVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: set_active_poll,
};

#[operator(poll = SET_ACTIVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::VertexWeightSetActive) -> OpResult {
    let Some((ob, _)) = poll::active(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let wg = p.weight_group.value;
    if wg != -1 {
        cx.object_mut(ob)?.vertex_groups_active_index = wg;
    }
    Ok(OpStatus::Finished)
}
