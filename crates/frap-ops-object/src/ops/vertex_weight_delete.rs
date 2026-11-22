// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_weight_delete(weight_group=-1)`, tier T1 (`tests/golden/cases/
//! object/vgroups_m8/weights_active_vertex.py`). Port of `vertex_weight_delete_exec` /
//! `vgroup_remove_weight` (`editors/object/object_vgroup.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): in Edit Mode the active vertex (the last vertex of the selection history)
//! leaves group `weight_group`. A missing group (`-1`, the default, included) raises
//! `RuntimeError: Error: Invalid vertex group index`, a locked one `Error: Vertex group is
//! locked`; nothing happens without an active vertex.
//!
//! Poll `vertex_group_vert_select_mesh_poll`: a mesh in Edit Mode.

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Weights};

fn weight_delete_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_weight_delete")
}

/// `vertex_group_vert_select_mesh_poll`.
const WEIGHT_DELETE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: weight_delete_poll,
};

#[operator(poll = WEIGHT_DELETE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::VertexWeightDelete) -> OpResult {
    let Some((_, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let g = weights::check_group_accessible(cx, me, p.weight_group.value)?;
    let Some(act) = weights::active_vert(cx, me) else {
        return Ok(OpStatus::Finished);
    };
    let Some(mut w) = Weights::load(cx, me) else {
        return Ok(OpStatus::Finished);
    };
    deform::remove_group(&mut w.dv[act], g as u32);
    w.store(cx)?;
    Ok(OpStatus::Finished)
}
