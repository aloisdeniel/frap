// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_remove(modifier='', report=False, use_selected_objects=False)`,
//! tier T1. Port of `modifier_remove_exec` (`editors/object/object_modifier.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `frap_modifiers::stack::edit::remove`.
//!
//! Verified on 5.2.4: removes the modifier named `modifier` (hidden property; scripts must pass
//! it) from the active object (every selected editable one with `use_selected_objects`), in
//! Object and Edit Mode; an unknown or empty name is `CANCELLED` without a message;
//! `report=True` adds the INFO `Removed modifier: <name>`; without an active object the poll
//! fails (`failed, context is incorrect`).

use frap_modifiers::stack::edit;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::modifier_add::shared;

fn remove_poll(c: &Context) -> Result<(), PollFail> {
    shared::edit_modifier_poll(c, "modifier_remove", true)
}

/// `edit_modifier_poll`.
const REMOVE_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: remove_poll,
};

#[operator(poll = REMOVE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ModifierRemove) -> OpResult {
    let name = p.modifier.value.clone();
    let objects = shared::edit_objects(&cx.ctx(), p.use_selected_objects.value);
    let mut changed = false;
    for ob in objects {
        let Some(i) = cx
            .main()
            .objects
            .get(ob)
            .and_then(|o| shared::find(o, &name))
        else {
            continue;
        };
        edit::remove(&mut cx.object_mut(ob)?.modifiers, i);
        changed = true;
    }
    if !changed {
        return Ok(OpStatus::Cancelled);
    }
    if p.report.value {
        cx.report(ReportType::Info, format!("Removed modifier: {name}"));
    }
    Ok(OpStatus::Finished)
}
