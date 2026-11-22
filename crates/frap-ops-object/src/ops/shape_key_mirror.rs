// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_mirror(use_topology=False)`, tier T1 (`tests/golden/cases/object/
//! shape_keys_m8/copy_lock_mirror.py`, `mirror_topology.py`). Port of `shape_key_mirror_exec`,
//! `object_shape_key_mirror` and `shape_key_mirror` (`editors/object/object_shapekey.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): the active block is mirrored along local X. The vertex
//! pairs come from the mesh positions (`mesh_get_x_mirror_vert`: within `0.00002` of the
//! mirrored position, or by topology with `use_topology`); a pair swaps its coordinates, a
//! vertex mirrored to itself flips X. Reports `N vertices mirrored` (INFO) or `N vertices
//! mirrored, M failed` (WARNING). A locked active block raises `RuntimeError: Error: The active
//! shape key of <object> is locked`.
//!
//! Poll `shape_key_mode_exists_poll`: an active block, not in Edit Mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::shape_key_add::common;
use crate::support::weights::{self, Topology, mirror};

fn mirror_poll(c: &Context) -> Result<(), PollFail> {
    common::mode_exists_poll(c)
}

/// `shape_key_mode_exists_poll`.
const MIRROR_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ModeObject,
    ],
    check: mirror_poll,
};

#[operator(poll = MIRROR_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ShapeKeyMirror) -> OpResult {
    let Some((ob, o)) = poll::active(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let name = o.header.name.clone();
    let Some((k, a)) = weights::active_block(&cx.ctx(), ob) else {
        return Ok(OpStatus::Cancelled);
    };
    if cx
        .main()
        .shape_keys
        .get(k)
        .is_some_and(|key| key.key_blocks[a].lock_shape)
    {
        return Err(OpError::Runtime(format!(
            "The active shape key of {name} is locked"
        )));
    }
    let Some((_, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let topo = Topology::load(cx, me).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
    let mirr = mirror::object_mirror_map(&topo.positions, &topo.edges, p.use_topology.value);
    let (main, log) = cx.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Ok(OpStatus::Cancelled);
    };
    let (totmirr, totfail) = weights::mirror_block(&mut key.key_blocks[a].data, &mirr);
    weights::report_mirror(cx, totmirr, totfail);
    Ok(OpStatus::Finished)
}
