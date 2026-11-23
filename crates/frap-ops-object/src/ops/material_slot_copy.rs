// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_copy()`, tier T0
//! (`tests/golden/cases/object/data_m8/material_slot_copy.py`). Port of
//! `material_slot_copy_exec` and `material_slot_populated_poll`
//! (`editors/render/render_shading.cc`), `BKE_object_material_array_assign`,
//! `object_material_assign` and `BKE_object_material_slot_remove`
//! (`blenkernel/intern/material.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4:
//! - poll: an active object with a material slot (`actcol > 0`; `failed, context is
//!   incorrect` otherwise);
//! - the active object's slot materials (each slot's effective material: the object's own
//!   for an `OBJECT` slot, the data's otherwise) are assigned to every other selected editable
//!   mesh object: a target with more slots first loses its **active** slot until the counts
//!   match (`BKE_object_material_slot_remove`, face indices shifted), then each slot gets the
//!   material where the target's active slot links (`BKE_MAT_ASSIGN_USERPREF`: the data when
//!   the target has no active slot), its data growing as needed;
//! - a target sharing the active object's data only takes the `OBJECT` slots (it inherits
//!   their link first); its data slots are left alone;
//! - a target whose slot count then equals the source's takes its active slot; always
//!   `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MaterialId, ObjectId};
use frap_scene::id::{SlotLink, slots};
use frap_scene::{ChangeLog, Context, Main};
use frap_spec_ops_object::object;

use super::material_slot_add::common::{actcol, mesh_of, slot_remove, users_of};

/// `material_slot_populated_poll`: an active object whose `actcol` is set.
fn populated_poll(c: &Context) -> Result<(), PollFail> {
    poll::active_object(c)?;
    let (_, ob) = poll::active(c).expect("invariant: checked by active_object");
    if actcol(ob, slots::slot_count(c.main, ob)) == 0 {
        return Err(poll::fail(
            PollReq::ActiveObject,
            "the active object needs a material slot (bpy.ops.object.material_slot_add())",
        ));
    }
    Ok(())
}

/// `material_slot_populated_poll`.
const POPULATED_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject],
    check: populated_poll,
};

/// The link `object_material_assign` gives a slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Assign {
    /// `BKE_MAT_ASSIGN_OBJECT`.
    Object,
    /// `BKE_MAT_ASSIGN_USERPREF`: the link of the active slot `actcol` (1-based, 0: none),
    /// else the factory preference (the data).
    UserPref(usize),
}

/// `object_material_assign(bmain, ob, ma, i + 1, assign_type, true)` for a mesh object.
fn assign(
    main: &mut Main,
    log: &mut ChangeLog,
    ob: IdRef<ObjectId>,
    ma: Option<IdRef<MaterialId>>,
    i: usize,
    how: Assign,
) {
    let Some(me) = mesh_of(main, ob) else {
        return;
    };
    let Some(o) = main.objects.get(ob) else {
        return;
    };
    let old = slots::slot_count(main, o);
    if i >= old {
        // The data's array grows (`*totcolp = act`), and so do the objects' arrays
        // (`MEM_realloc_zeroed_id`, `BKE_objects_materials_sync_length_all`).
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.materials.resize(i + 1, None);
        }
        for u in users_of(main, me) {
            if main
                .objects
                .get(u)
                .is_some_and(|x| x.material_slot_overrides.len() > old)
                && let Some(x) = main.objects.get_mut(u, log)
            {
                x.material_slot_overrides.truncate(old);
            }
        }
    }
    let totcol = old.max(i + 1);
    let Some(o) = main.objects.get(ob) else {
        return;
    };
    let in_object = match how {
        Assign::Object => true,
        Assign::UserPref(act) if act >= 1 && act <= totcol => o
            .material_slot_overrides
            .get(act - 1)
            .is_some_and(|s| s.link == SlotLink::Object),
        // `U.flag & USER_MAT_ON_OB` is off in the factory preferences.
        Assign::UserPref(_) => false,
    };
    if in_object {
        if let Some(o) = main.objects.get_mut(ob, log) {
            let s = slots::override_mut(o, i);
            s.link = SlotLink::Object;
            s.material = ma;
        }
    } else {
        if main
            .objects
            .get(ob)
            .and_then(|o| o.material_slot_overrides.get(i))
            .is_some_and(|s| s.link == SlotLink::Object)
            && let Some(o) = main.objects.get_mut(ob, log)
        {
            slots::override_mut(o, i).link = SlotLink::Data;
        }
        if let Some(m) = main.meshes.get_mut(me, log)
            && let Some(slot) = m.materials.get_mut(i)
        {
            *slot = ma;
        }
    }
}

/// `ob->actcol` as Blender keeps it: frap's 0-based index, 0 without slots.
fn actcol_of(main: &Main, ob: IdRef<ObjectId>) -> usize {
    main.objects
        .get(ob)
        .map_or(0, |o| actcol(o, slots::slot_count(main, o)))
}

fn set_actcol(main: &mut Main, log: &mut ChangeLog, ob: IdRef<ObjectId>, act: usize) {
    let index = act.saturating_sub(1) as i32;
    if main
        .objects
        .get(ob)
        .is_some_and(|o| o.active_material_index != index)
        && let Some(o) = main.objects.get_mut(ob, log)
    {
        o.active_material_index = index;
    }
}

/// `BKE_object_material_array_assign(bmain, ob, &matar, totcol, to_object_only)`.
fn array_assign(
    main: &mut Main,
    log: &mut ChangeLog,
    ob: IdRef<ObjectId>,
    matar: &[Option<IdRef<MaterialId>>],
    to_object_only: bool,
) {
    let totcol = matar.len();
    let actcol_orig = actcol_of(main, ob);
    let count = |main: &Main| {
        main.objects
            .get(ob)
            .map_or(0, |o| slots::slot_count(main, o))
    };
    while count(main) > totcol && slot_remove(main, log, ob) {}
    // `object_material_assign` reads `ob->actcol` and never changes it.
    let act = actcol_of(main, ob);
    for (i, &ma) in matar.iter().enumerate() {
        if to_object_only
            && main
                .objects
                .get(ob)
                .and_then(|o| o.material_slot_overrides.get(i))
                .is_none_or(|s| s.link != SlotLink::Object)
        {
            continue;
        }
        let how = if to_object_only {
            Assign::Object
        } else {
            Assign::UserPref(act)
        };
        assign(main, log, ob, ma, i, how);
    }
    let act = actcol_orig.min(count(main));
    set_actcol(main, log, ob, act);
}

#[operator(poll = POPULATED_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotCopy) -> OpResult {
    let (ob, targets) = {
        let c = cx.ctx();
        (c.object(), c.selected_editable_objects())
    };
    let Some(ob) = ob else {
        return Ok(OpStatus::Cancelled);
    };
    let (main, log) = cx.main_mut();
    let Some(src) = main.objects.get(ob) else {
        return Ok(OpStatus::Cancelled);
    };
    if mesh_of(main, ob).is_none() {
        return Ok(OpStatus::Cancelled);
    }
    let totcol = slots::slot_count(main, src);
    let matar: Vec<Option<IdRef<MaterialId>>> =
        (0..totcol).map(|i| slots::slot(main, src, i).1).collect();
    let src_bits: Vec<bool> = (0..totcol)
        .map(|i| slots::slot(main, src, i).0 == SlotLink::Object)
        .collect();
    let src_data = src.data;
    let src_act = actcol(src, totcol);
    for target in targets {
        if target == ob || mesh_of(main, target).is_none() {
            continue;
        }
        let same_data = main.objects.get(target).is_some_and(|o| o.data == src_data);
        if same_data
            && src_bits.iter().any(|&b| b)
            && let Some(o) = main.objects.get_mut(target, log)
        {
            for (i, &bit) in src_bits.iter().enumerate() {
                if bit {
                    slots::override_mut(o, i).link = SlotLink::Object;
                }
            }
        }
        array_assign(main, log, target, &matar, same_data);
        if main
            .objects
            .get(target)
            .is_some_and(|o| slots::slot_count(main, o) == totcol)
        {
            set_actcol(main, log, target, src_act);
        }
    }
    Ok(OpStatus::Finished)
}
