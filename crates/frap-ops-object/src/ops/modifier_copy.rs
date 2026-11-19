// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_copy(modifier='', use_selected_objects=False)`, tier T1. Port of
//! `modifier_copy_exec` and `ed::object::modifier_copy` (`editors/object/object_modifier.cc`,
//! Blender 5.2.4, GPL-2.0-or-later) over `frap_modifiers::stack::edit::copy`.
//!
//! Verified on 5.2.4: the copy (settings, show flags, pin) is inserted right after the original,
//! named `Bevel.001`, and becomes active; an unknown name is `CANCELLED`. The copy's ID
//! pointers (a NODES group, a texture) count as users, as Blender's `copy_data` does.

use frap_modifiers::stack::edit;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::modifier_add::shared;

fn copy_poll(c: &Context) -> Result<(), PollFail> {
    shared::edit_modifier_poll(c, "modifier_copy", true)
}

/// `edit_modifier_liboverride_allowed_poll` (frap has no library overrides).
const COPY_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: copy_poll,
};

#[operator(poll = COPY_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ModifierCopy) -> OpResult {
    let objects = shared::edit_objects(&cx.ctx(), p.use_selected_objects.value);
    let mut changed = false;
    for &ob in &objects {
        let Some(i) = cx
            .main()
            .objects
            .get(ob)
            .and_then(|o| shared::find(o, &p.modifier.value))
        else {
            continue;
        };
        edit::copy(&mut cx.object_mut(ob)?.modifiers, i);
        changed = true;
    }
    if !changed {
        shared::report_missing(cx, &objects, &p.modifier.value);
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
