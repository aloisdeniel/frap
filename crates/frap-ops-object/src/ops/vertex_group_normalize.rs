// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_normalize()`, tier T0. Port of `vertex_group_normalize_exec`
//! and `vgroup_normalize` (`editors/object/object_vgroup.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): the active group's weights are divided by their maximum (the selected
//! vertices only in Edit Mode), so the largest becomes 1; nothing changes when every weight is
//! 0. `CANCELLED` without an active group or a weight layer. Poll `vertex_group_poll`.

use frap_bmesh::shape::deform;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn normalize_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_normalize")
}

/// `vertex_group_poll`.
const NORMALIZE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: normalize_poll,
};

#[operator(poll = NORMALIZE_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupNormalize) -> OpResult {
    let Some((ob, me, Some(g))) = common::active_group(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let sel = common::use_vert_sel(cx, ob);
    let Some(dverts) = common::read_dverts(cx, me, sel) else {
        return Ok(OpStatus::Cancelled);
    };
    let g = g as u32;
    let weight_max = dverts
        .iter()
        .filter_map(|dv| deform::find(dv, g).map(|i| dv.weights[i].1))
        .fold(0.0f32, f32::max);
    if weight_max > 0.0 {
        common::for_each_dvert(cx, me, sel, |dv| {
            if let Some(i) = deform::find(dv, g) {
                let w = dv.weights[i].1 / weight_max;
                // In case of division errors with very low weights.
                dv.weights[i].1 = w.clamp(0.0, 1.0);
            }
        })?;
    }
    Ok(OpStatus::Finished)
}
