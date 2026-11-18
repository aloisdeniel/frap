// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_move_up(modifier='')`, tier T1. Port of `modifier_move_up_exec`
//! and `ed::object::modifier_move_up` (`editors/object/object_modifier.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `frap_modifiers::stack::edit::move_up`.
//!
//! Verified on 5.2.4: moves the named modifier of the active object one step up; the first one
//! is `CANCELLED` with the WARNING `Cannot move modifier beyond the start of the list`; an
//! unknown name is `CANCELLED` silently, as is a pinned modifier below an unpinned one.

use frap_modifiers::stack::edit;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::modifier_add::shared;

fn move_poll(c: &Context) -> Result<(), PollFail> {
    shared::edit_modifier_poll(c, "modifier_move_up", true)
}

/// `edit_modifier_poll`.
const MOVE_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: move_poll,
};

#[operator(poll = MOVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ModifierMoveUp) -> OpResult {
    let Some(ob) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(mut mods) = cx.main().objects.get(ob).map(|o| o.modifiers.clone()) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(i) = mods.iter().position(|m| m.name == p.modifier.value) else {
        return Ok(OpStatus::Cancelled);
    };
    match edit::move_up(&mut mods, i) {
        Ok(()) => {
            cx.object_mut(ob)?.modifiers = mods;
            Ok(OpStatus::Finished)
        }
        Err(msg) => {
            if let Some(msg) = msg {
                cx.report(ReportType::Warning, msg);
            }
            Ok(OpStatus::Cancelled)
        }
    }
}
