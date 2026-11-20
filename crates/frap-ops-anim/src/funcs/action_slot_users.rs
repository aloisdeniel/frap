// SPDX-License-Identifier: GPL-3.0-or-later
//! `slot.users() -> [ID]` (`rna_ActionSlot_users` → `Slot::users`, Blender 5.2.4): the
//! datablocks animated by this slot, through `animation_data.action` + `action_slot` or an NLA
//! strip (`foreach_action_slot_use`). Blender answers from a user cache it rebuilds lazily; frap
//! computes the list each time.

use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{internal, removed, slot_users, target};

#[rna_func(srna = "ActionSlot", func = "users")]
fn action_slot_users(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let (a, loc) = target(h.main(), this)?;
    let Loc::Slot(i) = loc else {
        return Err(internal("users() outside a slot"));
    };
    let main = h.main();
    let handle = main
        .actions
        .get(a)
        .and_then(|x| x.slots.get(i))
        .map(|s| s.handle)
        .ok_or_else(|| removed("ActionSlot"))?;
    Ok(Value::Tuple(
        slot_users(main, a, handle)
            .into_iter()
            .map(|id| Value::Id(Some(id)))
            .collect(),
    ))
}
