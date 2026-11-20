// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_select()`, tier T0 (Edit Mode). Port of
//! `vertex_group_select_exec` and `vgroup_select_verts` (`editors/object/object_vgroup.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): selects every visible vertex of the edit mesh assigned to
//! the active group (any weight), then flushes the selection to edges and faces
//! (`EDBM_select_flush_from_verts`). Without an active group or a weight layer nothing
//! changes (`FINISHED`). Poll `vertex_group_vert_select_poll` (Edit Mode).

use frap_bmesh::ElemFlag;
use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn select_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_group_select")
}

/// `vertex_group_vert_select_poll`.
const SELECT_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: select_poll,
};

/// `vgroup_select_verts(…, select)` on the active object's edit mesh.
pub(crate) fn select_verts(cx: &mut OpCtx, select: bool) -> Result<(), OpError> {
    let Some((_, me, Some(g))) = common::active_group(cx) else {
        return Ok(());
    };
    if !common::in_edit(cx, me) {
        return Ok(());
    }
    let has = {
        let (_, edit) = cx.split_edit();
        edit.meshes
            .get(&me.key())
            .is_some_and(|em| em.bm.vdata.deform.is_some())
    };
    if !has {
        return Ok(());
    }
    let bm = cx.bmesh_mut(me)?;
    let hits: Vec<_> = bm
        .vert_ids()
        .into_iter()
        .filter(|&v| {
            !bm.vert(v).flag.contains(ElemFlag::HIDDEN)
                && bm
                    .vdata
                    .deform
                    .as_ref()
                    .and_then(|d| d.get(v.slot()))
                    .is_some_and(|dv| deform::find(dv, g as u32).is_some())
        })
        .collect();
    for v in hits {
        bm.vert_select_set(v, select);
    }
    if select {
        bm.select_flush();
    } else {
        bm.deselect_flush();
    }
    Ok(())
}

#[operator(poll = SELECT_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupSelect) -> OpResult {
    select_verts(cx, true)?;
    Ok(OpStatus::Finished)
}
