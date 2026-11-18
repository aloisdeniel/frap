// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_set_active(modifier='')`, tier T1. Port of
//! `modifier_set_active_exec` (`editors/object/object_modifier.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): `BKE_object_modifier_set_active` with the named modifier of the active
//! object.
//!
//! Verified on 5.2.4: an empty or unknown name clears the active modifier and still finishes
//! (also on a camera); the poll only needs an active object (`ED_operator_object_active_only`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;
use smallvec::smallvec;

fn active_only(c: &Context) -> Result<(), PollFail> {
    match c.active_object() {
        Some(_) => Ok(()),
        None => Err(PollFail {
            message: None,
            unmet: smallvec![PollReq::ActiveObject],
            hint: None,
        }),
    }
}

/// `ED_operator_object_active_only`.
const ACTIVE_ONLY: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: active_only,
};

#[operator(poll = ACTIVE_ONLY, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ModifierSetActive) -> OpResult {
    let Some(ob) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(o) = cx.main().objects.get(ob) else {
        return Ok(OpStatus::Cancelled);
    };
    let target = o.modifiers.iter().position(|m| m.name == p.modifier.value);
    let changed = o
        .modifiers
        .iter()
        .enumerate()
        .any(|(j, m)| m.is_active != (Some(j) == target));
    if changed {
        frap_modifiers::stack::edit::set_active(&mut cx.object_mut(ob)?.modifiers, target);
    }
    Ok(OpStatus::Finished)
}
