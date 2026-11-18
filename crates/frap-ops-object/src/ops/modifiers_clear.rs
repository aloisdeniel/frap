// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifiers_clear()`, tier T1. Port of `modifiers_clear_exec` and
//! `modifiers_clear_poll` (`editors/object/object_modifier.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): removes every modifier of every selected editable object.
//!
//! Verified on 5.2.4: the poll needs an active local object (`Context missing active object`)
//! that supports modifiers (an active camera: `failed, context is incorrect`); the selected
//! objects are cleared whatever their type.

use frap_modifiers::registry;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;
use smallvec::smallvec;

use super::modifier_add::shared;

fn clear_poll(c: &Context) -> Result<(), PollFail> {
    shared::active_editable(c, "modifiers_clear")?;
    match poll::active(c) {
        Some((_, ob)) if registry::object_supports_modifiers(ob.object_type()) => Ok(()),
        _ => Err(PollFail {
            message: None,
            unmet: smallvec![PollReq::ActiveType("MESH")],
            hint: None,
        }),
    }
}

/// `modifiers_clear_poll`.
const CLEAR_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: clear_poll,
};

#[operator(poll = CLEAR_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::ModifiersClear) -> OpResult {
    let objects = cx.ctx().selected_editable_objects();
    for ob in objects {
        if cx
            .main()
            .objects
            .get(ob)
            .is_some_and(|o| !o.modifiers.is_empty())
        {
            cx.object_mut(ob)?.modifiers.clear();
        }
    }
    Ok(OpStatus::Finished)
}
