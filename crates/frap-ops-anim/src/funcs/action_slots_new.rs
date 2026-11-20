// SPDX-License-Identifier: GPL-3.0-or-later
//! `action.slots.new(id_type, name)` (`rna_Action_slots_new` → `slot_add_for_id_type` +
//! `slot_display_name_set`, Blender 5.2.4): a slot identified as the type's two-letter code plus
//! `name` (`OBCube`; `XX…` for `UNSPECIFIED`), made unique with `.001`, …; the handle continues
//! from the action's last one (the first slot of an action is 929201142). An empty name raises
//! `RuntimeError: Error: Invalid slot name '': name must not be empty.` (verified on 5.2.4).

use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, arg_str, loc_ptr, report_error, with_action};

#[rna_func(srna = "ActionSlots", func = "new")]
fn action_slots_new(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let id_type = arg_str(args, "id_type").to_owned();
    let name = arg_str(args, "name").to_owned();
    if name.is_empty() {
        return Err(report_error(format!(
            "Invalid slot name '{name}': name must not be empty."
        )));
    }
    let a = action_of(this)?;
    with_action(h, this, |act, _| {
        let handle = act.slot_add_for_id_type(&id_type, &name);
        let i = act.slot_index(handle).unwrap_or(0);
        Ok(loc_ptr(a, act, Loc::Slot(i)))
    })
}
