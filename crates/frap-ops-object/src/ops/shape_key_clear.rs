// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_clear()`, tier T0. Port of `shape_key_clear_exec`
//! (`editors/object/object_shapekey.cc`, Blender 5.2.4, GPL-2.0-or-later): every block's value
//! goes back to 0, or to the nearest value its slider range allows. `CANCELLED` without blocks.
//! Poll `shape_key_poll` (also in Edit Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::shape_key_add::common;

fn clear_poll(c: &Context) -> Result<(), PollFail> {
    common::shape_key_poll(c)
}

/// `shape_key_poll`.
const CLEAR_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: clear_poll,
};

#[operator(poll = CLEAR_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::ShapeKeyClear) -> OpResult {
    let c = cx.ctx();
    let Some((ob, _)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(k) = common::key_of(&c, ob) else {
        return Ok(OpStatus::Cancelled);
    };
    if c.main
        .shape_keys
        .get(k)
        .is_none_or(|key| key.key_blocks.is_empty())
    {
        return Ok(OpStatus::Cancelled);
    }
    let (main, log) = cx.main_mut();
    if let Some(key) = main.shape_keys.get_mut(k, log) {
        for kb in &mut key.key_blocks {
            // `clamp_f(0.0f, slidermin, slidermax)`.
            kb.value = 0.0f32.max(kb.slider_min).min(kb.slider_max);
        }
    }
    Ok(OpStatus::Finished)
}
