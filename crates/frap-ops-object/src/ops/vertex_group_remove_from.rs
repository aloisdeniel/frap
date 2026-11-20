// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_remove_from(use_all_groups=False, use_all_verts=False)`, tier
//! T0 (Edit Mode). Port of `vertex_group_remove_from_exec` and `BKE_object_defgroup_clear(_all)`
//! (`editors/object/object_vgroup.cc`, `blenkernel/intern/object_deform.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): removes the selected vertices (every vertex with `use_all_verts`) from
//! the active group, or from every group with `use_all_groups` (which always works on the
//! selection). `CANCELLED` when the mesh has no weights or there is no active group. Poll
//! `vertex_group_vert_select_unlocked_poll` (Edit Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn remove_from_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_unlocked_poll(c, "object.vertex_group_remove_from")
}

/// `vertex_group_vert_select_unlocked_poll`.
const REMOVE_FROM_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: remove_from_poll,
};

#[operator(poll = REMOVE_FROM_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::VertexGroupRemoveFrom) -> OpResult {
    let Some((_, me, act)) = common::active_group(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let changed = if p.use_all_groups.value {
        let n = cx
            .main()
            .meshes
            .get(me)
            .map_or(0, |m| m.vertex_group_names.len());
        let mut changed = false;
        for g in 0..n {
            changed |= common::clear_group(cx, me, g, true)?;
        }
        changed
    } else {
        match act {
            Some(g) => common::clear_group(cx, me, g, !p.use_all_verts.value)?,
            None => false,
        }
    };
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
