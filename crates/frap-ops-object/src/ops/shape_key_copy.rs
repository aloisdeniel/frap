// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_copy()`, tier T1 (`tests/golden/cases/object/shape_keys_m8/
//! copy_lock_mirror.py`). Port of `shape_key_copy_exec` and `BKE_keyblock_duplicate`
//! (`editors/object/object_shapekey.cc`, `blenkernel/intern/key.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): the active block is duplicated right after itself under the same name
//! made unique (`Key 1.001`), with its data, settings (value, range, interpolation, relative
//! key, vertex group, position) and flags (mute, lock, selection), and becomes active. As in
//! Blender, the `relative_key` indices of the other blocks are not shifted by the insertion.
//!
//! Poll `shape_key_mode_exists_poll`: an active block, not in Edit Mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::shape_key_add::common;
use crate::support::weights;

fn copy_poll(c: &Context) -> Result<(), PollFail> {
    common::mode_exists_poll(c)
}

/// `shape_key_mode_exists_poll`.
const COPY_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ModeObject,
    ],
    check: copy_poll,
};

#[operator(poll = COPY_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::ShapeKeyCopy) -> OpResult {
    let Some((ob, _)) = poll::active(&cx.ctx()) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some((k, src)) = weights::active_block(&cx.ctx(), ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Ok(OpStatus::Cancelled);
    };
    let name = key.key_blocks[src].name.clone();
    let i = key.add_block(Some(&name));
    let mut kb = key.key_blocks.remove(i);
    let from = &key.key_blocks[src];
    kb.data = from.data.clone();
    kb.copy_settings_from(from);
    kb.mute = from.mute;
    kb.lock_shape = from.lock_shape;
    kb.select = from.select;
    key.key_blocks.insert(src + 1, kb);
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.active_shape_key_index = (src + 1) as i32;
    }
    Ok(OpStatus::Finished)
}
