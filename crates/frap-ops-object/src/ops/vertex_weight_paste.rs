// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_weight_paste(weight_group=-1)`, tier T1 (`tests/golden/cases/object/
//! vgroups_m8/weights_active_vertex.py`). Port of `vertex_weight_paste_exec` /
//! `vgroup_copy_active_to_sel_single` (`editors/object/object_vgroup.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): in Edit Mode every other selected vertex takes the active vertex's weight
//! in group `weight_group` (0 where the active vertex is not in it and the vertex is). A
//! missing group (`-1`, the default, included) raises `RuntimeError: Error: Invalid vertex
//! group index`, a locked one `Error: Vertex group is locked`; nothing happens without an
//! active vertex.
//!
//! Poll `vertex_group_vert_select_mesh_poll`: a mesh in Edit Mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Topology, Weights, defvert};

fn weight_paste_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::vert_select_poll(c, "object.vertex_weight_paste")
}

/// `vertex_group_vert_select_mesh_poll`.
const WEIGHT_PASTE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
        PollReq::ModeEditMesh,
    ],
    check: weight_paste_poll,
};

#[operator(poll = WEIGHT_PASTE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::VertexWeightPaste) -> OpResult {
    let Some((_, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let g = weights::check_group_accessible(cx, me, p.weight_group.value)? as u32;
    let Some(act) = weights::active_vert(cx, me) else {
        return Ok(OpStatus::Finished);
    };
    let Some(mut w) = Weights::load(cx, me) else {
        return Ok(OpStatus::Finished);
    };
    let (use_mirror, use_topology) = weights::symmetry(cx, me);
    let mirror = if use_mirror {
        let topo = Topology::load(cx, me).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
        Some((
            weights::mirror_map(&w, &topo, use_topology),
            weights::group_names(cx, me),
            weights::locks(cx, me),
        ))
    } else {
        None
    };
    for i in 0..w.len() {
        if !w.select[i] || i == act {
            continue;
        }
        let src = w.dv[act].clone();
        defvert::copy_index(&mut w.dv[i], g, &src, g);
        if let Some((mirr, names, locks)) = &mirror {
            weights::mirror_update_vert(&mut w, mirr, i, None, names, locks);
        }
    }
    if let Some((mirr, names, locks)) = &mirror {
        weights::mirror_update_vert(&mut w, mirr, act, None, names, locks);
    }
    w.store(cx)?;
    Ok(OpStatus::Finished)
}
