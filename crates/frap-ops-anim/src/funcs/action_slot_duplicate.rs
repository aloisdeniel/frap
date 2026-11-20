// SPDX-License-Identifier: GPL-3.0-or-later
//! `slot.duplicate() -> ActionSlot` (`rna_ActionSlot_duplicate` → `animrig::duplicate_slot`,
//! Blender 5.2.4): a new slot with the next handle, the same type and flags and a unique
//! identifier (`OBCube.001`), whose channelbags are copies of the source's in every keyframe
//! strip. Nothing uses it yet.

use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{action_of, internal, loc_ptr, removed, with_action};

#[rna_func(srna = "ActionSlot", func = "duplicate")]
fn action_slot_duplicate(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let a = action_of(this)?;
    with_action(h, this, |act, loc| {
        let Loc::Slot(i) = loc else {
            return Err(internal("duplicate() outside a slot"));
        };
        let handle = act.slots[i].handle;
        let new = act
            .slot_duplicate(handle)
            .ok_or_else(|| removed("ActionSlot"))?;
        let j = act.slot_index(new).ok_or_else(|| removed("ActionSlot"))?;
        Ok(loc_ptr(a, act, Loc::Slot(j)))
    })
}
