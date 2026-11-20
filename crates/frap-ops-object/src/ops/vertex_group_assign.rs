// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_assign()`, tier T0 (Edit Mode). Port of
//! `vertex_group_assign_exec` and `vgroup_assign_verts` (`editors/object/object_vgroup.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): every selected vertex of the edit mesh gets
//! `tool_settings.vertex_group_weight` in the active group (added to the vertex when missing,
//! the weight layer created when the mesh has none). No active group: `RuntimeError: Error: No
//! active vertex group to operate on`. `use_auto_normalize` is off in the factory settings and
//! not bound by frap. Poll `vertex_group_vert_select_unlocked_poll` (Edit Mode).

use frap_bmesh::ElemFlag;
use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_rna::{IdRef, MeshId};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn assign_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_unlocked_poll(c, "object.vertex_group_assign")
}

/// `vertex_group_vert_select_unlocked_poll`.
const ASSIGN_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: assign_poll,
};

/// `vgroup_assign_verts` on the edit mesh of `me` (`def_nr = g`).
pub(crate) fn assign_verts(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    g: usize,
    weight: f32,
) -> Result<(), OpError> {
    let bm = cx.bmesh_mut(me)?;
    let ids = bm.vert_ids();
    let selected: Vec<_> = ids
        .into_iter()
        .filter(|&v| bm.vert(v).flag.contains(ElemFlag::SELECT))
        .collect();
    let d = deform::bmesh_deform_mut(bm);
    for v in selected {
        if let Some(dv) = d.get_mut(v.slot()) {
            *deform::ensure(dv, g as u32) = weight;
        }
    }
    Ok(())
}

/// `vertex_group_assign_exec`.
pub(crate) fn assign_active(cx: &mut OpCtx) -> OpResult {
    let weight = cx.ctx().tool_settings().vertex_group_weight;
    let Some((_, me, act)) = common::active_group(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(g) = act else {
        return Err(OpError::Runtime(
            "No active vertex group to operate on".into(),
        ));
    };
    if !common::in_edit(cx, me) {
        return Ok(OpStatus::Cancelled);
    }
    assign_verts(cx, me, g, weight)?;
    Ok(OpStatus::Finished)
}

#[operator(poll = ASSIGN_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupAssign) -> OpResult {
    assign_active(cx)
}
