// SPDX-License-Identifier: GPL-3.0-or-later
//! `ActionSlot.identifier` and `ActionSlot.name_display` writes (WP 6.1;
//! `rna_ActionSlot_identifier_set`, `rna_ActionSlot_name_display_set` and their update
//! `rna_ActionSlot_identifier_update` → `Action::slot_identifier_propagate`, Blender 5.2.4
//! makesrna/intern/rna_action.cc and animrig/intern/action.cc):
//! - an identifier shorter than 3 characters, or an empty display name, is refused with a
//!   global report (no exception) and changes nothing;
//! - the identifier's two-letter prefix always follows `target_id_type` (another prefix is
//!   replaced, with a printed warning);
//! - the name is made unique among the action's slots (`.001`, …);
//! - every `AnimData` using the slot gets the new identifier as `last_slot_identifier`.

use frap_anim::Action;
use frap_anim::rna::action::{Loc, locate};
use frap_rna::{ActionId, EmbeddedSlot, IdRef, PropDef, RnaError, RnaPtr, Root, Value, rna_hook};

use super::id_animation_data::{adt_of, adt_of_mut, removed};
use crate::id::paths::internal;
use crate::{Host, Main};

/// The action and slot index `this` points to.
fn slot_of(main: &Main, this: &RnaPtr) -> Result<(IdRef<ActionId>, usize), RnaError> {
    let Root::Id(a) = this.root else {
        return Err(internal("an action slot outside an action"));
    };
    let a: IdRef<ActionId> = a.typed().ok_or_else(|| internal("not an action"))?;
    let act = main.actions.get(a).ok_or_else(removed)?;
    match locate(act, &this.path) {
        Some((Loc::Slot(i), None)) => Ok((a, i)),
        _ => Err(removed()),
    }
}

/// `Action::slot_identifier_propagate`: every `AnimData` animated by the slot remembers its new
/// identifier.
pub fn propagate(h: &mut dyn Host, a: IdRef<ActionId>, handle: i32) {
    let main = h.main();
    let Some(identifier) = main
        .actions
        .get(a)
        .and_then(|x| x.slot(handle))
        .map(|s| s.identifier.clone())
    else {
        return;
    };
    let embedded = |owner: frap_rna::IdAny| Root::Embedded {
        owner,
        slot: EmbeddedSlot::NodeTree,
    };
    let mut roots: Vec<Root> = main.all_ids().into_iter().map(Root::Id).collect();
    roots.extend(main.materials.iter().map(|(r, _)| embedded(r.any())));
    roots.extend(main.worlds.iter().map(|(r, _)| embedded(r.any())));
    let users: Vec<Root> = roots
        .into_iter()
        .filter(|r| {
            adt_of(main, *r)
                .and_then(Option::as_ref)
                .is_some_and(|d| d.action == Some(a) && d.action_slot_handle == handle)
        })
        .collect();
    let (main, log) = h.main_mut();
    for r in users {
        if let Some(Some(d)) = adt_of_mut(main, log, r) {
            d.last_slot_identifier = identifier.clone();
        }
    }
}

fn edit(h: &mut dyn Host, this: &RnaPtr, f: impl FnOnce(&mut Action, i32)) -> Result<(), RnaError> {
    let (a, i) = slot_of(h.main(), this)?;
    let (main, log) = h.main_mut();
    let act = main.actions.get_mut(a, log).ok_or_else(removed)?;
    let handle = act.slots[i].handle;
    f(act, handle);
    propagate(h, a, handle);
    Ok(())
}

#[rna_hook(srna = "ActionSlot", prop = "identifier", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let new = v.as_str().unwrap_or_default().to_owned();
    if new.chars().count() < 3 {
        // `WM_global_report(RPT_ERROR, "Action slot identifiers should be at least three
        // characters")`: reported, not raised.
        return Ok(());
    }
    edit(h, this, |act, handle| {
        let prefix = act
            .slot(handle)
            .map(|s| frap_anim::action::id_type_prefix(&s.target_id_type))
            .unwrap_or("XX");
        let rest: String = new.chars().skip(2).collect();
        act.slot_identifier_define(handle, &format!("{prefix}{rest}"));
    })
}

#[rna_hook(srna = "ActionSlot", prop = "name_display", krate = crate)]
mod name_display {
    use super::*;

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let name = v.as_str().unwrap_or_default().to_owned();
        if name.is_empty() {
            // "Action slot display names cannot be empty": a global report.
            return Ok(());
        }
        edit(h, this, |act, handle| {
            act.slot_display_name_define(handle, &name)
        })
    }
}

/// The binding answers `NeedsHook` for both properties.
macro_rules! decls {
    ($($prop:literal),*) => {$(
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        const _: () = {
            #[allow(unsafe_code)]
            #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
            #[linkme(crate = frap_rna::linkme)]
            static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
                srna: "ActionSlot",
                prop: $prop,
                coll: false,
            };
        };
    )*};
}

decls!("identifier", "name_display");
