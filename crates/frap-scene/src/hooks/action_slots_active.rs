// SPDX-License-Identifier: GPL-3.0-or-later
//! `action.slots.active` (WP 6.1; `rna_ActionSlots_active_get/set`, Blender 5.2.4): the slot
//! flagged active, `None` without one. Assigning a slot makes it the only active one (by its
//! handle); assigning `None` clears the flag on every slot.

use frap_anim::rna::action::{Loc, locate};
use frap_rna::{ActionId, Field, IdRef, PropDef, RnaError, RnaPtr, Root, Seg, Value, rna_hook};

use super::id_animation_data::removed;
use crate::id::paths::internal;
use crate::{Host, HostRead};

/// `ActionSlots` is the API struct of `Action.slots` (no binding of its own).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ActionSlots",
    prop: "active",
    coll: false,
};

fn action_of(this: &RnaPtr) -> Result<IdRef<ActionId>, RnaError> {
    match this.root {
        Root::Id(a) => a.typed().ok_or_else(|| internal("not an action")),
        _ => Err(internal("not an action")),
    }
}

#[rna_hook(srna = "ActionSlots", prop = "active", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let a = action_of(this)?;
        let act = h.main().actions.get(a).ok_or_else(removed)?;
        Ok(Field::Value(match act.slots.iter().find(|s| s.active) {
            Some(s) => Value::Ptr(Box::new(
                RnaPtr::id(a.any(), &frap_spec_rna::ACTION)
                    .child(Seg::Prop("slots"), &frap_spec_rna::ACTION_SLOT)
                    .child(Seg::Key(s.identifier.clone()), &frap_spec_rna::ACTION_SLOT),
            )),
            None => Value::None,
        }))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let a = action_of(this)?;
        let handle = match &v {
            Value::Ptr(p) => {
                let Root::Id(sa) = p.root else {
                    return Err(internal("not a slot"));
                };
                let sa: IdRef<ActionId> = sa.typed().ok_or_else(|| internal("not a slot"))?;
                let act = h.main().actions.get(sa).ok_or_else(removed)?;
                match locate(act, &p.path) {
                    Some((Loc::Slot(i), None)) => act.slots[i].handle,
                    _ => return Err(removed()),
                }
            }
            _ => 0,
        };
        let (main, log) = h.main_mut();
        if let Some(act) = main.actions.get_mut(a, log) {
            act.slot_active_set(handle);
        }
        Ok(())
    }
}
