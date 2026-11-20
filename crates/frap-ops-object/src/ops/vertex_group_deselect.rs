// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_deselect()`, tier T0 (Edit Mode). Port of
//! `vertex_group_deselect_exec` (`vgroup_select_verts(…, 0)`, `editors/object/object_vgroup.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): deselects every visible vertex assigned to the active
//! group, then edges and faces that lost a selected vertex (`EDBM_select_flush_from_verts`).
//! Poll `vertex_group_vert_select_poll` (Edit Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use super::vertex_group_select::select_verts;

fn deselect_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_group_deselect")
}

/// `vertex_group_vert_select_poll`.
const DESELECT_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: deselect_poll,
};

#[operator(poll = DESELECT_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupDeselect) -> OpResult {
    select_verts(cx, false)?;
    Ok(OpStatus::Finished)
}
