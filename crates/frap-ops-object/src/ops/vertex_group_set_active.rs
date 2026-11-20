// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_set_active(group='')`, tier T0. Port of `set_active_group_exec`
//! and `vgroup_itemf` (`editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! `group` is a dynamic enum of the active object's vertex-group names; the chosen group
//! becomes active (unset: the first). Poll `vertex_group_poll`.

use frap_op::prelude::*;
use frap_op::{EnumItemOwned, Poll, PollReq, operator};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;

fn set_active_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_set_active")
}

/// `vertex_group_poll`.
const SET_ACTIVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: set_active_poll,
};

/// `vgroup_itemf`: the vertex groups of the active object.
fn group_items(c: &Context) -> Vec<EnumItemOwned> {
    poll::active(c)
        .map(|(_, o)| {
            common::names(c, o)
                .iter()
                .map(|n| EnumItemOwned {
                    identifier: n.clone(),
                    name: n.clone(),
                    description: "".into(),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[operator(poll = SET_ACTIVE_POLL, tier = T0, dyn_enums = [("group", group_items)])]
fn exec(cx: &mut OpCtx, p: object::VertexGroupSetActive) -> OpResult {
    let c = cx.ctx();
    let Some((ob, o)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let names = common::names(&c, o);
    let nr = if p.group.is_set {
        match names.iter().position(|n| *n == p.group.value) {
            Some(i) => i,
            None => return Ok(OpStatus::Cancelled),
        }
    } else {
        0
    };
    cx.object_mut(ob)?.vertex_groups_active_index = nr as i32;
    Ok(OpStatus::Finished)
}
