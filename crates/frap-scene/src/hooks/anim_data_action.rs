// SPDX-License-Identifier: GPL-3.0-or-later
//! Action and slot assignment (WP 6.1): `AnimData.action`, `action_slot`,
//! `action_slot_handle`, `action_suitable_slots` and the same four on `NlaStrip`.
//!
//! Ported from blender/source/blender/animrig/intern/action.cc (`generic_assign_action`,
//! `generic_assign_action_slot`, `generic_assign_action_slot_handle`) and animrig/intern/nla.cc
//! (`nla::assign_action`), with the RNA wrappers of makesrna/intern/rna_animation.cc and
//! rna_nla.cc (Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4:
//! - assigning an action first un-assigns the previous one, remembering its slot's identifier
//!   in `last_slot_identifier`; the new action's slot is auto-chosen from that identifier, then
//!   the ID's own name (`OBCube`), then a lone untyped slot; none leaves the handle 0;
//! - the chosen slot is bound to the ID's type (an `XX` slot becomes `OB…`);
//! - `action_slot = slot` raises `RuntimeError: Error: This slot (OBCube) does not belong to the
//!   assigned Action` or `… is not suitable for this data-block type (OB)`, and without an action
//!   `Error: Cannot set slot without an assigned Action.`;
//! - `action_slot_handle = h` reports the same problems without raising (an `int` setter has
//!   no report list); a handle not in the action un-assigns the slot.
//! - an NLA strip whose auto-assignment found nothing takes the action's only slot when it is
//!   suitable.

use frap_anim::Action;
use frap_anim::rna::action::{Loc, Node};
use frap_anim::rna::anim_data::{AdtLoc, locate};
use frap_rna::{
    ActionId, CollRef, Field, IdRef, Item, PropDef, PyExc, RnaColl, RnaError, RnaErrorKind, RnaPtr,
    Root, Seg, StructDef, StructRef, Value, rna_hook,
};
use smol_str::SmolStr;

use super::id_animation_data::{adt_of, adt_of_mut, animated_id, below_adt, removed};
use crate::{Host, HostRead, Main};

/// `BKE_report(RPT_ERROR)` reaching Python.
fn report(msg: impl std::fmt::Display) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        format!("Error: {msg}\n"),
    )
}

/// An action use: `(action, slot handle, last slot identifier)` of an `AnimData` or a strip.
#[derive(Clone, Debug, PartialEq)]
struct Use {
    action: Option<IdRef<ActionId>>,
    handle: i32,
    last: SmolStr,
}

/// Where the use lives.
#[derive(Copy, Clone)]
enum Owner {
    Adt,
    Strip(usize, usize),
}

fn owner_of(main: &Main, this: &RnaPtr) -> Result<Owner, RnaError> {
    let adt = adt_of(main, this.root)
        .and_then(Option::as_ref)
        .ok_or_else(removed)?;
    match locate(adt, below_adt(this)?) {
        Some((AdtLoc::Root, None)) => Ok(Owner::Adt),
        Some((AdtLoc::Strip(t, s), None)) => Ok(Owner::Strip(t, s)),
        _ => Err(removed()),
    }
}

fn read_use(main: &Main, root: Root, owner: Owner) -> Result<Use, RnaError> {
    let adt = adt_of(main, root)
        .and_then(Option::as_ref)
        .ok_or_else(removed)?;
    Ok(match owner {
        Owner::Adt => Use {
            action: adt.action,
            handle: adt.action_slot_handle,
            last: adt.last_slot_identifier.clone(),
        },
        Owner::Strip(t, s) => {
            let x = adt
                .nla_tracks
                .get(t)
                .and_then(|t| t.strips.get(s))
                .ok_or_else(removed)?;
            Use {
                action: x.action,
                handle: x.action_slot_handle,
                last: x.last_slot_identifier.clone(),
            }
        }
    })
}

fn write_use(h: &mut dyn Host, root: Root, owner: Owner, u: Use) -> Result<(), RnaError> {
    let (main, log) = h.main_mut();
    let adt = adt_of_mut(main, log, root)
        .and_then(Option::as_mut)
        .ok_or_else(removed)?;
    match owner {
        Owner::Adt => {
            adt.action = u.action;
            adt.action_slot_handle = u.handle;
            adt.last_slot_identifier = u.last;
        }
        Owner::Strip(t, s) => {
            let x = adt
                .nla_tracks
                .get_mut(t)
                .and_then(|t| t.strips.get_mut(s))
                .ok_or_else(removed)?;
            x.action = u.action;
            x.action_slot_handle = u.handle;
            x.last_slot_identifier = u.last;
        }
    }
    Ok(())
}

fn action(main: &Main, a: Option<IdRef<ActionId>>) -> Option<&Action> {
    main.actions.get(a?)
}

/// `generic_assign_action_slot` step 1: un-assigns the current slot, remembering its identifier.
fn unassign_slot(main: &Main, u: &mut Use) {
    if let Some(s) = action(main, u.action).and_then(|a| a.slot(u.handle)) {
        u.last = s.identifier.clone();
    }
    u.handle = 0;
}

/// The rest of `generic_assign_action_slot`: binds slot `handle` of the use's action to the
/// animated ID's type and records it.
fn assign_slot(h: &mut dyn Host, u: &mut Use, handle: i32, id_type: &str) {
    let (main, log) = h.main_mut();
    let Some(a) = u.action.and_then(|a| main.actions.get_mut(a, log)) else {
        return;
    };
    a.slot_setup_for_id(handle, id_type);
    if let Some(s) = a.slot(handle) {
        u.handle = handle;
        u.last = s.identifier.clone();
    }
}

/// `generic_assign_action` (+ the NLA's lone-slot fallback for strips).
fn assign_action(
    h: &mut dyn Host,
    this: &RnaPtr,
    new: Option<IdRef<ActionId>>,
) -> Result<(), RnaError> {
    let owner = owner_of(h.main(), this)?;
    let (id_type, id_name) = animated_id(h.main(), this.root)?;
    let mut u = read_use(h.main(), this.root, owner)?;
    if u.action.is_some() {
        if u.handle != 0 {
            unassign_slot(h.main(), &mut u);
        }
        u.action = None;
    }
    if let Some(new) = new {
        u.action = Some(new);
        let a = action(h.main(), Some(new)).ok_or_else(removed)?;
        let mut slot = a.slot_for_autoassign(id_type, &id_name, &u.last);
        if slot.is_none() && matches!(owner, Owner::Strip(..)) {
            slot = match a.slots.as_slice() {
                [only] if only.is_suitable_for(id_type) => Some(only.handle),
                _ => None,
            };
        }
        if let Some(s) = slot {
            assign_slot(h, &mut u, s, id_type);
        }
    }
    write_use(h, this.root, owner, u)
}

/// Why a slot cannot be assigned (`ActionSlotAssignmentResult`).
enum Refused {
    MissingAction,
    NotFromAction(SmolStr),
    NotSuitable(SmolStr, &'static str),
}

impl Refused {
    fn error(self) -> RnaError {
        match self {
            Refused::MissingAction => report("Cannot set slot without an assigned Action."),
            Refused::NotFromAction(id) => report(format!(
                "This slot ({id}) does not belong to the assigned Action"
            )),
            Refused::NotSuitable(id, code) => report(format!(
                "This slot ({id}) is not suitable for this data-block type ({code})"
            )),
        }
    }
}

/// `generic_assign_action_slot(slot, …)`: `slot` is `(action, handle)` or `None` (un-assign).
fn assign_action_slot(
    h: &mut dyn Host,
    this: &RnaPtr,
    slot: Option<(IdRef<ActionId>, i32)>,
) -> Result<(), Refused> {
    let main = h.main();
    let Ok(owner) = owner_of(main, this) else {
        return Err(Refused::MissingAction);
    };
    let Ok((id_type, _)) = animated_id(main, this.root) else {
        return Err(Refused::MissingAction);
    };
    let Ok(mut u) = read_use(main, this.root, owner) else {
        return Err(Refused::MissingAction);
    };
    let Some(current) = u.action else {
        return Err(Refused::MissingAction);
    };
    if let Some((a, handle)) = slot {
        let identifier = action(main, Some(a))
            .and_then(|x| x.slot(handle))
            .map(|s| (s.identifier.clone(), s.is_suitable_for(id_type)));
        let Some((identifier, suitable)) = identifier else {
            return Err(Refused::MissingAction);
        };
        if a != current {
            return Err(Refused::NotFromAction(identifier));
        }
        if !suitable {
            let code = frap_anim::action::id_type_prefix(id_type);
            return Err(Refused::NotSuitable(identifier, code));
        }
    }
    unassign_slot(h.main(), &mut u);
    if let Some((_, handle)) = slot {
        assign_slot(h, &mut u, handle, id_type);
    }
    write_use(h, this.root, owner, u).map_err(|_| Refused::MissingAction)
}

/// The `(action, handle)` of an `ActionSlot` pointer value.
fn slot_of_ptr(main: &Main, p: &RnaPtr) -> Option<(IdRef<ActionId>, i32)> {
    let Root::Id(a) = p.root else {
        return None;
    };
    let a: IdRef<ActionId> = a.typed()?;
    let act = main.actions.get(a)?;
    let (loc, _) = frap_anim::rna::action::locate(act, &p.path)?;
    let Loc::Slot(i) = loc else {
        return None;
    };
    Some((a, act.slots.get(i)?.handle))
}

/// The `ActionSlot` pointer of slot `handle` of `a`, `None` when unassigned.
fn slot_ptr(main: &Main, a: Option<IdRef<ActionId>>, handle: i32) -> Value {
    let Some(a) = a else {
        return Value::None;
    };
    match main.actions.get(a).and_then(|x| x.slot(handle)) {
        Some(s) if handle != 0 => Value::Ptr(Box::new(
            RnaPtr::id(a.any(), &frap_spec_rna::ACTION)
                .child(Seg::Prop("slots"), &frap_spec_rna::ACTION_SLOT)
                .child(Seg::Key(s.identifier.clone()), &frap_spec_rna::ACTION_SLOT),
        )),
        _ => Value::None,
    }
}

/// `action_suitable_slots`: the slots of the action suitable for the animated ID.
struct SuitableSlots<'a> {
    action: Option<&'a Action>,
    indices: Vec<usize>,
}

impl RnaColl for SuitableSlots<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::ACTION_SLOT
    }
    fn len(&self) -> usize {
        self.indices.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        let a = self.action?;
        Some(a.slots.get(*self.indices.get(i)?)?.identifier.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.key_of(i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        match (self.action, self.indices.get(i)) {
            (Some(a), Some(&s)) => Ok(Item::Struct(StructRef::Owned(Box::new(Node {
                a,
                loc: Loc::Slot(s),
            })))),
            _ => Err(frap_rna::traits::index_out_of_range(i, self.len())),
        }
    }
}

fn get_slot<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let owner = owner_of(main, this)?;
    let u = read_use(main, this.root, owner)?;
    Ok(Field::Value(slot_ptr(main, u.action, u.handle)))
}

fn get_suitable<'h>(h: &'h dyn HostRead, this: &RnaPtr) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let owner = owner_of(main, this)?;
    let u = read_use(main, this.root, owner)?;
    let (id_type, _) = animated_id(main, this.root)?;
    let a = action(main, u.action);
    let indices = a.map_or_else(Vec::new, |a| {
        (0..a.slots.len())
            .filter(|&i| a.slots[i].is_suitable_for(id_type))
            .collect()
    });
    Ok(Field::Coll(CollRef::Owned(Box::new(SuitableSlots {
        action: a,
        indices,
    }))))
}

fn set_action(
    h: &mut dyn Host,
    this: &RnaPtr,
    _p: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let new = match v {
        Value::Id(Some(a)) => a.typed(),
        _ => None,
    };
    assign_action(h, this, new)
}

fn set_slot(
    h: &mut dyn Host,
    this: &RnaPtr,
    _p: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let slot = match &v {
        Value::Ptr(p) => Some(slot_of_ptr(h.main(), p).ok_or_else(removed)?),
        _ => None,
    };
    assign_action_slot(h, this, slot).map_err(Refused::error)
}

fn set_slot_handle(
    h: &mut dyn Host,
    this: &RnaPtr,
    _p: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let handle = match v {
        Value::Int(i) => i32::try_from(i).unwrap_or(0),
        _ => 0,
    };
    let owner = owner_of(h.main(), this)?;
    let u = read_use(h.main(), this.root, owner)?;
    // `generic_assign_action_slot_handle`; its problems are global reports, not exceptions.
    let Some(a) = u.action else {
        if handle == 0 {
            write_use(h, this.root, owner, Use { handle: 0, ..u })?;
        }
        return Ok(());
    };
    let slot = action(h.main(), Some(a))
        .and_then(|x| x.slot(handle))
        .map(|s| (a, s.handle));
    let _ = assign_action_slot(h, this, slot);
    Ok(())
}

/// Declarations: the bindings answer `NeedsHook` for these properties.
macro_rules! decls {
    ($(($srna:literal, $prop:literal, $coll:literal)),* $(,)?) => {$(
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        const _: () = {
            #[allow(unsafe_code)]
            #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
            #[linkme(crate = frap_rna::linkme)]
            static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
                srna: $srna,
                prop: $prop,
                coll: $coll,
            };
        };
    )*};
}

decls! {
    ("AnimData", "action", false),
    ("AnimData", "action_slot", false),
    ("AnimData", "action_slot_handle", false),
    ("AnimData", "action_suitable_slots", true),
    ("NlaStrip", "action", false),
    ("NlaStrip", "action_slot", false),
    ("NlaStrip", "action_slot_handle", false),
    ("NlaStrip", "action_suitable_slots", true),
}

#[rna_hook(srna = "AnimData", prop = "action", set = set_action, krate = crate)]
mod adt_action {}

#[rna_hook(srna = "NlaStrip", prop = "action", set = set_action, krate = crate)]
mod strip_action {}

#[rna_hook(srna = "AnimData", prop = "action_slot", get = slot_get, set = set_slot, krate = crate)]
mod adt_slot {}

#[rna_hook(srna = "NlaStrip", prop = "action_slot", get = slot_get, set = set_slot, krate = crate)]
mod strip_slot {}

#[rna_hook(srna = "AnimData", prop = "action_slot_handle", set = set_slot_handle, krate = crate)]
mod adt_slot_handle {}

#[rna_hook(srna = "NlaStrip", prop = "action_slot_handle", set = set_slot_handle, krate = crate)]
mod strip_slot_handle {}

#[rna_hook(srna = "AnimData", prop = "action_suitable_slots", get = suitable_get, krate = crate)]
mod adt_suitable {}

#[rna_hook(srna = "NlaStrip", prop = "action_suitable_slots", get = suitable_get, krate = crate)]
mod strip_suitable {}

fn slot_get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _p: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    get_slot(h, this)
}

fn suitable_get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _p: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    get_suitable(h, this)
}
