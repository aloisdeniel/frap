// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_assign_new()`, tier T0 (Edit Mode). Port of
//! `vertex_group_assign_new_exec` (`editors/object/object_vgroup.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): adds a group (`Group`, unique, made active), then assigns the selected
//! vertices to it with `tool_settings.vertex_group_weight` (`vertex_group_assign`). Poll
//! `vertex_group_vert_select_poll` (Edit Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use super::vertex_group_assign::assign_active;

fn assign_new_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_group_assign_new")
}

/// `vertex_group_vert_select_poll`.
const ASSIGN_NEW_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: assign_new_poll,
};

#[operator(poll = ASSIGN_NEW_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupAssignNew) -> OpResult {
    let c = cx.ctx();
    let Some((ob, o)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(me) = common::mesh_of(&c, o) else {
        return Ok(OpStatus::Cancelled);
    };
    common::add_group(cx, ob, me)?;
    assign_active(cx)
}
