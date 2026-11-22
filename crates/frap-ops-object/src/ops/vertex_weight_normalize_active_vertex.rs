// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_weight_normalize_active_vertex()`, tier T1 (`tests/golden/cases/
//! object/vgroups_m8/weights_active_vertex.py`). Port of
//! `vertex_weight_normalize_active_vertex_exec` / `vgroup_normalize_active_vertex`
//! (`editors/object/object_vgroup.cc`, `BKE_defvert_normalize_lock_map`,
//! `blenkernel/intern/deform.cc`, Blender 5.2.4, GPL-2.0-or-later): in Edit Mode the weights of
//! the active vertex (the last vertex of the selection history) in the groups of
//! `ToolSettings.vertex_group_subset` (`ALL`) are scaled to sum to 1; locked groups keep their
//! weights and the others share what is left. Without an active vertex (or without weights)
//! the operator is `CANCELLED`.
//!
//! Poll `vertex_group_vert_select_mesh_poll`: a mesh in Edit Mode.

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Topology, Weights};

fn normalize_active_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_weight_normalize_active_vertex")
}

/// `vertex_group_vert_select_mesh_poll`.
const NORMALIZE_ACTIVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: normalize_active_poll,
};

#[operator(poll = NORMALIZE_ACTIVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::VertexWeightNormalizeActiveVertex) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(act) = weights::active_vert(cx, me) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(mut w) = Weights::load(cx, me) else {
        return Ok(OpStatus::Cancelled);
    };
    let (validmap, _) = {
        let c = cx.ctx();
        weights::subset(&c, ob, weights::tool_subset(&c))
    };
    let locks = weights::lock_flags(cx, me).unwrap_or_default();
    deform::normalize_ex(&mut w.dv[act], &validmap, &locks, &[]);
    let (use_mirror, use_topology) = weights::symmetry(cx, me);
    if use_mirror {
        let topo = Topology::load(cx, me).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
        let mirr = weights::mirror_map(&w, &topo, use_topology);
        let names = weights::group_names(cx, me);
        let all_locks = weights::locks(cx, me);
        weights::mirror_update_vert(&mut w, &mirr, act, None, &names, &all_locks);
    }
    w.store(cx)?;
    Ok(OpStatus::Finished)
}
