// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_move(direction='UP')`, tier T1 (`tests/golden/cases/object/
//! vgroups_m8/copy_lock_move_sort.py`). Port of `vgroup_move_exec`, `BLI_listbase_link_move`
//! and `vgroup_do_remap` (`editors/object/object_vgroup.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): the active group moves one place up or down the list; weights and the
//! active index follow it. At the end of the list nothing moves (still `FINISHED`); without an
//! active group the operator is `CANCELLED`.
//!
//! Poll `vertex_group_poll`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights;

fn move_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_move")
}

/// `vertex_group_poll`.
const MOVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: move_poll,
};

#[operator(poll = MOVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::VertexGroupMove) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let act = {
        let c = cx.ctx();
        c.main
            .objects
            .get(ob)
            .and_then(|o| weights::active_group(&c, o))
    };
    let Some(act) = act else {
        return Ok(OpStatus::Cancelled);
    };
    let mut names = weights::group_names(cx, me);
    let to = match p.direction.value.identifier() {
        "DOWN" => act + 1,
        _ => match act.checked_sub(1) {
            Some(t) => t,
            None => return Ok(OpStatus::Finished),
        },
    };
    if to >= names.len() {
        return Ok(OpStatus::Finished);
    }
    names.swap(act, to);
    weights::reorder_groups(cx, ob, me, names)?;
    Ok(OpStatus::Finished)
}
