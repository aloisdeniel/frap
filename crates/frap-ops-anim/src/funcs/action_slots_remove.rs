// SPDX-License-Identifier: GPL-3.0-or-later
//! `action.slots.remove(action_slot)` (`rna_Action_slots_remove` → `Action::slot_remove`,
//! Blender 5.2.4): removes the slot and its channelbags from every keyframe strip; its users
//! keep their (now dangling) handle, as in Blender, where handles are never reused. A slot of
//! another action raises `RuntimeError: Error: This slot does not belong to this Action`.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_ptr, report_error, slot_arg, with_action};

#[rna_func(srna = "ActionSlots", func = "remove")]
fn action_slots_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    let slot = arg_ptr(args, "action_slot").and_then(|p| slot_arg(h.main(), p));
    let Some((_, handle)) = slot.filter(|(sa, _)| *sa == a) else {
        return Err(report_error("This slot does not belong to this Action"));
    };
    with_action(h, this, |act, _| {
        act.slot_remove(handle);
        Ok(Value::None)
    })
}
