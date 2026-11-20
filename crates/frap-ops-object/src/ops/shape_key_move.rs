// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_move(type='TOP')`, tier T0. Port of `shape_key_move_exec` and
//! `BKE_keyblock_move` (`editors/object/object_shapekey.cc`, `blenkernel/intern/key.cc`,
//! Blender 5.2.4, GPL-2.0-or-later): the selected blocks and the active one move up (`TOP`,
//! `UP`, never above the basis) or down (`DOWN`, `BOTTOM`), relative keys and the active index
//! following them. `CANCELLED` when nothing moves. Poll `shape_key_move_poll` (two blocks, not
//! in Edit Mode).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;
use frap_spec_ops_object::object::shape_key_move::Type;

use super::shape_key_add::common;

fn move_poll(c: &Context) -> Result<(), PollFail> {
    common::move_poll(c)
}

/// `shape_key_move_poll`.
const MOVE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ModeObject,
    ],
    check: move_poll,
};

#[operator(poll = MOVE_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::ShapeKeyMove) -> OpResult {
    let c = cx.ctx();
    let Some((ob, o)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(k) = common::key_of(&c, ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let mut active = i64::from(o.active_shape_key_index);
    let ty = p.r#type.value;
    let (main, log) = cx.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Ok(OpStatus::Cancelled);
    };
    let totkey = key.key_blocks.len() as i64;
    let mut changed = false;
    match ty {
        Type::Top | Type::Up => {
            let mut top_index = 1i64;
            for index in 1..totkey {
                if !common::is_selected(key, index as usize, active + 1) {
                    continue;
                }
                let new_index = if ty == Type::Top {
                    top_index
                } else {
                    (index - 1).max(top_index)
                };
                top_index += 1;
                changed |= key.move_block(Some(index as usize), new_index, &mut active);
            }
        }
        Type::Down | Type::Bottom => {
            let mut bottom_index = totkey - 1;
            for index in (1..totkey).rev() {
                if !common::is_selected(key, index as usize, active + 1) {
                    continue;
                }
                let new_index = if ty == Type::Bottom {
                    bottom_index
                } else {
                    (index + 1).min(bottom_index)
                };
                bottom_index -= 1;
                changed |= key.move_block(Some(index as usize), new_index, &mut active);
            }
        }
    }
    if !changed {
        return Ok(OpStatus::Cancelled);
    }
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.active_shape_key_index = active as i32;
    }
    Ok(OpStatus::Finished)
}
