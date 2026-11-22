// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_lock(action='LOCK')`, tier T1 (`tests/golden/cases/object/
//! shape_keys_m8/copy_lock_mirror.py`). Port of `shape_key_lock_exec` and
//! `shape_key_exists_poll` (`editors/object/object_shapekey.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `LOCK` sets `ShapeKey.lock_shape` on every block of the active object's
//! key, `UNLOCK` clears it. Works in Edit Mode too.
//!
//! Poll `shape_key_exists_poll`: an editable active object with editable data and an active
//! block.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;
use smallvec::smallvec;

use super::shape_key_add::common;
use crate::support::weights;

/// `shape_key_exists_poll`: [`common::shape_key_poll`] and an active block.
pub(crate) fn exists_poll(c: &Context) -> Result<(), PollFail> {
    common::shape_key_poll(c)?;
    let (ob, _) = poll::active(c).expect("invariant: checked by shape_key_poll");
    if weights::active_block(c, ob).is_none() {
        return Err(PollFail {
            message: None,
            unmet: smallvec![],
            hint: Some(
                "the active object has no shape key: bpy.ops.object.shape_key_add(from_mix=False)"
                    .into(),
            ),
        });
    }
    Ok(())
}

/// `shape_key_exists_poll`.
const LOCK_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: exists_poll,
};

#[operator(poll = LOCK_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ShapeKeyLock) -> OpResult {
    let Some((ob, _)) = poll::active(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(k) = common::key_of(&cx.ctx(), ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let lock = p.action.value.identifier() == "LOCK";
    let (main, log) = cx.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Ok(OpStatus::Cancelled);
    };
    if key.key_blocks.is_empty() {
        return Ok(OpStatus::Cancelled);
    }
    for kb in &mut key.key_blocks {
        kb.lock_shape = lock;
    }
    Ok(OpStatus::Finished)
}
