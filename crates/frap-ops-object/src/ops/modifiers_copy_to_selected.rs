// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifiers_copy_to_selected()`, tier T0
//! (`tests/golden/cases/object/data_m8/modifiers_copy.py`). Port of
//! `object_modifiers_copy_exec` and `modifiers_copy_to_selected_poll`
//! (`editors/object/object_modifier.cc`, Blender 5.2.4, GPL-2.0-or-later) over the copy of
//! `object.modifier_copy_to_selected` ([`super::modifier_copy_to_selected::copy_to_object`]).
//!
//! Verified on 5.2.4:
//! - poll `ED_operator_object_active_editable` (`Context missing active object`, …), an
//!   active object of a type with modifiers (`failed, context is incorrect`) and at least one
//!   modifier (`Active object has no modifiers`);
//! - every modifier of the active object, in stack order, is copied to every other selected
//!   object (linked ones included), each with the WARNINGs of the single copy; the copy of the
//!   source's active modifier becomes the target's active one;
//! - always `FINISHED`; a Hook or Collision modifier in the stack is refused with the ERROR
//!   `Copying modifier '…' to object '…' failed` per target, which `bpy.ops` raises as a
//!   `RuntimeError` while keeping the other copies. frap raises the same error and rolls the
//!   statement back (D-158).

use frap_modifiers::stack::edit;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::modifier_add::shared;
use super::modifier_copy_to_selected::{
    copy_to_object, report_refused, supports_modifiers, targets,
};

const OP: &str = "modifiers_copy_to_selected";

/// `modifiers_copy_to_selected_poll`.
fn copy_poll(c: &Context) -> Result<(), PollFail> {
    shared::active_editable(c, OP)?;
    let (_, ob) = poll::active(c).expect("invariant: checked by active_editable");
    if !supports_modifiers(ob.object_type()) {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make an object with modifiers (a mesh) active",
        ));
    }
    if ob.modifiers.is_empty() {
        let mut f = poll::fail(
            PollReq::ActiveObject,
            "add a modifier to the active object first (bpy.ops.object.modifier_add(type=…))",
        );
        f.message = Some(shared::poll_message(OP, "Active object has no modifiers"));
        return Err(f);
    }
    Ok(())
}

/// `modifiers_copy_to_selected_poll`.
const COPY_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveEditable],
    check: copy_poll,
};

#[operator(
    poll = COPY_POLL,
    tier = T0,
    notes = [
        "persistent_uid is one more than the largest in use (D-47)",
        "a Hook or Collision modifier in the stack raises RuntimeError and the whole statement is rolled back (D-158)",
    ]
)]
fn exec(cx: &mut OpCtx, _p: object::ModifiersCopyToSelected) -> OpResult {
    let Some(act) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let mods = cx
        .main()
        .objects
        .get(act)
        .map(|o| o.modifiers.clone())
        .unwrap_or_default();
    let targets = targets(&cx.ctx(), act, false);
    let mut errors: Vec<String> = Vec::new();
    for ob in targets {
        for md in &mods {
            let dst = cx.object_mut(ob)?;
            match copy_to_object(md, dst)? {
                Ok(i) => {
                    if md.is_active {
                        edit::set_active(&mut dst.modifiers, Some(i));
                    }
                }
                Err(r) => report_refused(cx, r, &mut errors),
            }
        }
    }
    if !errors.is_empty() {
        return Err(OpError::Runtime(errors.join("\nError: ")));
    }
    Ok(OpStatus::Finished)
}
