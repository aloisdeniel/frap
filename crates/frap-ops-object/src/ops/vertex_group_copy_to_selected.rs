// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_copy_to_selected()`, tier T1 (`tests/golden/cases/object/
//! vgroups_m8/copy_to_selected.py`). Port of `vertex_group_copy_to_selected_exec` and
//! `vgroup_array_copy` (`editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later):
//! every other selected editable mesh object takes the active object's vertex groups (names,
//! locks, active index) and weights, when both meshes have weights for the same number of
//! vertices. A target sharing the active object's mesh counts as done and changes nothing; a
//! target without weights gets a weight layer first (and loses its groups when the copy then
//! fails); both meshes without weights count as done without copying anything.
//!
//! Nothing done, or a failure: `RuntimeError: Error: Copy vertex groups to selected: N done,
//! M failed (object data must support vertex groups and have matching indices)`. Blender keeps
//! the objects it did copy to; frap statements are atomic, so a failing call changes nothing
//! (deviations D-148).
//!
//! Poll `vertex_group_poll`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Weights};

fn copy_to_selected_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_copy_to_selected")
}

/// `vertex_group_poll`.
const COPY_TO_SELECTED_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: copy_to_selected_poll,
};

/// `vgroup_array_copy(ob, ob_from)`: true when the groups were copied (or nothing had to be).
fn array_copy(
    cx: &mut OpCtx,
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
    from: IdRef<ObjectId>,
    me_from: IdRef<MeshId>,
) -> Result<bool, OpError> {
    if me == me_from {
        return Ok(true);
    }
    let src = Weights::load(cx, me_from);
    let mut dst = Weights::load(cx, me);
    let mut new_vgroup = false;
    if dst.is_none() && src.is_some() {
        dst = Weights::load_or_create(cx, me);
        new_vgroup = true;
    }
    let tot = dst.as_ref().map_or(0, Weights::len);
    let tot_from = src.as_ref().map_or(0, Weights::len);
    let (Some(src), Some(mut dst)) = (src, dst) else {
        return Ok(tot == tot_from);
    };
    if tot == 0 || tot != tot_from {
        if new_vgroup {
            weights::remove_all_groups(cx, ob, me)?;
        }
        return Ok(tot == tot_from);
    }
    let (names, locks, active) = {
        let c = cx.ctx();
        let m = c.main.meshes.get(me_from);
        (
            m.map(|m| m.vertex_group_names.clone()).unwrap_or_default(),
            m.map(|m| m.vertex_group_locks.clone()).unwrap_or_default(),
            c.main
                .objects
                .get(from)
                .map_or(-1, |o| o.vertex_groups_active_index),
        )
    };
    let m = cx.mesh_mut(me)?;
    m.vertex_group_names = names;
    m.vertex_group_locks = locks;
    cx.object_mut(ob)?.vertex_groups_active_index = active;
    dst.dv.clone_from(&src.dv);
    dst.store(cx)?;
    Ok(true)
}

#[operator(poll = COPY_TO_SELECTED_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::VertexGroupCopyToSelected) -> OpResult {
    let Some((obact, me_act)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let targets: Vec<(IdRef<ObjectId>, IdRef<MeshId>)> = {
        let c = cx.ctx();
        c.selected_editable_objects()
            .into_iter()
            .filter(|&r| r != obact)
            .filter_map(|r| {
                let o = c.main.objects.get(r)?;
                common::supports_vgroup(o).then_some(())?;
                Some((r, weights::mesh_of(&c, o)?))
            })
            .collect()
    };
    let (mut changed, mut fail) = (0, 0);
    for (ob, me) in targets {
        if array_copy(cx, ob, me, obact, me_act)? {
            changed += 1;
        } else {
            fail += 1;
        }
    }
    if (changed == 0 && fail == 0) || fail > 0 {
        return Err(OpError::Runtime(format!(
            "Copy vertex groups to selected: {changed} done, {fail} failed (object data must \
             support vertex groups and have matching indices)"
        )));
    }
    Ok(OpStatus::Finished)
}
